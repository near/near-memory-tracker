mod opts;
mod query;
mod utils;

use crate::opts::{Opts, SubCommand};
use anyhow::*;
use clap::Parser;
use tracing::*;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).finish().init();
    info!("init");
    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Query(cmd) => cmd.handle()?,
    };
    Ok(())
}
