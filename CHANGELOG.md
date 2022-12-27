# 0.2.1

- Fixed incorrect truncation behavior.
- Correctly handle `EIO` on linux.
- Make EOF not appear if the process does not send to stderr.

# 0.2.0

- Add `--disable-pager` disable pagers.
- Add `--script-mode` to retain stdout/stderr as separate
  streams but without raw mode on the terminal.
- Added `--disable-raw` to explicitly disable raw mode.
- Improved `EINTR` handling.
- Improve compatibility with older rust versions.

# 0.1.0

- Initial release
