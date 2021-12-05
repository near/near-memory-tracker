mod opts;

use crate::opts::{Opts, SubCommand};
use anyhow::Result;
use clap::Clap;

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Empty(empty_cmd) => empty_cmd.handle()?,
    };
    Ok(())
}
