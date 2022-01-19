use crate::utils;
use anyhow::Context;
use std::fs::File;
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct MemUsedCmd {
    #[clap(long)]
    pid: i32,
}

impl MemUsedCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        info!(?self.pid);
        let smaps = utils::read_smaps(self.pid).with_context(|| "read_smaps failed")?;

        let mut total_present_pages = 0;
        let start = Instant::now();
        let page_map_file = PathBuf::from("/proc").join(self.pid.to_string()).join("pagemap");
        let mut file = File::open(page_map_file.clone())
            .with_context(|| format!("page_map_file not found file={:?}", page_map_file))?;

        let page_size = utils::get_page_size()?;

        for (smap, addresses) in
            utils::compute_present_pages(&smaps, &mut file, page_size, false)?.iter()
        {
            info!(?smap, len = addresses.len());
            total_present_pages += addresses.len();
        }
        // compute memory used in not mmaped files
        info!(mem_used_mb = total_present_pages * page_size / crate::utils::MIB, total_present_pages, took = ?start.elapsed());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::utils::get_page_size;
    use tracing_subscriber::util::SubscriberInitExt;

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_size() {
        tracing_subscriber::fmt().with_writer(std::io::stderr).finish().init();
        assert_eq!(get_page_size().ok(), Some(4096));
    }
}
