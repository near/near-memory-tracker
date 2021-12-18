use clap::AppSettings;

#[derive(clap_derive::Parser, Debug)]
#[clap(version = "0.1")]
#[clap(setting = AppSettings::SubcommandRequiredElseHelp)]
pub(crate) struct Opts {
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

#[derive(clap_derive::Parser, Debug)]
pub(super) enum SubCommand {
    #[clap(name = "refactor_deepsize")]
    Empty(EmptyCmd),
}

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct EmptyCmd {}

impl EmptyCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        println!("Hello World");
        Ok(())
    }
}
