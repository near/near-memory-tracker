mod analyze;
mod mem_used;
mod opts;
mod symbols;
mod utils;

use crate::opts::SubCommand;
use anyhow::Context;
use clap::Parser;
use tracing::info;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
    let format = tracing_subscriber::fmt::format()
        .with_level(true) // don't include levels in formatted output
        .with_target(true) // don't include targets
        .without_time();

    tracing_subscriber::fmt().event_format(format).with_writer(std::io::stderr).finish().init();
    info!("init");
    let opts = crate::opts::Opts::parse();

    info!(?opts.subcmd);
    match opts.subcmd {
        SubCommand::Analyze(cmd) => cmd.handle().with_context(|| "analyze_cmd failed")?,
        SubCommand::MemUsed(cmd) => cmd.handle().with_context(|| "query_cmd failed")?,
        SubCommand::Symbols(cmd) => cmd.handle().with_context(|| "symbols_cmd failed")?,
    };
    Ok(())
}
