use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Error;
use clap::Parser;

use crate::spawn::{spawn, SpawnOptions};

/// teetty is a wrapper binary to execute a command in a pty with remote control
/// facilities.
#[derive(Debug, Parser)]
#[command(about, arg_required_else_help = true, max_term_width = 92)]
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
    #[arg(short = 's', long)]
    script_mode: bool,
    /// Tries to tell a process to not use a pager like `LESS`.
    #[arg(long, short = 'P')]
    disable_pager: bool,
    /// Disables raw terminal mode.  Depending on the application being proxied
    /// you might want to enable this.  In script mode, raw mode is automatically
    /// disabled.
    #[arg(short = 'R', long)]
    disable_raw: bool,
    /// Enables implementation defined output processing (OPOST)
    #[arg(long)]
    output_processing: bool,
    /// The command and the arguments to run
    #[arg(last = true)]
    command: Vec<OsString>,
    /// Prints version info
    #[arg(long)]
    version: bool,
}

pub fn execute() -> Result<i32, Error> {
    let args = Cli::parse();
    if args.version {
        eprintln!("teetty {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    spawn(&SpawnOptions {
        args: &args.command[..],
        out_path: args.out_path.as_deref(),
        truncate_out: args.truncate_out,
        no_flush: args.no_flush,
        script_mode: args.script_mode,
        disable_pager: args.disable_pager,
        disable_raw: args.disable_raw,
        output_processing: args.output_processing,
        in_path: args.in_path.as_deref(),
    })
}
