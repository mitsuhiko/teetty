//! `tty-spawn` is the underlying library on which
//! [`teetty`](https://github.com/mitsuhiko/teetty) is built.  It lets you spawn
//! processes in a fake TTY and duplex stdin/stdout so you can communicate with an
//! otherwise user attended process.
use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::prelude::{AsRawFd, OpenOptionsExt, OsStrExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, io};

use nix::errno::Errno;
use nix::libc::{login_tty, O_NONBLOCK, TIOCGWINSZ, TIOCSWINSZ, VEOF};
use nix::pty::{openpty, Winsize};
use nix::sys::select::{select, FdSet};
use nix::sys::signal::{killpg, Signal};
use nix::sys::stat::Mode;
use nix::sys::termios::{
    cfmakeraw, tcgetattr, tcsetattr, LocalFlags, OutputFlags, SetArg, Termios,
};
use nix::sys::time::TimeVal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{dup2, execvp, fork, isatty, mkfifo, read, tcgetpgrp, write, ForkResult, Pid};
use signal_hook::consts::SIGWINCH;

/// Lets you spawn processes with a TTY connected.
pub struct TtySpawn {
    options: Option<SpawnOptions>,
}

impl TtySpawn {
    /// Creates a new [`TtySpawn`] for a given command.
    pub fn new<S: AsRef<OsStr>>(cmd: S) -> TtySpawn {
        TtySpawn {
            options: Some(SpawnOptions {
                command: vec![cmd.as_ref().to_os_string()],
                stdin_file: None,
                stdout_file: None,
                script_mode: false,
                no_flush: false,
                no_echo: false,
                no_pager: false,
                no_raw: false,
            }),
        }
    }

    /// Alternative way to construct a [`TtySpawn`].
    ///
    /// Takes an iterator of command and arguments.  If the iterator is empty this
    /// panicks.
    ///
    /// # Panicks
    ///
    /// If the iterator is empty, this panics.
    pub fn new_cmdline<S: AsRef<OsStr>, I: Iterator<Item = S>>(mut cmdline: I) -> Self {
        let mut rv = TtySpawn::new(cmdline.next().expect("empty cmdline"));
        rv.args(cmdline);
        rv
    }

    /// Adds a new argument to the command.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.options_mut().command.push(arg.as_ref().to_os_string());
        self
    }

    /// Adds multiple arguments from an iterator.
    pub fn args<S: AsRef<OsStr>, I: Iterator<Item = S>>(&mut self, args: I) -> &mut Self {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Sets an input file for stdin.
    ///
    /// It's recommended that this is a named pipe and as a general recommendation
    /// this file should be opened with `O_NONBLOCK`.
    ///
    /// # Platform Specifics
    ///
    /// While we will never write into the file it's strongly recommended to
    /// ensure that the file is opened writable too.  The reason for this is that
    /// on Linux, if the last writer (temporarily) disconnects from a FIFO polling
    /// primitives such as the one used by `tty-spawn` will keep reporting that the
    /// file is ready while there not actually being any more data coming in.  The
    /// solution to this problem is to ensure that there is at least always one
    /// writer open which can be ensured by also opening this file for writing.
    pub fn stdin_file(&mut self, f: File) -> &mut Self {
        self.options_mut().stdin_file = Some(f);
        self
    }

    /// Sets a path as input file for stdin.
    pub fn stdin_path<P: AsRef<Path>>(&mut self, path: P) -> Result<&mut Self, io::Error> {
        let path = path.as_ref();
        mkfifo_atomic(&path)?;
        // for the justification for write(true) see the explanation on
        // [`stdin_file`](Self::stdin_file).
        Ok(self.stdin_file(
            File::options()
                .read(true)
                .write(true)
                .custom_flags(O_NONBLOCK)
                .open(path)?,
        ))
    }

    /// Sets an output file for stdout.
    pub fn stdout_file(&mut self, f: File) -> &mut Self {
        self.options_mut().stdout_file = Some(f);
        self
    }

    /// Sets a path as output file for stdout.
    ///
    /// If the `truncate` flag is set to `true` the file will be truncated
    /// first, otherwise it will be appended to.
    pub fn stdout_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        truncate: bool,
    ) -> Result<&mut Self, io::Error> {
        Ok(self.stdout_file(if !truncate {
            File::options().append(true).create(true).open(path)?
        } else {
            File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)?
        }))
    }

    /// Enables script mode.
    ///
    /// In script mode stdout/stderr are retained as separate streams, the terminal is
    /// not opened in raw mode.  Additionally some output processing is disabled so
    /// usually you will find LF retained and not converted to CLRF.  This will also
    /// attempt to disable pagers and turn off ECHO intelligently in some cases.
    pub fn script_mode(&mut self, yes: bool) -> &mut Self {
        self.options_mut().script_mode = yes;
        self
    }

    /// Can be used to turn flushing off.
    ///
    /// By default output is flushed constantly.
    pub fn flush(&mut self, yes: bool) -> &mut Self {
        self.options_mut().no_flush = !yes;
        self
    }

    /// Can be used to turn echo off.
    ///
    /// By default echo is turned on.
    pub fn echo(&mut self, yes: bool) -> &mut Self {
        self.options_mut().no_echo = !yes;
        self
    }

    /// Tries to use `cat` as pager.
    ///
    /// When this is enabled then processes are instructed to use `cat` as pager.
    /// This is useful when raw terminals are disabled in which case most pagers
    /// will break.
    pub fn pager(&mut self, yes: bool) -> &mut Self {
        self.options_mut().no_pager = !yes;
        self
    }

    /// Can be used to turn raw terminal mode off.
    ///
    /// By default the terminal is in raw mode but in some cases you might want to
    /// turn this off.  If raw mode is disabled then pagers will not work and so
    /// will most input operations.
    pub fn raw(&mut self, yes: bool) -> &mut Self {
        self.options_mut().no_raw = !yes;
        self
    }

    /// Spawns the application in the TTY.
    pub fn spawn(&mut self) -> Result<i32, io::Error> {
        Ok(spawn(
            self.options.take().expect("builder only works once"),
        )?)
    }

    fn options_mut(&mut self) -> &mut SpawnOptions {
        self.options.as_mut().expect("builder only works once")
    }
}

