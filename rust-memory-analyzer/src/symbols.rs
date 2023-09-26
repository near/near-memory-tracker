use rustc_demangle::demangle;
use std::process::{Command, Stdio};
use std::usize;
use tracing::info;

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct SymbolsCmd {
    #[clap(long)]
    binary_path: String,
}

#[allow(unused)]
#[derive(Debug)]
pub struct Symbol {
    pub offset: usize,
    pub unk: String,
    pub raw_symbol: String,
    pub symbol: String,
}

impl SymbolsCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        for symbol in get_symbols(&self.binary_path)? {
            info!(symbol = ?symbol);
        }
        Ok(())
    }
}

pub fn get_symbols(binary_path: &str) -> anyhow::Result<Vec<Symbol>> {
    let output = (Command::new("nm").arg("-an").arg(binary_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    Ok(String::from_utf8_lossy(output.stdout.as_slice())
        .split('\n')
        .map(|line| line.split(' ').collect::<Vec<_>>())
        .filter(|s| s.len() >= 3)
        .map(|split| Symbol {
            offset: usize::from_str_radix(split[0], 16).unwrap_or_default(),
            unk: split[1].to_string(),
            raw_symbol: split[2].to_string(),
            symbol: demangle(split[2]).to_string(),
        })
        .collect())
}
