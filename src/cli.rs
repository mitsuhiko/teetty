use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Error;
use clap::Parser;

use crate::spawn::{spawn, SpawnOptions};

/// teetty is a wrapper binary to execute a command in a pty with remote control
/// facilities.
#[derive(Debug, Parser)]
#[command(version, about, arg_required_else_help = true, max_term_width = 92)]
pub struct Cli {
    /// A path to a FIFO or file.  When provided it's contents are monitored and
    /// sent to the terminal as input.
    #[arg(short, long = "in", value_name = "PATH")]
    in_path: Option<PathBuf>,
    /// Path to an optional output file.  stdout and stderr are captured and streamed
    /// into this file in addition to the terminal output.
    #[arg(short, long = "out", value_name = "PATH")]
    out_path: Option<PathBuf>,
    /// When this flag is set the output file is truncated first.
    #[arg(long = "truncate")]
    truncate_out: bool,
    /// Disables the default output flushing after all writes.
    #[arg(short = 'F', long = "no-flush")]
    no_flush: bool,
    /// Enables script mode.  Script mode retains the separation of stdout/stderr,
    /// disables raw mode and pagers.  The end result is that for most tools they
    /// still believe to be connected to a terminal, but keyboard input typically
    /// will no longer work.  In this form teetty can be plugged in to places that
    /// do not require interactivity but you still want an executable think it's
    /// connected to a terminal.
    #[arg(long)]
    script_mode: bool,
    /// Tries to tell a process to not use a pager like `LESS`.
    #[arg(long)]
    disable_pager: bool,
    /// The command and the arguments to run
    #[arg(last = true)]
    command: Vec<OsString>,
}

pub fn execute() -> Result<i32, Error> {
    let args = Cli::parse();
    spawn(&SpawnOptions {
        args: &args.command[..],
        out_path: args.out_path.as_deref(),
        truncate_out: args.truncate_out,
        no_flush: args.no_flush,
        script_mode: args.script_mode,
        disable_pager: args.disable_pager,
        in_path: args.in_path.as_deref(),
    })
}