struct SpawnOptions {
    command: Vec<OsString>,
    stdin_file: Option<File>,
    stdout_file: Option<File>,
    script_mode: bool,
    no_flush: bool,
    no_echo: bool,
    no_pager: bool,
    no_raw: bool,
}

/// Spawns a process in a PTY in a manor similar to `script`
/// but with separate stdout/stderr.
///
/// It leaves stdin/stdout/stderr connected but also writes events into the
/// optional `out` log file.  Additionally it can retrieve instructions from
/// the given control socket.
fn spawn(mut opts: SpawnOptions) -> Result<i32, Errno> {
    // if we can't retrieve the terminal atts we're not directly connected
    // to a pty in which case we won't do any of the terminal related
    // operations.
    let term_attrs = tcgetattr(io::stdin()).ok();
    let winsize = term_attrs
        .as_ref()
        .and_then(|_| get_winsize(io::stdin().as_fd()));

    // Create the outer pty for stdout
    let pty = openpty(&winsize, &term_attrs)?;

    // In script mode we set up a secondary pty.  One could also use `pipe()`
    // here but in that case the `isatty()` call on stderr would report that
    // it's not connected to a tty which is what we want to prevent.
    let (_restore_term, stderr_pty) = if opts.script_mode {
        let term_attrs = tcgetattr(io::stderr()).ok();
        let winsize = term_attrs
            .as_ref()
            .and_then(|_| get_winsize(io::stderr().as_fd()));
        let stderr_pty = openpty(&winsize, &term_attrs)?;
        (None, Some(stderr_pty))

    // If we are not disabling raw, we change to raw mode.  This switches the
    // terminal to raw mode and restores it on Drop.  Unfortunately due to all
    // our shenanigans here we have no real guarantee that `Drop` is called so
    // there will be cases where the term is left in raw state and requires a
    // reset :(
    } else if !opts.no_raw {
        (
            term_attrs.as_ref().map(|term_attrs| {
                let mut raw_attrs = term_attrs.clone();
                cfmakeraw(&mut raw_attrs);
                raw_attrs.local_flags.remove(LocalFlags::ECHO);
                tcsetattr(io::stdin(), SetArg::TCSAFLUSH, &raw_attrs).ok();
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
    if let Ok(mut term_attrs) = tcgetattr(&pty.master) {
        if opts.script_mode {
            term_attrs.output_flags.remove(OutputFlags::OPOST);
        }
        if opts.no_echo || (opts.script_mode && !isatty(io::stdin().as_raw_fd()).unwrap_or(false)) {
            term_attrs.local_flags.remove(LocalFlags::ECHO);
        }
        tcsetattr(&pty.master, SetArg::TCSAFLUSH, &term_attrs).ok();
    }

    // Fork and establish the communication loop in the parent.  This unfortunately
    // has to merge stdout/stderr since the pseudo terminal only has one stream for
    // both.
    if let ForkResult::Parent { child } = unsafe { fork()? } {
        return Ok(communication_loop(
            pty.master,
            child,
            term_attrs.is_some(),
            opts.stdout_file.as_mut(),
            opts.stdin_file.as_mut(),
            stderr_pty.map(|x| x.master),
            !opts.no_flush,
        )?);
    }

    // set the pagers to `cat` if it's disabled.
    if opts.no_pager || opts.script_mode {
        unsafe {
            env::set_var("PAGER", "cat");
        }
    }

    // If we reach this point we're the child and we want to turn into the
    // target executable after having set up the tty with `login_tty` which
    // rebinds stdin/stdout/stderr to the pty.
    let args = opts
        .command
        .iter()
        .filter_map(|x| CString::new(x.as_bytes()).ok())
        .collect::<Vec<_>>();
    unsafe {
        login_tty(pty.slave.as_raw_fd());
        if let Some(ref stderr_pty) = stderr_pty {
            dup2(stderr_pty.slave.as_raw_fd(), io::stderr().as_raw_fd())?;
        }
    }

    // Since this returns Infallible rather than ! due to limitations, we need
    // this dummy match.
    match execvp(&args[0], &args)? {}
}

fn communication_loop(
    master: OwnedFd,
    child: Pid,
    is_tty: bool,
    mut out_file: Option<&mut File>,
    in_file: Option<&mut File>,
    stderr: Option<OwnedFd>,
    flush: bool,
) -> Result<i32, Errno> {
    let mut buf = [0; 4096];
    let mut read_stdin = true;
    let mut done = false;
    let stdin = io::stdin();

    let got_winch = Arc::new(AtomicBool::new(false));
    if is_tty {
        signal_hook::flag::register(SIGWINCH, Arc::clone(&got_winch)).ok();
    }

    while !done {
        if got_winch.load(Ordering::Relaxed) {
            forward_winsize(master.as_fd(), stderr.as_ref().map(|x| x.as_fd()))?;
            got_winch.store(false, Ordering::Relaxed);
        }

        let mut read_fds = FdSet::new();
        let mut timeout = TimeVal::new(1, 0);
        read_fds.insert(master.as_fd());
        if !read_stdin && is_tty {
            read_stdin = true;
        }
        if read_stdin {
            read_fds.insert(stdin.as_fd());
        }
        if let Some(ref f) = in_file {
            read_fds.insert(f.as_fd());
        }
        if let Some(ref fd) = stderr {
            read_fds.insert(fd.as_fd());
        }
        match select(None, Some(&mut read_fds), None, None, Some(&mut timeout)) {
            Ok(0) | Err(Errno::EINTR | Errno::EAGAIN) => continue,
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        }

        if read_fds.contains(stdin.as_fd()) {
            match read(stdin.as_raw_fd(), &mut buf) {
                Ok(0) => {
                    send_eof_sequence(master.as_fd());
                    read_stdin = false;
                }
                Ok(n) => {
                    write_all(master.as_fd(), &buf[..n])?;
                }
                Err(Errno::EINTR | Errno::EAGAIN) => {}
                // on linux a closed tty raises EIO
                Err(Errno::EIO) => {
                    done = true;
                }
                Err(err) => return Err(err.into()),
            };
        }
        if let Some(ref f) = in_file {
            if read_fds.contains(f.as_fd()) {
                // use read() here so that we can handle EAGAIN/EINTR
                // without this we might recieve resource temporary unavailable
                // see https://github.com/mitsuhiko/teetty/issues/3
                match read(f.as_raw_fd(), &mut buf) {
                    Ok(0) | Err(Errno::EAGAIN | Errno::EINTR) => {}
                    Err(err) => return Err(err.into()),
                    Ok(n) => {
                        write_all(master.as_fd(), &buf[..n])?;
                    }
                }
            }
        }
        if let Some(ref fd) = stderr {
            if read_fds.contains(fd.as_fd()) {
                match read(fd.as_raw_fd(), &mut buf) {
                    Ok(0) | Err(_) => {}
                    Ok(n) => {
                        forward_and_log(io::stderr().as_fd(), &mut out_file, &buf[..n], flush)?;
                    }
                }
            }
        }
        if read_fds.contains(master.as_fd()) {
            match read(master.as_raw_fd(), &mut buf) {
                // on linux a closed tty raises EIO
                Ok(0) | Err(Errno::EIO) => {
                    done = true;
                }
                Ok(n) => forward_and_log(io::stdout().as_fd(), &mut out_file, &buf[..n], flush)?,
                Err(Errno::EAGAIN | Errno::EINTR) => {}
                Err(err) => return Err(err.into()),
            };
        }
    }

    Ok(match waitpid(child, None)? {
        WaitStatus::Exited(_, status) => status,
        WaitStatus::Signaled(_, signal, _) => 128 + signal as i32,
        _ => 1,
    })
}

fn forward_and_log(
    fd: BorrowedFd,
    out_file: &mut Option<&mut File>,
    buf: &[u8],
    flush: bool,
) -> Result<(), Errno> {
    if let Some(logfile) = out_file {
        logfile.write_all(buf).map_err(|x| match x.raw_os_error() {
            Some(errno) => Errno::from_raw(errno),
            None => Errno::EINVAL,
        })?;
        if flush {
            logfile.flush().ok();
        }
    }
    write_all(fd, buf)?;
    Ok(())
}

/// Forwards the winsize and emits SIGWINCH
fn forward_winsize(master: BorrowedFd, stderr_master: Option<BorrowedFd>) -> Result<(), Errno> {
    if let Some(winsize) = get_winsize(io::stdin().as_fd()) {
        set_winsize(master, winsize).ok();
        if let Some(second_master) = stderr_master {
            set_winsize(second_master, winsize).ok();
        }
        if let Ok(pgrp) = tcgetpgrp(master) {
            killpg(pgrp, Signal::SIGWINCH).ok();
        }
    }
    Ok(())
}

/// If possible, returns the terminal size of the given fd.
fn get_winsize(fd: BorrowedFd) -> Option<Winsize> {
    nix::ioctl_read_bad!(_get_window_size, TIOCGWINSZ, Winsize);
    let mut size: Winsize = unsafe { std::mem::zeroed() };
    unsafe { _get_window_size(fd.as_raw_fd(), &mut size).ok()? };
    Some(size)
}

/// Sets the winsize
fn set_winsize(fd: BorrowedFd, winsize: Winsize) -> Result<(), Errno> {
    nix::ioctl_write_ptr_bad!(_set_window_size, TIOCSWINSZ, Winsize);
    unsafe { _set_window_size(fd.as_raw_fd(), &winsize) }?;
    Ok(())
}

/// Sends an EOF signal to the terminal if it's in canonical mode.
fn send_eof_sequence(fd: BorrowedFd) {
    if let Ok(attrs) = tcgetattr(fd) {
        if attrs.local_flags.contains(LocalFlags::ICANON) {
            write(fd, &[attrs.control_chars[VEOF]]).ok();
        }
    }
}

/// Calls write in a loop until it's done.
fn write_all(fd: BorrowedFd, mut buf: &[u8]) -> Result<(), Errno> {
    while !buf.is_empty() {
        // we generally assume that EINTR/EAGAIN can't happen on write()
        let n = write(fd, buf)?;
        buf = &buf[n..];
    }
    Ok(())
}

/// Creates a FIFO at the path if the file does not exist yet.
fn mkfifo_atomic(path: &Path) -> Result<(), Errno> {
    match mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR) {
        Ok(()) | Err(Errno::EEXIST) => Ok(()),
        Err(err) => Err(err),
    }
}

struct RestoreTerm(Termios);

impl Drop for RestoreTerm {
    fn drop(&mut self) {
        tcsetattr(io::stdin(), SetArg::TCSAFLUSH, &self.0).ok();
    }
}
