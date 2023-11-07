use crate::symbols::get_symbols;
use crate::utils::{compute_present_pages, get_page_size, read_smaps, Counter, Smap, MIB};
use anyhow::Context;
use itertools::Itertools;
use near_rust_allocator_proxy::AllocHeader;
use nix::sys::uio::{IoVec, RemoteIoVec};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::fs::File;
use std::ops::Not;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, error, info};

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct AnalyzeCmd {
    #[clap(long)]
    pid: i32,
    #[clap(long)]
    print_raw_symbols: bool,
    #[clap(long, conflicts_with("print_raw_symbols"))]
    print_ptr: bool,
}

impl AnalyzeCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        info!(?self.pid);
        let smaps = read_smaps(self.pid).with_context(|| "read_smaps failed")?;

        let start = Instant::now();
        let page_map_file = PathBuf::from("/proc").join(self.pid.to_string()).join("pagemap");
        let mut file = File::open(page_map_file.clone())
            .with_context(|| format!("page_map_file not found file={:?}", page_map_file))?;

        let page_size = get_page_size()?;
        info!(?page_size);

        let proc_exe_path = PathBuf::from("/proc").join(self.pid.to_string()).join("exe");
        info!(?proc_exe_path);
        let exe_path = fs::read_link(proc_exe_path).with_context(|| "unable to read exe path")?;
        info!(?exe_path);

        let mmaped_exec = Self::get_mmaped_exe_regions(&smaps, exe_path.clone());
        info!(mapped_exec_len = ?mmaped_exec.len());
        // compute memory used in not mmaped files

        let mut ptr_2_memory: HashMap<*mut c_void, Counter> = HashMap::new();

        info!("Reading pages.");
        let mut buffer = vec![0u8; page_size + std::mem::size_of::<AllocHeader>()];

        let not_mmaped_pages: Vec<_> =
            compute_present_pages(&smaps, &mut file, page_size, false)?.to_vec();
        let total_present_pages: usize = not_mmaped_pages.iter().map(|x| (x.1.len())).sum();
        info!("Read pages.");
        for (smap, addresses) in not_mmaped_pages.iter() {
            debug!(?smap, len = addresses.len());
            assert_eq!((smap.to - smap.from) % page_size, 0, "pages not multiple of {}", page_size);

            for ad in addresses {
                let input = [IoVec::from_mut_slice(buffer.as_mut_slice())];
                let output = [RemoteIoVec { base: *ad, len: page_size }];

                nix::sys::uio::process_vm_readv(Pid::from_raw(self.pid), &input, &output)?;
                // TODO: Allocation headers, which are split between 2 consecutive pages are not counter correctly.
                for val in (0..page_size / 8).map(|v| v * 8) {
                    let ah = unsafe {
                        &mut *(buffer.as_mut_slice()[val..].as_ptr() as *mut AllocHeader)
                    };
                    if ah.is_allocated() {
                        let ptr = ah.stack()[0];
                        if ptr != usize::MAX as *mut c_void
                            && ptr.is_null().not()
                            && ah.size() < u32::MAX as usize
                        {
                            *ptr_2_memory.entry(ptr).or_default() += Counter::with_size(ah.size());
                        }
                    }
                }
            }
        }
        info!("Getting exe path");

        let str_exe_path = exe_path.to_str().unwrap();
        info!(?str_exe_path, "Getting symbols.");
        let symbols = get_symbols(str_exe_path)?;
        info!(symbols = symbols.len());

        let mut func_2_mem: HashMap<String, Counter> = HashMap::new();
        let present_allocated_with_proxy = ptr_2_memory.iter().map(|x| x.1.size).sum();
        for (ptr, val) in ptr_2_memory.iter() {
            let symbol_mappings = (mmaped_exec.iter())
                .filter(|x| ((x.from as *mut c_void) <= (*ptr)) && ((*ptr as usize) < x.to))
                .filter_map(|smap| {
                    let file_offset = (*ptr as usize) - smap.from + smap.offset;

                    if let Some(last_sym) =
                        symbols.iter().filter(|s| s.offset <= file_offset).last()
                    {
                        let key = if self.print_ptr {
                            format!("{:?}", ptr)
                        } else if self.print_raw_symbols {
                            last_sym.raw_symbol.clone()
                        } else {
                            last_sym.symbol.clone()
                        };
                        Some((key, val))
                    } else {
                        None
                    }
                })
                .collect_vec();
            if symbol_mappings.is_empty() {
                error!(?ptr, "couldn't resolve ptr");
                *func_2_mem.entry(format!("{:?}", ptr)).or_default() += *val;
            } else if symbol_mappings.len() > 1 {
                error!(?ptr, symbols = ?symbol_mappings.iter().take(10).collect_vec(), "multiple symbols mapped");
                *func_2_mem.entry(format!("{:?}", ptr)).or_default() += *val;
            }
            for (key, val) in symbol_mappings.into_iter().take(1) {
                *func_2_mem.entry(key).or_default() += *val;
            }
        }
        info!("Results");
        let mut func_2_mem: Vec<_> = func_2_mem.iter().collect();
        func_2_mem.sort_by(|x, y| x.1.size.partial_cmp(&y.1.size).unwrap());
        for (func, counter) in func_2_mem.iter().filter(|c| c.1.size >= MIB) {
            info!(?func, count = counter.cnt, size_mb = counter.size / MIB);
        }

        let mapped_file_pages: usize = compute_present_pages(&smaps, &mut file, page_size, true)
            .with_context(|| "compute_present_pages")?
            .iter()
            .map(|x| x.1.len())
            .sum();
        let mapped_files_mb = mapped_file_pages * (page_size as usize) / MIB;
        let resident_but_not_used_mb = ((total_present_pages * (page_size as usize))
            .saturating_sub(present_allocated_with_proxy))
            / MIB;
        let total_size_mb =
            resident_but_not_used_mb + present_allocated_with_proxy / MIB + mapped_files_mb;
        info!(took = ?start.elapsed(), total_size_mb);

        info!(took = ?start.elapsed(), resident_but_not_used_mb, allocated_with_proxy_mb = present_allocated_with_proxy / MIB, mapped_files_mb);
        Ok(())
    }

    fn get_mmaped_exe_regions(smaps: &[Smap], exe_path: PathBuf) -> Vec<Smap> {
        let mut mmaped_exec = Vec::new();
        for smap in smaps.iter().filter(|x| x.mapped_file.is_some()) {
            if let Some(x) = &smap.mapped_file {
                if x.as_str() == exe_path.to_str().unwrap() {
                    info!(?smap);
                    mmaped_exec.push(smap.clone());
                }
            }
        }
        mmaped_exec
    }
}
