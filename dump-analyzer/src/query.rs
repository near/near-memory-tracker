use crate::utils::{read_lines, split};
use anyhow::*;
use std::path::PathBuf;
use tracing::*;

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct QueryCmd {
    #[clap(long)]
    pid: usize,
}

impl QueryCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        info!(?self.pid);
        let smaps = read_smaps(self.pid).with_context(|| "read_smaps failed")?;
        for smap in smaps {
            info!(?smap);
        }
        let page_map_file = PathBuf::from("/proc").join(self.pid.to_string()).join("pagemap");
        info!(?page_map_file);
        Ok(())
    }
}

#[derive(Debug)]
#[allow(unused)]
struct Smap {
    from: u64,
    to: u64,
    mapped_file: Option<String>,
    is_stack: bool,
    offset: u64,
}

fn read_smaps(pid: usize) -> Result<Vec<Smap>> {
    let path = PathBuf::from("/proc").join(pid.to_string()).join("smaps");
    info!(?path);
    Ok(read_lines(path)?
        .map(|l| (split(&l, ' '), l))
        .filter(|(s, _)| s.len() >= 3 && s[0].contains('-'))
        .map(|(sp, line)| {
            let addresses = &sp[0];
            let flags = &sp[1];
            let offset = &sp[2];

            let mapped_file = if line.len() > 73 { Some(line[73..].to_string()) } else { None };
            info!(?line);
            info!(?addresses, ?flags, ?offset, ?mapped_file);

            let pair = split(&sp[0], '-');
            let from = u64::from_str_radix(pair[0].as_str(), 16).unwrap_or_default();
            let to = u64::from_str_radix(pair[1].as_str(), 16).unwrap_or_default();
            let offset = u64::from_str_radix(offset.as_str(), 16).unwrap_or_default();
            Smap { from, to, mapped_file, is_stack: false, offset }
        })
        .collect())
}
