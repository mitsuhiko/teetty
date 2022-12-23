# teetty

[![Crates.io](https://img.shields.io/crates/d/teetty.svg)](https://crates.io/crates/teetty)
[![License](https://img.shields.io/github/license/mitsuhiko/teetty)](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)

`teetty` is a wrapper binary to execute a command in a pty while providing remote
control facilities.

This allows logging the stdout of a process to a file, without the output being
any different from if you were not to pass it through `teetty`.  From the
perspective of the program, it's connected to a terminal.  At the same time
however `teetty` multiplexes the output into both terminal and an optional log
file and it also lets you send input remotely into the program while the user's
keyboard is still attached.

```bash
$ cargo install teetty
```

![](https://raw.githubusercontent.com/mitsuhiko/teetty/main/assets/demo.gif)

## Example

In one terminal we first we open a [FIFO](https://en.wikipedia.org/wiki/Named_pipe)
named `stdin` into which we can write input, then we tell `teetty` to connect to
it, and write the output into a `stdout` and spawn a python process.

```bash
$ mkfifo ./stdin
$ teetty --in ./stdin --out ./stdout -- python
Python 3.8.12 (default, Mar  3 2022, 14:54:16)
[Clang 13.0.0 (clang-1300.0.29.30)] on darwin
Type "help", "copyright", "credits" or "license" for more information.
>>>
```

We can duplex the output by watching the `stdout` file with `tail -f` in another
window:

```bash
$ tail -f /tmp/hello-out
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

## License and Links

* [Issue Tracker](https://github.com/mitsuhiko/teetty/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
