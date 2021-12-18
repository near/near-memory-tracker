use backtrace::Backtrace;
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{fs, mem};

const MEBIBYTE: usize = 1024 * 1024;
const SKIP_ADDR: *mut c_void = 0x700000000000 as *mut c_void;
/// Configure how often should we print stack trace, whenever new record is reached.
pub(crate) static REPORT_USAGE_INTERVAL: AtomicUsize = AtomicUsize::new(512 * MEBIBYTE);
/// Should be a configurable option.
pub(crate) static SAVE_STACK_TRACES_TO_FILE: AtomicBool = AtomicBool::new(false);
/// Should be a configurable option.
pub(crate) static ENABLE_STACK_TRACE: AtomicBool = AtomicBool::new(false);
/// TODO: Make this configurable.
const LOGS_PATH: &str = "/tmp/logs";

const COUNTERS_SIZE: usize = 16384;
static MEM_SIZE: [AtomicUsize; COUNTERS_SIZE] = unsafe {
    // SAFETY: Rust [guarantees](https://doc.rust-lang.org/stable/std/sync/atomic/struct.AtomicUsize.html)
    // that `usize` and `AtomicUsize` have the same representation.
    std::mem::transmute::<[usize; COUNTERS_SIZE], [AtomicUsize; COUNTERS_SIZE]>(
        [0usize; COUNTERS_SIZE],
    )
};
static MEM_CNT: [AtomicUsize; COUNTERS_SIZE] = unsafe {
    std::mem::transmute::<[usize; COUNTERS_SIZE], [AtomicUsize; COUNTERS_SIZE]>(
        [0usize; COUNTERS_SIZE],
    )
};

const CACHE_SIZE: usize = 1 << 20;
static mut SKIP_CACHE: [u8; CACHE_SIZE] = [0; CACHE_SIZE];
static mut CHECKED_CACHE: [u8; CACHE_SIZE] = [0; CACHE_SIZE];

const STACK_SIZE: usize = 1;

#[repr(C)]
struct AllocHeader {
    magic: u64,
    size: u64,
    tid: u64,
    stack: [*mut c_void; STACK_SIZE],
}

const ALLOC_HEADER_SIZE: usize = mem::size_of::<AllocHeader>();
const MAGIC_RUST: usize = 0x12345678991100;
const FREED_MAGIC: usize = 0x100;

thread_local! {
    static TID: Cell<usize> = Cell::new(0);
    static MEMORY_USAGE_MAX: Cell<usize> = Cell::new(0);
    static MEMORY_USAGE_LAST_REPORT: Cell<usize> = Cell::new(0);
    static NUM_ALLOCATIONS: Cell<usize> = Cell::new(0);
    static IN_TRACE: Cell<usize> = Cell::new(0);
}

pub fn get_tid() -> usize {
    TID.with(|f| {
        let mut v = f.get();
        if v == 0 {
            // thread::current().id().as_u64() is still unstable
            #[cfg(target_os = "linux")]
            {
                v = nix::unistd::gettid().as_raw() as usize;
            }
            #[cfg(not(target_os = "linux"))]
            {
                static NTHREADS: AtomicUsize = AtomicUsize::new(0);
                v = NTHREADS.fetch_add(1, Ordering::Relaxed) as usize;
            }
            f.set(v)
        }
        v
    })
}

fn murmur64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.overflowing_mul(0xff51afd7ed558ccd).0;
    h ^= h >> 33;
    h = h.overflowing_mul(0xc4ceb9fe1a85ec53).0;
    h ^= h >> 33;
    h
}

const IGNORE_START: &[&str] = &[
    "__rg_",
    "_ZN10tokio_util",
    "_ZN20reed_solomon_erasure",
    "_ZN3std",
    "_ZN4core",
    "_ZN5actix",
    "_ZN5alloc",
    "_ZN5tokio",
    "_ZN6base64",
    "_ZN6cached",
    "_ZN8smallvec",
    "_ZN9hashbrown",
];

const IGNORE_INSIDE: &[&str] = &[
    "$LT$actix..",
    "$LT$alloc..",
    "$LT$base64..",
    "$LT$cached..",
    "$LT$core..",
    "$LT$hashbrown..",
    "$LT$reed_solomon_erasure..",
    "$LT$serde_json..",
    "$LT$std..",
    "$LT$tokio..",
    "$LT$tokio_util..",
    "$LT$tracing_subscriber..",
    "allocator",
];

