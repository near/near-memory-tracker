use anyhow::Result;
use clap::AppSettings;
use std::io::Empty;

#[derive(clap::Parser, Debug)]
#[clap(version = "0.1")]
#[clap(setting = AppSettings::SubcommandRequiredElseHelp)]
pub(crate) struct Opts {
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

#[derive(clap::Parser, Debug)]
pub(super) enum SubCommand {
    #[clap(name = "refactor_deepsize")]
    Empty(EmptyCmd),
}

#[derive(clap::Parser, Debug)]
pub(crate) struct EmptyCmd {}

impl EmptyCmd {
    fn handle(&self) -> Result<()> {
        println!("Hello World");
        Ok(())
    }
}
