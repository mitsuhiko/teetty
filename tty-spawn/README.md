# tty-spawn

[![Build Status](https://github.com/mitsuhiko/teetty/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/teetty/actions?query=workflow%3ATests)
[![Crates.io](https://img.shields.io/crates/d/teetty.svg)](https://crates.io/crates/teetty)
[![License](https://img.shields.io/github/license/mitsuhiko/teetty)](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)

`tty-spawn` is the underlying library on which
[`teetty`](https://github.com/mitsuhiko/teetty) is built.  It lets you spawn
processes in a fake TTY and duplex stdin/stdout so you can communicate with an
otherwise user attended process.

## License and Links

* [Issue Tracker](https://github.com/mitsuhiko/teetty/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/teetty/blob/main/LICENSE)
