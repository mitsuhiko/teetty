use std::env;
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::Write;
use std::os::unix::prelude::{AsRawFd, OsStrExt};

use nix::errno::Errno;
use nix::libc::{
    login_tty, SIGWINCH, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO, TIOCGWINSZ, TIOCSWINSZ, VEOF,
};
use nix::pty::{openpty, Winsize};
use nix::sys::select::{select, FdSet};
use nix::sys::signal::{killpg, Signal};
use nix::sys::termios::{
    cfmakeraw, tcgetattr, tcsetattr, LocalFlags, OutputFlags, SetArg, Termios,
};
use nix::sys::time::TimeVal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{close, dup2, execvp, fork, isatty, read, tcgetpgrp, write, ForkResult, Pid};
use signal_hook::iterator::Signals;

/// Controls how the process spawns.
pub struct SpawnOptions<'a> {
    pub command: &'a [&'a OsStr],
    pub in_file: Option<File>,
    pub out_file: Option<File>,
    pub script_mode: bool,
    pub no_flush: bool,
    pub no_echo: bool,
    pub no_pager: bool,
    pub no_raw: bool,
}

impl<'a> SpawnOptions<'a> {
    /// Spawns a process in a PTY in a manor similar to `script`
    /// but with separate stdout/stderr.
    ///
    /// It leaves stdin/stdout/stderr connected but also writes events into the
    /// optional `out` log file.  Additionally it can retrieve instructions from
    /// the given control socket.
    pub fn spawn(mut self) -> Result<i32, Errno> {
        // if we can't retrieve the terminal atts we're not directly connected
        // to a pty in which case we won't do any of the terminal related
        // operations.
        let term_attrs = tcgetattr(STDIN_FILENO).ok();
        let winsize = term_attrs.as_ref().and_then(|_| get_winsize(STDIN_FILENO));

        // Create the outer pty for stdout
        let pty = openpty(&winsize, &term_attrs)?;

        // In script mode we set up a secondary pty.  One could also use `pipe()`
        // here but in that case the `isatty()` call on stderr would report that
        // it's not connected to a tty which is what we want to prevent.
        let (_restore_term, stderr_pty) = if self.script_mode {
            let term_attrs = tcgetattr(STDERR_FILENO).ok();
            let winsize = term_attrs.as_ref().and_then(|_| get_winsize(STDERR_FILENO));
            let stderr_pty = openpty(&winsize, &term_attrs)?;
            (None, Some(stderr_pty))

        // If we are not disabling raw, we change to raw mode.  This switches the
        // terminal to raw mode and restores it on Drop.  Unfortunately due to all
        // our shenanigans here we have no real guarantee that `Drop` is called so
        // there will be cases where the term is left in raw state and requires a
        // reset :(
        } else if !self.no_raw {
            (
                term_attrs.as_ref().map(|term_attrs| {
                    let mut raw_attrs = term_attrs.clone();
                    cfmakeraw(&mut raw_attrs);
                    raw_attrs.local_flags.remove(LocalFlags::ECHO);
                    tcsetattr(STDIN_FILENO, SetArg::TCSAFLUSH, &raw_attrs).ok();
                    RestoreTerm(term_attrs.clone())
                }),
                None,
            )

        // at this point we're neither in scrop mode, nor is raw enabled. do nothing
        } else {
            (None, None)
        };

        // set some flags after pty has been created.  There are cases where we
        // want to remove the ECHO flag so we don't see ^D and similar things in
        // the output.  Likewise in script mode we want to remove OPOST which will
        // otherwise convert LF to CRLF.
        if let Ok(mut term_attrs) = tcgetattr(pty.master) {
            if self.script_mode {
                term_attrs.output_flags.remove(OutputFlags::OPOST);
            }
            if self.no_echo || (self.script_mode && !isatty(STDIN_FILENO).unwrap_or(false)) {
                term_attrs.local_flags.remove(LocalFlags::ECHO);
            }
            tcsetattr(pty.master, SetArg::TCSAFLUSH, &term_attrs).ok();
        }

        // Fork and establish the communication loop in the parent.  This unfortunately
        // has to merge stdout/stderr since the pseudo terminal only has one stream for
        // both.
        if let ForkResult::Parent { child } = unsafe { fork()? } {
            close(pty.slave)?;
            if let Some(ref stderr_pty) = stderr_pty {
                close(stderr_pty.slave)?;
            }
            if term_attrs.is_some() {
                sigwinch_passthrough(pty.master, stderr_pty.as_ref().map(|x| x.master))?;
            }
            return Ok(communication_loop(
                pty.master,
                child,
                term_attrs.is_some(),
                self.out_file.as_mut(),
                self.in_file.as_mut(),
                stderr_pty.as_ref().map(|x| x.master),
                !self.no_flush,
            )?);
        }

        // set the pagers to `cat` if it's disabled.
        if self.no_pager || self.script_mode {
            env::set_var("PAGER", "cat");
        }

        // If we reach this point we're the child and we want to turn into the
        // target executable after having set up the tty with `login_tty` which
        // rebinds stdin/stdout/stderr to the pty.
        let args = self
            .command
            .iter()
            .filter_map(|x| CString::new(x.as_bytes()).ok())
            .collect::<Vec<_>>();
        close(pty.master)?;
        if let Some(ref stderr_pty) = stderr_pty {
            close(stderr_pty.master)?;
        }
        unsafe {
            login_tty(pty.slave);
            if let Some(ref stderr_pty) = stderr_pty {
                dup2(stderr_pty.slave, STDERR_FILENO)?;
            }
        }

        // Since this returns Infallible rather than ! due to limitations, we need
        // this dummy match.
        match execvp(&args[0], &args)? {}
    }
}

