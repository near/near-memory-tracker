mod opts;

use crate::diff_splitter::handle_diff_splitter;
use crate::opts::{Opts, SubCommand};
use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Empty(empty_cmd) => empty_cmd.handle(),
    }
    Ok(())
}
