use std::env;
use std::ffi::{CString, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::prelude::{OpenOptionsExt, OsStrExt};
use std::path::Path;

use anyhow::Error;
use nix::errno::Errno;
use nix::libc::{
    login_tty, O_NONBLOCK, SIGWINCH, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO, TIOCGWINSZ, VEOF,
};
use nix::pty::{openpty, Winsize};
use nix::sys::select::{select, FdSet};
use nix::sys::signal::{killpg, Signal};
use nix::sys::stat::Mode;
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, LocalFlags, SetArg, Termios};
use nix::sys::time::TimeVal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{
    close, dup2, execvp, fork, mkfifo, pipe, read, tcgetpgrp, write, ForkResult, Pid,
};
use signal_hook::iterator::Signals;

pub struct SpawnOptions<'a> {
    pub args: &'a [OsString],
    pub out_path: Option<&'a Path>,
    pub truncate_out: bool,
    pub no_flush: bool,
    pub script_mode: bool,
    pub disable_pager: bool,
    pub in_path: Option<&'a Path>,
}

/// Spawns a process in a PTY in a manor similar to `script`
/// but with separate stdout/stderr.
///
/// It leaves stdin/stdout/stderr connected but also writes events into the
/// optional `out` log file.  Additionally it can retrieve instructions from
/// the given control socket.
pub fn spawn(opts: &SpawnOptions) -> Result<i32, Error> {
    // if we can't retrieve the terminal atts we're not directly connected
    // to a pty in which case we won't do any of the terminal related
    // operations.
    let term_attrs = tcgetattr(STDIN_FILENO).ok();
    let winsize = term_attrs.as_ref().and_then(|_| get_winsize(STDIN_FILENO));

    // Create the outer pty for stdout
    let pty = openpty(&winsize, &term_attrs)?;

    // This switches the terminal to raw mode and restores it on Drop.  Unfortunately
    // due to all our shenanigans here we have no real guarantee that `Drop` is called
    // so there will be cases where the term is left in raw state and requires a reset :(
    let (_restore_term, stderr_parent, stderr_child) = if opts.script_mode {
        let (r, w) = pipe()?;
        (None, Some(r), Some(w))
    } else {
        (
            term_attrs.as_ref().map(|term_attrs| {
                let mut raw_attrs = term_attrs.clone();
                cfmakeraw(&mut raw_attrs);
                raw_attrs.local_flags.remove(LocalFlags::ECHO);
                tcsetattr(STDIN_FILENO, SetArg::TCSAFLUSH, &raw_attrs).ok();
                RestoreTerm(term_attrs.clone())
            }),
            None,
            None,
        )
    };

    // crate a fifo if stdin is pointed to a non existing file
    if let Some(ref path) = opts.in_path {
        mkfifo_atomic(&path)?;
    }

    // Fork and establish the communication loop in the parent.  This unfortunately
    // has to merge stdout/stderr since the pseudo terminal only has one stream for
    // both.
    if let ForkResult::Parent { child } = unsafe { fork()? } {
        close(pty.slave)?;
        if term_attrs.is_some() {
            sigwinch_passthrough(pty.master)?;
        }
        let mut out_file = match opts.out_path {
            Some(p) => Some(
                File::options()
                    .append(true)
                    .create(true)
                    .truncate(opts.truncate_out)
                    .open(p)?,
            ),
            None => None,
        };
        let mut in_file = match opts.in_path {
            Some(p) => Some(
                File::options()
                    .read(true)
                    .custom_flags(O_NONBLOCK)
                    .open(p)?,
            ),
            None => None,
        };
        return Ok(communication_loop(
            pty.master,
            child,
            term_attrs.is_some(),
            out_file.as_mut(),
            in_file.as_mut(),
            stderr_parent,
            !opts.no_flush,
        )?);
    }

    // set the pagers to `cat` if it's disabled.
    if opts.disable_pager || opts.script_mode {
        env::set_var("PAGER", "cat");
    }

    // If we reach this point we're the child and we want to turn into the
    // target executable after having set up the tty with `login_tty` which
    // rebinds stdin/stdout/stderr to the pty.
    let args = opts
        .args
        .iter()
        .filter_map(|x| CString::new(x.as_os_str().as_bytes()).ok())
        .collect::<Vec<_>>();
    close(pty.master)?;
    unsafe {
        login_tty(pty.slave);
        if let Some(fd) = stderr_child {
            dup2(fd, STDERR_FILENO)?;
        }
    }
    execvp(&args[0], &args)?;
    unreachable!();
}

/// Listens to a SIGWINCH signal in a background thread and forwards it to the pty.
fn sigwinch_passthrough(master: i32) -> Result<(), Errno> {
    // this does not seem to work properly with vim at least.  It's probably that the
    // killpg is going to the wrong process?
    std::thread::spawn(move || {
        for _ in &mut Signals::new(&[SIGWINCH]).unwrap() {
            if let Some(winsize) = get_winsize(STDIN_FILENO) {
                set_winsize(master, winsize).ok();
                if let Ok(pgrp) = tcgetpgrp(master) {
                    killpg(pgrp, Signal::SIGWINCH).ok();
                }
            }
        }
    });
    Ok(())
}