fn communication_loop(
    master: i32,
    child: Pid,
    is_tty: bool,
    mut out_file: Option<&mut File>,
    mut in_file: Option<&mut File>,
    stderr: Option<i32>,
    flush: bool,
) -> Result<i32, Errno> {
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
                    send_eof_sequence(master);
                    read_stdin = false;
                }
                Ok(n) => {
                    write_all(master, &buf[..n])?;
                }
                Err(Errno::EINTR | Errno::EAGAIN) => {}
                // on linux a closed tty raises EIO
                Err(Errno::EIO) => {
                    done = true;
                }
                Err(err) => return Err(err.into()),
            };
        }
        if let Some(ref mut f) = in_file {
            if read_fds.contains(f.as_raw_fd()) {
                // use read() here so that we can handle EAGAIN/EINTR
                // without this we might recieve resource temporary unavailable
                // see https://github.com/mitsuhiko/teetty/issues/3
                match read(f.as_raw_fd(), &mut buf) {
                    Ok(0) | Err(Errno::EAGAIN | Errno::EINTR) => {}
                    Err(err) => return Err(err.into()),
                    Ok(n) => {
                        write_all(master, &buf[..n])?;
                    }
                }
            }
        }
        if let Some(fd) = stderr {
            if read_fds.contains(fd) {
                match read(fd, &mut buf) {
                    Ok(0) | Err(_) => {}
                    Ok(n) => {
                        forward_and_log(STDERR_FILENO, &mut out_file, &buf[..n], flush)?;
                    }
                }
            }
        }
        if read_fds.contains(master) {
            match read(master, &mut buf) {
                // on linux a closed tty raises EIO
                Ok(0) | Err(Errno::EIO) => {
                    done = true;
                }
                Ok(n) => forward_and_log(STDOUT_FILENO, &mut out_file, &buf[..n], flush)?,
                Err(Errno::EAGAIN | Errno::EINTR) => {}
                Err(err) => return Err(err.into()),
            };
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
    flush: bool,
) -> Result<(), Errno> {
    if let Some(logfile) = out_file {
        logfile.write_all(buf).map_err(|x| match x.raw_os_error() {
            Some(errno) => Errno::from_i32(errno),
            None => Errno::EINVAL,
        })?;
        if flush {
            logfile.flush().ok();
        }
    }
    write_all(fd, buf)?;
    Ok(())
}

/// Listens to a SIGWINCH signal in a background thread and forwards it to the pty.
fn sigwinch_passthrough(master: i32, stderr_master: Option<i32>) -> Result<(), Errno> {
    // this does not seem to work properly with vim at least.  It's probably that the
    // killpg is going to the wrong process?
    std::thread::spawn(move || {
        for _ in &mut Signals::new(&[SIGWINCH]).unwrap() {
            if let Some(winsize) = get_winsize(STDIN_FILENO) {
                set_winsize(master, winsize).ok();
                if let Some(second_master) = stderr_master {
                    set_winsize(second_master, winsize).ok();
                }
                if let Ok(pgrp) = tcgetpgrp(master) {
                    killpg(pgrp, Signal::SIGWINCH).ok();
                }
            }
        }
    });
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
fn set_winsize(fd: i32, winsize: Winsize) -> Result<(), Errno> {
    nix::ioctl_write_ptr_bad!(_set_window_size, TIOCSWINSZ, Winsize);
    unsafe { _set_window_size(fd, &winsize) }?;
    Ok(())
}

/// Sends an EOF signal to the terminal if it's in canonical mode.
fn send_eof_sequence(fd: i32) {
    if let Ok(attrs) = tcgetattr(fd) {
        if attrs.local_flags.contains(LocalFlags::ICANON) {
            write(fd, &[attrs.control_chars[VEOF]]).ok();
        }
    }
}

/// Calls write in a loop until it's done.
fn write_all(fd: i32, mut buf: &[u8]) -> Result<(), Errno> {
    while !buf.is_empty() {
        // we generally assume that EINTR/EAGAIN can't happen on write()
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
