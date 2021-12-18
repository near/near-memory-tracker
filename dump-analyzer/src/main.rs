mod opts;
mod query;

use crate::opts::{Opts, SubCommand};
use clap::Parser;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).finish().init();
    tracing::info!("init");
    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Query(cmd) => cmd.handle()?,
    };
    Ok(())
}
