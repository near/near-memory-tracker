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
        .map(|l| (split(&l), l))
        .filter(|(s, _)| s.len() >= 3 && s[0].len() == 25)
        .map(|(split, line)| {
            let addresses = &split[0];
            let flags = &split[1];
            let offset = &split[2];

            let mapped_file = if line.len() > 73 { Some(line[73..].to_string()) } else { None };
            info!(?line);
            info!(?addresses, ?flags, ?offset, ?mapped_file);

            let from = 0;
            let to = 0;
            let offset = 0;
            Smap { from, to, mapped_file, is_stack: false, offset }
        })
        .collect())
}

/*
vector<Smap> read_smaps(int pid) {
const int SIZE = 1024;
char str[SIZE];
ostringstream maps_file_name, pagemap_file_name;
maps_file_name << "/proc/" << pid << "/smaps";
ifstream smaps_in(maps_file_name.str().c_str(), ios::in);
vector<Smap> result;

while (smaps_in.getline(str,SIZE)) {
uint64_t a1 = 0, a2 = 0, offset = 0;
char flags[256];

// 7f34d3290000-7f34e3a00000 r--p 00000000 0
if (sscanf( str, "%" PRIx64 "-%" PRIx64 " %s %" PRIX64, &a1, &a2, flags, &offset) >= 2) {
sscanf( str, "%" PRIx64 "-%" PRIx64, &a1, &a2);
string mapped_file;
if (strlen(str) >= 73) {
mapped_file = string(&str[73]);
}
result.push_back((Smap){a1, a2, mapped_file, false, offset});
}
if (result.size() > 0 && strncmp(str, "VmFlags:", strlen("VmFlags:")) == 0) {
stringstream ss(str);
string tmp;
while (ss >> tmp) if (tmp == "gd") result.back().is_stack = true;

}

}
cout<<"read_maps "<<result.size()<<endl;
return result;
}
*/
