use backtrace::Backtrace;
#[cfg(target_os = "linux")]
use libc;
use log::{info, warn};
#[cfg(target_os = "linux")]
use rand::Rng;
use std::alloc::{GlobalAlloc, Layout};
use std::cell::RefCell;
use std::cmp::{max, min};
#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::mem;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

const MEBIBYTE: usize = 1024 * 1024;
const MIN_BLOCK_SIZE: usize = 1000;
const SMALL_BLOCK_TRACE_PROBABILITY: usize = 1;
const REPORT_USAGE_INTERVAL: usize = 512 * MEBIBYTE;
const SKIP_ADDR: u64 = 0x700000000000;
const PRINT_STACK_TRACE_ON_MEMORY_SPIKE: bool = true;

#[cfg(target_os = "linux")]
const ENABLE_STACK_TRACE: bool = true;

// Currently there is no point in getting stack traces on non-linux platform, because other tools don't support linux.
#[cfg(not(target_os = "linux"))]
const ENABLE_STACK_TRACE: bool = false;

const COUNTERS_SIZE: usize = 16384;
static TOTAL_MEMORY_USAGE: AtomicUsize = AtomicUsize::new(0);
static MEM_SIZE: [AtomicUsize; COUNTERS_SIZE] = unsafe {
    // SAFETY: Rust [guarantees](https://doc.rust-lang.org/stable/std/sync/atomic/struct.AtomicUsize.html)
    // that `usize` and `AtomicUsize` have the same representation.
    std::mem::transmute::<[usize; COUNTERS_SIZE], [AtomicUsize; COUNTERS_SIZE]>([0usize; 16384])
};
static MEM_CNT: [AtomicUsize; COUNTERS_SIZE] = unsafe {
    std::mem::transmute::<[usize; COUNTERS_SIZE], [AtomicUsize; COUNTERS_SIZE]>([0usize; 16384])
};

static mut SKIP_PTR: [u8; 1 << 20] = [0; 1 << 20];
static mut CHECKED_PTR: [u8; 1 << 20] = [0; 1 << 20];

const STACK_SIZE: usize = 1;
const MAX_STACK: usize = 15;
const SAVE_STACK_TRACES_TO_FILE: bool = false;

const SKIPPED_TRACE: *mut c_void = 1 as *mut c_void;
const MISSING_TRACE: *mut c_void = 2 as *mut c_void;

#[repr(C)]
struct AllocHeader {
    magic: u64,
    size: u64,
    tid: u64,
    stack: [*mut c_void; STACK_SIZE],
}

const HEADER_SIZE: usize = mem::size_of::<AllocHeader>();
const MAGIC_RUST: usize = 0x12345678991100;

thread_local! {
    pub static TID: RefCell<usize> = RefCell::new(0);
    pub static IN_TRACE: RefCell<usize> = RefCell::new(0);
    pub static MEMORY_USAGE_MAX: RefCell<usize> = RefCell::new(0);
    pub static MEMORY_USAGE_LAST_REPORT: RefCell<usize> = RefCell::new(0);
}

#[cfg(not(target_os = "linux"))]
pub static NTHREADS: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "linux")]
pub fn get_tid() -> usize {
    let res = TID.with(|t| {
        if *t.borrow() == 0 {
            *t.borrow_mut() = nix::unistd::gettid().as_raw() as usize;
        }
        *t.borrow()
    });
    res
}

#[cfg(not(target_os = "linux"))]
pub fn get_tid() -> usize {
    let res = TID.with(|t| {
        if *t.borrow() == 0 {
            *t.borrow_mut() = NTHREADS.fetch_add(1, Ordering::SeqCst) as usize;
        }
        *t.borrow()
    });
    res
}

pub fn murmur64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.overflowing_mul(0xff51afd7ed558ccd).0;
    h ^= h >> 33;
    h = h.overflowing_mul(0xc4ceb9fe1a85ec53).0;
    h ^= h >> 33;
    return h;
}

const IGNORE_START: &'static [&'static str] = &[
    "__rg_",
    "_ZN5actix",
    "_ZN5alloc",
    "_ZN6base64",
    "_ZN6cached",
    "_ZN4core",
    "_ZN9hashbrown",
    "_ZN20reed_solomon_erasure",
    "_ZN5tokio",
    "_ZN10tokio_util",
    "_ZN3std",
    "_ZN8smallvec",
];

const IGNORE_INSIDE: &'static [&'static str] = &[
    "$LT$actix..",
    "$LT$alloc..",
    "$LT$base64..",
    "$LT$cached..",
    "$LT$core..",
    "$LT$hashbrown..",
    "$LT$reed_solomon_erasure..",
    "$LT$tokio..",
    "$LT$tokio_util..",
    "$LT$serde_json..",
    "$LT$std..",
    "$LT$tracing_subscriber..",
];