fn communication_loop(
    master: i32,
    child: Pid,
    is_tty: bool,
    mut out_file: Option<&mut File>,
    mut in_file: Option<&mut File>,
    stderr: Option<i32>,
    flush: bool,
) -> Result<i32, Error> {
    let mut buf = [0; 4096];
    let mut read_stdin = true;
    let mut done = false;

    while !done {
        let mut read_fds = FdSet::new();
        let mut timeout = TimeVal::new(1, 0);
        read_fds.insert(master);
        if !read_stdin && is_tty {
            read_stdin = true;
        }
        if read_stdin {
            read_fds.insert(STDIN_FILENO);
        }
        if let Some(ref f) = in_file {
            read_fds.insert(f.as_raw_fd());
        }
        if let Some(fd) = stderr {
            read_fds.insert(fd);
        }
        match select(None, Some(&mut read_fds), None, None, Some(&mut timeout)) {
            Ok(0) | Err(Errno::EINTR | Errno::EAGAIN) => continue,
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        }

        if read_fds.contains(STDIN_FILENO) {
            match read(STDIN_FILENO, &mut buf) {
                Ok(0) => {
                    if let Ok(attrs) = tcgetattr(master) {
                        if attrs.local_flags.contains(LocalFlags::ICANON) {
                            write_all(master, &[attrs.control_chars[VEOF]])?;
                        }
                    }
                    read_stdin = false;
                }
                Ok(n) => {
                    write_all(master, &buf[..n])?;
                }
                Err(Errno::EINTR | Errno::EAGAIN) => {}
                Err(err) => return Err(err.into()),
            };
        }
        if let Some(ref mut f) = in_file {
            if read_fds.contains(f.as_raw_fd()) {
                let n = f.read(&mut buf)?;
                if n > 0 {
                    write_all(master, &buf[..n])?;
                };
            }
        }
        if read_fds.contains(master) {
            match read(master, &mut buf) {
                Ok(0) => {
                    done = true;
                }
                Ok(n) => forward_and_log(STDOUT_FILENO, &mut out_file, &buf, n, flush)?,
                Err(Errno::EAGAIN | Errno::EINTR) => {}
                Err(err) => return Err(err.into()),
            };
        }
        if let Some(fd) = stderr {
            if read_fds.contains(fd) {
                match read(fd, &mut buf) {
                    Ok(0) | Err(_) => {}
                    Ok(n) => {
                        forward_and_log(STDERR_FILENO, &mut out_file, &buf, n, flush)?;
                    }
                }
            }
        }
    }

    let code = match waitpid(child, None)? {
        WaitStatus::Exited(_, status) => status,
        WaitStatus::Signaled(_, signal, _) => 128 + signal as i32,
        _ => 1,
    };
    close(master)?;
    Ok(code)
}

fn forward_and_log(
    fd: i32,
    out_file: &mut Option<&mut File>,
    buf: &[u8],
    n: usize,
    flush: bool,
) -> Result<(), Error> {
    if let Some(logfile) = out_file {
        logfile.write_all(&buf[..n])?;
        if flush {
            logfile.flush()?;
        }
    }
    write_all(fd, &buf[..n])?;
    Ok(())
}

/// If possible, returns the terminal size of the given fd.
fn get_winsize(fd: i32) -> Option<Winsize> {
    nix::ioctl_read_bad!(_get_window_size, TIOCGWINSZ, Winsize);
    let mut size: Winsize = unsafe { std::mem::zeroed() };
    unsafe { _get_window_size(fd, &mut size).ok()? };
    Some(size)
}

/// Sets the winsize
fn set_winsize(fd: i32, mut winsize: Winsize) -> Result<(), Errno> {
    nix::ioctl_write_ptr_bad!(_set_window_size, TIOCGWINSZ, Winsize);
    unsafe { _set_window_size(fd, &mut winsize) }?;
    Ok(())
}

/// Creates a FIFO at the path if the file does not exist yet.
fn mkfifo_atomic(path: &Path) -> Result<(), Errno> {
    match mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR) {
        Ok(()) | Err(Errno::EEXIST) => Ok(()),
        Err(err) => Err(err),
    }
}

fn write_all(fd: i32, mut buf: &[u8]) -> Result<(), Errno> {
    while !buf.is_empty() {
        let n = write(fd, buf)?;
        buf = &buf[n..];
    }
    Ok(())
}

struct RestoreTerm(Termios);

impl Drop for RestoreTerm {
    fn drop(&mut self) {
        tcsetattr(STDIN_FILENO, SetArg::TCSAFLUSH, &self.0).ok();
    }
}
