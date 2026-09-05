use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use zen_ipc::{Action, OutputAction};

use crate::utils::version;

#[derive(Parser)]
#[command(author, version = version(), about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_value_name = "SUBCOMMAND")]
#[command(subcommand_help_heading = "Subcommands")]
pub struct Cli {
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub session: bool,
    #[arg(last = true)]
    pub command: Vec<OsString>,

    #[command(subcommand)]
    pub subcommand: Option<Sub>,
}

#[derive(Subcommand)]
pub enum Sub {
    Msg {
        #[command(subcommand)]
        msg: Msg,
        #[arg(short, long)]
        json: bool,
    },
    Validate {
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    Panic,
    Completions { shell: CompletionShell },
}

#[derive(Subcommand)]
pub enum Msg {
    Outputs,
    Workspaces,
    Windows,
    Layers,
    KeyboardLayouts,
    FocusedOutput,
    FocusedWindow,
    PickWindow,
    PickColor,
    Action {
        #[command(subcommand)]
        action: Action,
    },
    Output {
        #[arg()]
        output: String,
        #[command(subcommand)]
        action: OutputAction,
    },
    EventStream,
    Version,
    RequestError,
    OverviewState,
    Casts,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Nushell,
}

impl TryFrom<CompletionShell> for Shell {
    type Error = &'static str;

    fn try_from(shell: CompletionShell) -> Result<Self, Self::Error> {
        match shell {
            CompletionShell::Bash => Ok(Shell::Bash),
            CompletionShell::Elvish => Ok(Shell::Elvish),
            CompletionShell::Fish => Ok(Shell::Fish),
            CompletionShell::PowerShell => Ok(Shell::PowerShell),
            CompletionShell::Zsh => Ok(Shell::Zsh),
            CompletionShell::Nushell => Err("Nushell should be handled separately"),
        }
    }
}
