use crate::analyze::AnalyzeCmd;
use crate::mem_used::MemUsedCmd;
use crate::symbols::SymbolsCmd;
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
    MemUsed(MemUsedCmd),
    Analyze(AnalyzeCmd),
    Symbols(SymbolsCmd),
}