fn skip_ptr(addr: *mut c_void) -> bool {
    if addr as u64 >= SKIP_ADDR {
        return true;
    }
    let mut found = false;
    backtrace::resolve(addr, |symbol| {
        if let Some(name) = symbol.name() {
            let name = name.as_str().unwrap_or("");
            for &s in IGNORE_START {
                if name.starts_with(s) {
                    found = true;
                    break;
                }
            }
            for &s in IGNORE_INSIDE {
                if name.contains(s) {
                    found = true;
                    break;
                }
            }
        }
    });

    return found;
}

pub fn total_memory_usage() -> usize {
    TOTAL_MEMORY_USAGE.load(Ordering::SeqCst)
}

pub fn current_thread_memory_usage() -> usize {
    let tid = get_tid();
    let memory_usage = MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::SeqCst);
    memory_usage
}

pub fn thread_memory_usage(tid: usize) -> usize {
    let memory_usage = MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::SeqCst);
    memory_usage
}

pub fn thread_memory_count(tid: usize) -> usize {
    let memory_cnt = MEM_CNT[tid % COUNTERS_SIZE].load(Ordering::SeqCst);
    memory_cnt
}

pub fn current_thread_peak_memory_usage() -> usize {
    MEMORY_USAGE_MAX.with(|x| *x.borrow())
}

