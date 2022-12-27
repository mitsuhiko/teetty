use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Error;
use clap::{Arg, ArgAction, Command};

use crate::spawn::{spawn, SpawnOptions};

pub fn execute() -> Result<i32, Error> {
    let matches = Command::new("teetty")
        .override_usage("teetty [OPTIONS] -- [COMMAND ...]")
        .max_term_width(92)
        .about(
            "teetty is a wrapper binary to execute a command in a pty with \
            remote control facilities.",
        )
        .arg(
            Arg::new("command")
                .help("The command and the arguments to run")
                .num_args(1..)
                .value_name("COMMAND")
                .value_parser(clap::builder::OsStringValueParser::new())
                .required_unless_present_any(["help", "version"])
                .last(true),
        )
        .arg(
            Arg::new("in_path")
                .help(
                    "A path to a FIFO or file.  When provided it's contents are \
                    monitored and sent to the terminal as input",
                )
                .short('i')
                .long("in")
                .value_name("PATH")
                .value_parser(clap::builder::PathBufValueParser::new()),
        )
        .arg(
            Arg::new("out_path")
                .help(
                    "Path to an optional output file.  stdout and stderr are \
                     captured and streamed into this file in addition to the \
                     terminal output",
                )
                .short('o')
                .long("out")
                .value_name("PATH")
                .value_parser(clap::builder::PathBufValueParser::new()),
        )
        .arg(
            Arg::new("truncate_out")
                .help("When this flag is set the output file is truncated first")
                .long("truncate")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_flush")
                .help("isables the default output flushing after all writes")
                .short('F')
                .long("no-flush")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("script_mode")
                .help(
                    "Enables script mode.  Script mode retains the separation of stdout/stderr, \
                    disables raw mode and pagers.  The end result is that for most tools they \
                    still believe to be connected to a terminal, but keyboard input typically \
                    will no longer work.  In this form teetty can be plugged in to places that \
                    do not require interactivity but you still want an executable think it's \
                    connected to a terminal.",
                )
                .short('s')
                .long("script-mode")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("disable_pager")
                .help("ries to tell a process to not use a pager like `LESS`")
                .short('P')
                .long("disable-pager")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("disable_raw")
                .help(
                    "Disables raw terminal mode.  Depending on the application being \
                    proxied you might want to enable this.  In script mode, raw mode \
                    is automatically disabled",
                )
                .short('R')
                .long("disable-raw")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("version")
                .help("Prints version info")
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    if matches.get_flag("version") {
        eprintln!("teetty {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    let args = matches
        .get_many::<OsString>("command")
        .unwrap()
        .map(|x| x.as_os_str())
        .collect::<Vec<_>>();
    spawn(&SpawnOptions {
        args: &args[..],
        in_path: matches.get_one::<PathBuf>("in_path").map(|x| x.as_path()),
        out_path: matches.get_one::<PathBuf>("out_path").map(|x| x.as_path()),
        truncate_out: matches.get_flag("truncate_out"),
        no_flush: matches.get_flag("no_flush"),
        script_mode: matches.get_flag("script_mode"),
        disable_pager: matches.get_flag("disable_pager"),
        disable_raw: matches.get_flag("disable_raw"),
    })
}
