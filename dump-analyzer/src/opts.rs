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
    Query(QueryCmd),
}

#[derive(clap_derive::Parser, Debug)]
pub(crate) struct QueryCmd {
    #[clap(long)]
    pid: String,
}

impl QueryCmd {
    pub(crate) fn handle(&self) -> anyhow::Result<()> {
        tracing::info!(?self.pid);
        Ok(())
    }
}
