use anyhow::Context;
use std::fs::File;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::{io, usize};
use tracing::{debug, info};

pub const MIB: usize = 1 << 20;

pub fn read_lines(file_name: impl AsRef<Path>) -> anyhow::Result<impl Iterator<Item = String>> {
    let file = File::open(file_name)?;
    Ok(io::BufReader::new(file).lines().filter_map(|x| x.ok()))
}

pub fn read_smaps(pid: i32) -> anyhow::Result<Vec<Smap>> {
    let path = PathBuf::from("/proc").join(pid.to_string()).join("smaps");
    info!(?path);
    Ok(read_lines(path.clone())
        .with_context(|| format!("cant open path={:?}", &path))?
        .map(|l| (l.split(' ').map(|s| s.to_string()).collect::<Vec<_>>(), l))
        .filter(|(s, _)| s.len() >= 3 && s[0].contains('-'))
        .map(|(sp, line)| {
            let (addresses, _flags, offset) = (&sp[0], &sp[1], &sp[2]);

            let split = addresses.split_once('-').unwrap();
            Smap {
                from: usize::from_str_radix(split.0, 16).unwrap_or_default(),
                to: usize::from_str_radix(split.1, 16).unwrap_or_default(),
                mapped_file: if line.len() > 73 { Some(line[73..].to_string()) } else { None },
                is_stack: false,
                offset: usize::from_str_radix(offset, 16).unwrap_or_default(),
            }
        })
        .collect())
}

/// 4096 on `x86_64` linux
pub fn get_page_size() -> anyhow::Result<usize> {
    let res = std::process::Command::new("getconf")
        .arg("PAGESIZE")
        .output()
        .expect("failed to execute process");

    Ok(String::from_utf8(res.stdout)?.trim_end().parse()?)
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Counter {
    pub cnt: usize,
    pub size: usize,
}

impl Counter {
    pub fn with_size(size: usize) -> Self {
        Self { cnt: 1, size }
    }
}

impl AddAssign for Counter {
    fn add_assign(&mut self, other: Self) {
        *self = Self { cnt: self.cnt + other.cnt, size: self.size + other.size };
    }
}

#[derive(Debug, Clone)]
pub struct Smap {
    pub from: usize,
    pub to: usize,
    pub mapped_file: Option<String>,
    #[allow(unused)]
    is_stack: bool,
    pub offset: usize,
}

pub fn compute_present_pages(
    smaps: &[Smap],
    file: &mut File,
    page_size: usize,
    mapped: bool,
) -> anyhow::Result<Vec<(Smap, Vec<usize>)>> {
    let mut res = Vec::new();
    for smap in smaps.iter().filter(|s| s.mapped_file.is_some() == mapped) {
        // Verify that all memory regions are divided into pages.
        file.seek(SeekFrom::Start((smap.from / page_size * PAGE_MAP_ENTRY_SIZE) as u64))?;
        let entries = (smap.to - smap.from) / page_size;
        let mut x: Vec<u8> = Vec::new();
        x.resize(entries * PAGE_MAP_ENTRY_SIZE, 0);

        let read = file.read(x.as_mut_slice()).with_context(|| "read_exact")?;
        if read != entries {
            debug!(read, entries, "didn't ready the buffer completely");
        }
        let present_count: Vec<usize> = (0..entries)
            .filter(|i| x[8 * i + 7] & (1 << 7) != 0)
            .map(|i| smap.from + i * page_size)
            .collect();
        res.push((smap.clone(), present_count));
    }
    Ok(res)
}

// https://www.kernel.org/doc/Documentation/vm/pagemap.txt
const PAGE_MAP_ENTRY_SIZE: usize = std::mem::size_of::<u64>();
