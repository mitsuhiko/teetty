use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::exit;

use anyhow::Error;
use clap::{Arg, ArgAction, Command};

use nix::errno::Errno;
use nix::libc::O_NONBLOCK;
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use tty_spawn::SpawnOptions;

fn execute() -> Result<i32, Error> {
    let matches = make_app().get_matches();

    if matches.get_flag("version") {
        eprintln!("teetty {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    let command = matches
        .get_many::<OsString>("command")
        .unwrap()
        .map(|x| x.as_os_str())
        .collect::<Vec<_>>();
    let in_file = match matches.get_one::<PathBuf>("in_path") {
        Some(p) => {
            mkfifo_atomic(&p)?;
            Some(
                File::options()
                    .read(true)
                    .custom_flags(O_NONBLOCK)
                    .open(p)?,
            )
        }
        None => None,
    };
    let out_file = match matches.get_one::<PathBuf>("out_path") {
        Some(p) => Some(if !matches.get_flag("truncate_out") {
            File::options().append(true).create(true).open(p)?
        } else {
            File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(p)?
        }),
        None => None,
    };

    Ok(SpawnOptions {
        command: &command[..],
        in_file,
        out_file,
        script_mode: matches.get_flag("script_mode"),
        no_flush: matches.get_flag("no_flush"),
        no_echo: matches.get_flag("no_echo"),
        no_pager: matches.get_flag("no_pager"),
        no_raw: matches.get_flag("no_raw"),
    }
    .spawn()?)
}

fn make_app() -> Command {
    Command::new("teetty")
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
            Arg::new("no_flush")
                .help("Disables the default output flushing after all writes")
                .short('F')
                .long("no-flush")
                .aliases(["disable-flush"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_echo")
                .help("Disables echoing of inputs")
                .short('E')
                .long("no-echo")
                .aliases(["disable-echo"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_pager")
                .help("Tries to tell a process to not use a pager like `LESS`")
                .short('P')
                .long("no-pager")
                .aliases(["disable-pager"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_raw")
                .help(
                    "Disables raw terminal mode.  Depending on the application being \
                    proxied you might want to enable this.  In script mode, raw mode \
                    is automatically disabled",
                )
                .short('R')
                .long("no-raw")
                .aliases(["disable-raw"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("version")
                .help("Prints version info")
                .long("version")
                .action(ArgAction::SetTrue),
        )
}

/// Creates a FIFO at the path if the file does not exist yet.
fn mkfifo_atomic(path: &Path) -> Result<(), Errno> {
    match mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR) {
        Ok(()) | Err(Errno::EEXIST) => Ok(()),
        Err(err) => Err(err),
    }
}

fn main() {
    exit(match execute() {
        Ok(code) => code,
        Err(err) => {
            writeln!(std::io::stderr(), "teetty: {}", err).ok();
            1
        }
    })
}
