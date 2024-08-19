# teetty

[![Build Status](https://github.com/mitsuhiko/teetty/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/teetty/actions?query=workflow%3ATests)
[![Crates.io](https://img.shields.io/crates/d/teetty.svg)](https://crates.io/crates/teetty)
[![License](https://img.shields.io/github/license/mitsuhiko/teetty)](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
[![rustc 1.63.0](https://img.shields.io/badge/rust-1.63%2B-orange.svg)](https://img.shields.io/badge/rust-1.63%2B-orange.svg)

`teetty` is a wrapper binary to execute a command in a pty while providing remote
control facilities.

This allows logging the stdout of a process to a file, without the output being
any different from if you were not to pass it through `teetty`.  From the
perspective of the program, it's connected to a terminal.  At the same time
however `teetty` multiplexes the output into both terminal and an optional log
file and it also lets you send input remotely into the program while the user's
keyboard is still attached.  The underlying functionality is available in the
[`tty-spawn`](https://github.com/mitsuhiko/teetty/tree/main/tty-spawn) crate.

```bash
$ cargo install teetty
```

![](https://raw.githubusercontent.com/mitsuhiko/teetty/main/assets/demo.gif)

## Example

In one terminal we tell `teetty` to create and connect to a new
[FIFO](https://en.wikipedia.org/wiki/Named_pipe) named `stdin`, write the output
into a file named `stdout` and spawn a python process.

```bash
$ teetty --in ./stdin --out ./stdout -- python
Python 3.8.12 (default, Mar  3 2022, 14:54:16)
[Clang 13.0.0 (clang-1300.0.29.30)] on darwin
Type "help", "copyright", "credits" or "license" for more information.
>>>
```

We can duplex the output by watching the `stdout` file with `tail -f` in another
window:

```bash
$ tail -f ./stdout
Python 3.8.12 (default, Mar  3 2022, 14:54:16)
[Clang 13.0.0 (clang-1300.0.29.30)] on darwin
Type "help", "copyright", "credits" or "license" for more information.
>>>
```

In yet another window we can now remote control this python process and observe
the output both in the original process as well as the window where we have `tail`
running:

```bash
$ echo 'import sys' > ./stdin
$ echo 'print(sys.version_info)' > ./stdin
```

```python
>>> import sys
>>> sys.version_info
sys.version_info(major=3, minor=8, micro=12, releaselevel='final', serial=0)
```

## Script Mode

By default `teetty` opens a pty and connects it to the application.  Due to how
pseudo terminals work stdout and stderr will be merged together.  In this mode
the terminal will be placed in raw mode which means that applications like Vim
that want to move cursors around will function correctly.

`teetty` provides a second mode called "script mode" that can be enabled with
`--script-mode`.  In that mode stdout and stderr stay separated.  This is
accomplished by leaving stdout connected to the pseudo terminal and by
connecting stderr to a secondary internal pty.  Because this is a setup that
execuables are not familiar with it causes all kinds of visual artifacts when
raw mode is enabled.  To combat this, in this mode pagers and raw mode are
automatically disabled.

**Note on stream synchronization:** unfortunately stdout/stderr currently are not
propertly synchronized in script mode.  See [#6](https://github.com/mitsuhiko/teetty/issues/6)
for more information.

## FIFOs, Flushing and Control Characters

It's generally assumped that the `--in` path is a FIFO but it's possible for this
to be pointed to a file just as well.  For the `--out` parameter there is a significant
difference between it being a FIFO or a file.  If it's pointed to a FIFO then the
writes will block immediately until someone starts reading from it (eg with `cat`).
On the other hand if it's pointed to a file, then `tail -f` can be used to read from
it as it happens, but old data will accumulate in the output file.

Out of the box the output is flushed constantly, but this can be disabled by passing
the `--no-flush` flag.

The connected standard input is connected to a terminal.  This means that control
sequences can be sent in via the FIFO.  For instance sending `\x04` to the process
will try to end it:

```bash
echo -n $'\004' > ./stdin
```

## Related Projects

These are some related projects:

- [`faketty`](https://github.com/dtolnay/faketty): emulates two fake ttys to retain
  stdout and stderr.  This is similar to `teetty` in `--script-mode`.
- [`script`](https://man7.org/linux/man-pages/man1/script.1.html): a built-in tool into
  most unices which can capture output of terminals.
- [`tmux`](https://github.com/tmux/tmux): emulates an entire terminal including
  drawing surface and more. Lets you detach and reattach to multiple terminal
  sessions.
- [`expect`](https://linux.die.net/man/1/expect): lets you script interactive command
  line utilities. Variations of this tool exist for programming languages like
  [`pexpect`](https://pypi.org/project/pexpect) for Python.

## License and Links

* [Issue Tracker](https://github.com/mitsuhiko/teetty/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