fn skip_ptr(addr: *mut c_void) -> bool {
    let mut found = false;
    backtrace::resolve(addr, |symbol| {
        found = found
            || symbol
                .name()
                .map(|name| name.as_str())
                .flatten()
                .map(|name| {
                    IGNORE_START.iter().filter(|s: &&&str| name.starts_with(**s)).any(|_| true)
                        || IGNORE_INSIDE.iter().filter(|s: &&&str| name.contains(**s)).any(|_| true)
                })
                .unwrap_or_default()
    });

    found
}

pub fn total_memory_usage() -> usize {
    MEM_SIZE.iter().map(|v| v.load(Ordering::Relaxed)).sum()
}

pub fn current_thread_memory_usage() -> usize {
    let tid = get_tid();

    MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::Relaxed)
}

pub fn thread_memory_usage(tid: usize) -> usize {
    MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::Relaxed)
}

pub fn thread_memory_count(tid: usize) -> usize {
    MEM_CNT[tid % COUNTERS_SIZE].load(Ordering::Relaxed)
}

pub fn current_thread_peak_memory_usage() -> usize {
    MEMORY_USAGE_MAX.with(|x| x.get())
}

pub fn reset_memory_usage_max() {
    let tid = get_tid();
    let memory_usage = MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::Relaxed);
    MEMORY_USAGE_MAX.with(|x| x.set(memory_usage));
}

pub struct MyAllocator<A> {
    inner: A,
}

impl<A> MyAllocator<A> {
    pub const fn new(inner: A) -> MyAllocator<A> {
        MyAllocator { inner }
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for MyAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let tid = get_tid();
        let new_layout =
            Layout::from_size_align(layout.size() + ALLOC_HEADER_SIZE, layout.align()).unwrap();

        let res = self.inner.alloc(new_layout);
        let memory_usage = MEM_SIZE[tid % COUNTERS_SIZE]
            .fetch_add(layout.size(), Ordering::Relaxed)
            + layout.size();

        MEM_CNT[tid % COUNTERS_SIZE].fetch_add(1, Ordering::Relaxed);

        MEMORY_USAGE_MAX.with(|val| {
            if val.get() < memory_usage {
                val.set(memory_usage);
            }
        });

        let mut header = AllocHeader {
            magic: (MAGIC_RUST + STACK_SIZE) as u64,
            size: layout.size() as u64,
            tid: tid as u64,
            stack: [null_mut::<c_void>(); STACK_SIZE],
        };

        IN_TRACE.with(|in_trace| {
            if in_trace.replace(1) != 0 {
                return;
            }
            Self::print_stack_trace_on_memory_spike(layout, tid, memory_usage);
            if ENABLE_STACK_TRACE.load(Ordering::Relaxed) {
                Self::compute_stack_trace(layout, tid, &mut header.stack);
            }
            in_trace.set(0);
        });
        *(res as *mut AllocHeader) = header;

        res.add(ALLOC_HEADER_SIZE)
    }

    unsafe fn dealloc(&self, mut ptr: *mut u8, layout: Layout) {
        let new_layout =
            Layout::from_size_align(layout.size() + ALLOC_HEADER_SIZE, layout.align()).unwrap();

        ptr = ptr.offset(-(ALLOC_HEADER_SIZE as isize));

        (*(ptr as *mut AllocHeader)).magic = (MAGIC_RUST + STACK_SIZE + FREED_MAGIC) as u64;
        let header_tid: usize = (*(ptr as *mut AllocHeader)).tid as usize;

        MEM_SIZE[header_tid % COUNTERS_SIZE].fetch_sub(layout.size(), Ordering::Relaxed);
        MEM_CNT[header_tid % COUNTERS_SIZE].fetch_sub(1, Ordering::Relaxed);

        self.inner.dealloc(ptr, new_layout);
    }
}

impl<A: GlobalAlloc> MyAllocator<A> {
    unsafe fn print_stack_trace_on_memory_spike(layout: Layout, tid: usize, memory_usage: usize) {
        MEMORY_USAGE_LAST_REPORT.with(|memory_usage_last_report| {
            if memory_usage
                > REPORT_USAGE_INTERVAL
                    .load(Ordering::Relaxed)
                    .saturating_add(memory_usage_last_report.get())
            {
                memory_usage_last_report.set(memory_usage);
                tracing::warn!(
                    tid,
                    memory_usage_mb = memory_usage / MEBIBYTE,
                    added_mb = layout.size() / MEBIBYTE,
                    bt = ?Backtrace::new(),
                    "reached new record of memory usage",
                );
            }
        });
    }
}

