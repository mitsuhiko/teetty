# tty-spawn

[![Crates.io](https://img.shields.io/crates/d/tty-spawn.svg)](https://crates.io/crates/tty-spawn)
[![License](https://img.shields.io/github/license/mitsuhiko/teetty)](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
[![rustc 1.63.0](https://img.shields.io/badge/rust-1.63%2B-orange.svg)](https://img.shields.io/badge/rust-1.63%2B-orange.svg)
[![Documentation](https://docs.rs/tty-spawn/badge.svg)](https://docs.rs/tty-spawn)

`tty-spawn` is the underlying library on which
[`teetty`](https://github.com/mitsuhiko/teetty) is built.  It lets you spawn
processes in a fake TTY and duplex stdin/stdout so you can communicate with an
otherwise user attended process.

## License and Links

* [Documentation](https://docs.rs/tty-spawn/)
* [Issue Tracker](https://github.com/mitsuhiko/teetty/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
