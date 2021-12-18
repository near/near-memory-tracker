use crate::utils::read_lines;
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
        let _ = read_smaps(self.pid).with_context(|| "read_smaps failed")?;
        Ok(())
    }
}

#[allow(unused)]
struct Smap {
    from: u64,
    to: u64,
    mapped_file: String,
    is_stack: bool,
    offset: u64,
}

fn read_smaps(pid: usize) -> anyhow::Result<Vec<Smap>> {
    let path = PathBuf::from("/proc").join(pid.to_string()).join("smaps");
    info!(?path);
    for line in read_lines(path)? {
        info!(?line)
        /*
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

        *
                 */
    }
    bail!("NOT IMPLEMENTED")
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