pub fn reset_memory_usage_max() {
    let tid = get_tid();
    let memory_usage = MEM_SIZE[tid % COUNTERS_SIZE].load(Ordering::SeqCst);
    MEMORY_USAGE_MAX.with(|x| *x.borrow_mut() = memory_usage);
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
        let new_layout =
            Layout::from_size_align(layout.size() + HEADER_SIZE, layout.align()).unwrap();

        let res = self.inner.alloc(new_layout);

        let tid = get_tid();
        let memory_usage = layout.size()
            + MEM_SIZE[tid % COUNTERS_SIZE].fetch_add(layout.size(), Ordering::SeqCst);
        TOTAL_MEMORY_USAGE.fetch_add(layout.size(), Ordering::SeqCst);
        MEM_CNT[tid % COUNTERS_SIZE].fetch_add(1, Ordering::SeqCst);

        if PRINT_STACK_TRACE_ON_MEMORY_SPIKE
            && memory_usage > REPORT_USAGE_INTERVAL + MEMORY_USAGE_LAST_REPORT.with(|x| *x.borrow())
        {
            if IN_TRACE.with(|in_trace| *in_trace.borrow()) == 0 {
                IN_TRACE.with(|in_trace| *in_trace.borrow_mut() = 1);
                MEMORY_USAGE_LAST_REPORT.with(|x| *x.borrow_mut() = memory_usage);

                let bt = Backtrace::new();

                warn!(
                    "Thread {} reached new record of memory usage {}MiB\n{:?} added: {:?}",
                    tid,
                    memory_usage / MEBIBYTE,
                    bt,
                    layout.size() / MEBIBYTE,
                );
                IN_TRACE.with(|in_trace| *in_trace.borrow_mut() = 0);
            }
        }
        if memory_usage > MEMORY_USAGE_MAX.with(|x| *x.borrow()) {
            MEMORY_USAGE_MAX.with(|x| *x.borrow_mut() = memory_usage);
        }

        let mut addr: Option<*mut c_void> = Some(MISSING_TRACE);
        let mut ary: [*mut c_void; MAX_STACK + 1] = [0 as *mut c_void; MAX_STACK + 1];
        let mut chosen_i = 0;

        #[cfg(target_os = "linux")]
        if ENABLE_STACK_TRACE && IN_TRACE.with(|in_trace| *in_trace.borrow()) == 0 {
            IN_TRACE.with(|in_trace| *in_trace.borrow_mut() = 1);
            if layout.size() >= MIN_BLOCK_SIZE
                || rand::thread_rng().gen_range(0, 100) < SMALL_BLOCK_TRACE_PROBABILITY
            {
                let size = libc::backtrace(ary.as_ptr() as *mut *mut c_void, MAX_STACK as i32);
                ary[0] = 0 as *mut c_void;
                for i in 1..min(size as usize, MAX_STACK) {
                    addr = Some(ary[i] as *mut c_void);
                    chosen_i = i;
                    if ary[i] < SKIP_ADDR as *mut c_void {
                        let hash = murmur64(ary[i] as u64) % (1 << 23);
                        if (SKIP_PTR[(hash / 8) as usize] >> hash % 8) & 1 == 1 {
                            continue;
                        }
                        if (CHECKED_PTR[(hash / 8) as usize] >> hash % 8) & 1 == 1 {
                            break;
                        }
                        if SAVE_STACK_TRACES_TO_FILE {
                            backtrace::resolve(ary[i], |symbol| {
                                let fname = format!("/tmp/logs/{}", tid);
                                if let Ok(mut f) = OpenOptions::new()
                                    .create(true)
                                    .write(true)
                                    .append(true)
                                    .open(fname)
                                {
                                    if let Some(path) = symbol.filename() {
                                        f.write(
                                            format!(
                                                "PATH {:?} {} {}\n",
                                                ary[i],
                                                symbol.lineno().unwrap_or(0),
                                                path.to_str().unwrap_or("<UNKNOWN>")
                                            )
                                            .as_bytes(),
                                        )
                                        .unwrap();
                                    }
                                    if let Some(name) = symbol.name() {
                                        f.write(
                                            format!("SYMBOL {:?} {}\n", ary[i], name.to_string())
                                                .as_bytes(),
                                        )
                                        .unwrap();
                                    }
                                }
                            });
                        }

                        let should_skip = skip_ptr(ary[i]);
                        if should_skip {
                            SKIP_PTR[(hash / 8) as usize] |= 1 << hash % 8;
                            continue;
                        }
                        CHECKED_PTR[(hash / 8) as usize] |= 1 << hash % 8;

                        if SAVE_STACK_TRACES_TO_FILE {
                            let fname = format!("/tmp/logs/{}", tid);

                            if let Ok(mut f) = OpenOptions::new()
                                .create(true)
                                .write(true)
                                .append(true)
                                .open(fname)
                            {
                                f.write(format!("STACK_FOR {:?}\n", addr).as_bytes())
                                    .unwrap();
                                let ary2: [*mut c_void; 256] = [0 as *mut c_void; 256];
                                let size2 = libc::backtrace(ary2.as_ptr() as *mut *mut c_void, 256)
                                    as usize;
                                for i in 0..size2 {
                                    let addr2 = ary2[i];

                                    backtrace::resolve(addr2, |symbol| {
                                        if let Some(name) = symbol.name() {
                                            let name = name.as_str().unwrap_or("");

                                            f.write(
                                                format!("STACK {:?} {:?} {:?}\n", i, addr2, name)
                                                    .as_bytes(),
                                            )
                                            .unwrap();
                                        }
                                    });
                                }
                            }
                        }
                        break;
                    }
                }
            } else {
                addr = Some(SKIPPED_TRACE);
            }
            IN_TRACE.with(|in_trace| *in_trace.borrow_mut() = 0);
        }

        let mut stack = [0 as *mut c_void; STACK_SIZE];
        stack[0] = addr.unwrap_or(0 as *mut c_void);
        for i in 1..STACK_SIZE {
            stack[i] =
                ary[min(MAX_STACK as isize, max(0, chosen_i as isize + i as isize)) as usize];
        }

        let header = AllocHeader {
            magic: (MAGIC_RUST + STACK_SIZE) as u64,
            size: layout.size() as u64,
            tid: tid as u64,
            stack,
        };

        *(res as *mut AllocHeader) = header;

        res.offset(HEADER_SIZE as isize)
    }

    unsafe fn dealloc(&self, mut ptr: *mut u8, layout: Layout) {
        let new_layout =
            Layout::from_size_align(layout.size() + HEADER_SIZE, layout.align()).unwrap();

        ptr = ptr.offset(-(HEADER_SIZE as isize));

        (*(ptr as *mut AllocHeader)).magic = (MAGIC_RUST + STACK_SIZE + 0x100) as u64;
        let tid: usize = (*(ptr as *mut AllocHeader)).tid as usize;

        MEM_SIZE[tid % COUNTERS_SIZE].fetch_sub(layout.size(), Ordering::SeqCst);
        TOTAL_MEMORY_USAGE.fetch_sub(layout.size(), Ordering::SeqCst);
        MEM_CNT[tid % COUNTERS_SIZE].fetch_sub(1, Ordering::SeqCst);

        self.inner.dealloc(ptr, new_layout);
    }
}

pub fn print_counters_ary() {
    info!("tid {}", get_tid());
    let mut total_cnt: usize = 0;
    let mut total_size: usize = 0;
    for idx in 0..COUNTERS_SIZE {
        let val: usize = MEM_SIZE.get(idx).unwrap().load(Ordering::SeqCst);
        if val != 0 {
            let cnt = MEM_CNT.get(idx).unwrap().load(Ordering::SeqCst);
            total_cnt += cnt;
            info!("COUNTERS {}: {} {}", idx, cnt, val);
            total_size += val;
        }
    }
    info!("COUNTERS TOTAL {} {}", total_cnt, total_size);
}
