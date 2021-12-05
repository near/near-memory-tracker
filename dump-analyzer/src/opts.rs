use std::io::Empty;
use clap::{AppSettings, Clap};
use anyhow::Result;

#[derive(Clap, Debug)]
#[clap(version = "0.1")]
#[clap(setting = AppSettings::SubcommandRequiredElseHelp)]
pub(crate) struct Opts {
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

#[derive(Clap, Debug)]
pub(super) enum SubCommand {
    #[clap(name = "refactor_deepsize")]
    Empty(EmptyCmd),
}

#[derive(Clap, Debug)]
pub(crate) struct EmptyCmd {}

impl EmptyCmd {
    fn handle(&self) -> Result<()> {
        println!("Hello World");
        Ok(())
    }
}
