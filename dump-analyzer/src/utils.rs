use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;

pub fn read_lines(file_name: impl AsRef<Path>) -> anyhow::Result<impl Iterator<Item = String>> {
    let file = File::open(file_name)?;
    Ok(io::BufReader::new(file).lines().filter_map(|x| x.ok()))
}