impl<A: GlobalAlloc> MyAllocator<A> {
    #[inline]
    unsafe fn compute_stack_trace(
        layout: Layout,
        tid: usize,
        stack: &mut [*mut c_void; STACK_SIZE],
    ) {
        if Self::should_compute_trace(layout) {
            const MISSING_TRACE: *mut c_void = 2 as *mut c_void;
            stack[0] = MISSING_TRACE;
            backtrace::trace(|frame| {
                let addr = frame.ip();
                stack[0] = addr as *mut c_void;
                if addr >= SKIP_ADDR as *mut c_void {
                    true
                } else {
                    let hash = murmur64(addr as u64) % (8 * CACHE_SIZE as u64);
                    let i = (hash / 8) as usize;
                    let cur_bit = 1 << (hash % 8);
                    if SKIP_CACHE[i] & cur_bit != 0 {
                        true
                    } else if CHECKED_CACHE[i] & cur_bit != 0 {
                        false
                    } else if skip_ptr(addr) {
                        SKIP_CACHE[i] |= cur_bit;
                        true
                    } else {
                        CHECKED_CACHE[i] |= cur_bit;

                        if SAVE_STACK_TRACES_TO_FILE.load(Ordering::Relaxed) {
                            Self::save_trace_to_file(tid, addr);
                        }
                        false
                    }
                }
            })
        }
    }

    unsafe fn should_compute_trace(layout: Layout) -> bool {
        match layout.size() {
            // 1% of the time
            0..=999 => {
                (murmur64(NUM_ALLOCATIONS.with(|key| {
                    // key.update() is still unstable
                    let val = key.get();
                    key.set(val + 1);
                    val
                }) as u64)
                    % 1024)
                    < 10
            }
            // 100%
            _ => true,
        }
    }

    unsafe fn save_trace_to_file(tid: usize, addr: *mut c_void) {
        backtrace::resolve(addr, |symbol| {
            let _ = fs::create_dir_all(LOGS_PATH);
            let file_name = format!("{}/{}", LOGS_PATH, tid);
            if let Ok(mut file) =
                OpenOptions::new().create(true).write(true).append(true).open(file_name)
            {
                if let Some(path) = symbol.filename() {
                    writeln!(
                        file,
                        "PATH addr={:?} symbol={} path={:?}",
                        addr,
                        symbol.lineno().unwrap_or_default(),
                        path.as_os_str()
                    )
                    .unwrap();
                }
                if let Some(name) = symbol.name() {
                    writeln!(file, "SYMBOL addr={:?} name={:?}", addr, name.as_str()).unwrap();
                }
            }
        });
    }
}

pub fn print_counters_ary() {
    tracing::info!(tid = get_tid(), "tid");
    let mut total_cnt: usize = 0;
    let mut total_size: usize = 0;
    for idx in 0..COUNTERS_SIZE {
        let val = MEM_SIZE[idx].load(Ordering::Relaxed);
        if val != 0 {
            let cnt = MEM_CNT[idx].load(Ordering::Relaxed);
            total_cnt += cnt;
            total_size += val;

            tracing::info!(idx, cnt, val, "COUNTERS");
        }
    }
    tracing::info!(total_cnt, total_size, "COUNTERS TOTAL");
}

#[cfg(test)]
mod test {
    use crate::allocator::{print_counters_ary, total_memory_usage, MyAllocator};
    use std::alloc::{GlobalAlloc, Layout};
    use std::ptr::null_mut;
    use tracing_subscriber::util::SubscriberInitExt;

    #[test]
    fn test_print_counters_ary() {
        tracing_subscriber::fmt().with_writer(std::io::stderr).finish().init();
        print_counters_ary();
    }

    static ALLOC: MyAllocator<tikv_jemallocator::Jemalloc> =
        MyAllocator::new(tikv_jemallocator::Jemalloc);

    #[test]
    // Works only if run alone.
    fn test_allocator() {
        let layout = Layout::from_size_align(32, 1).unwrap();
        let ptr = unsafe { ALLOC.alloc(layout) };
        assert_ne!(ptr, null_mut());

        assert_eq!(total_memory_usage(), 32);

        unsafe { ALLOC.dealloc(ptr, layout) };
    }
}
