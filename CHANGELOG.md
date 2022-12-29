# 0.3.0

- Trim down dependencies to no longer use clap derives.
- Changed echo behavior to only disable ECHO in script mode if
  it's not connected to a tty and not at all when it's used
  outside of script mode unless `--no-echo` is given.
- Changed all `--disable-X` to `--no-X` with hidden aliases.

# 0.2.3

- Restore `OPOST` for all modes but script mode.   Remove
  `--output-processing` flag.  This behavior is too buggy in raw
  terminal mode and breaks newline rendering in some cases.

# 0.2.2

- Fix incorrect handling of input stream closing.
- Ensure `ECHO` mode is consistently disabled.
- Turn off `OPOST` by default.
- Added `--output-processing` to enable `OPOST`.

# 0.2.1

- Fixed incorrect truncation behavior.
- Correctly handle `EIO` on linux.
- Make EOF not appear if the process does not send to stderr.
- Fixed incorrect SIGWINCH handling.

# 0.2.0

- Add `--disable-pager` disable pagers.
- Add `--script-mode` to retain stdout/stderr as separate
  streams but without raw mode on the terminal.
- Added `--disable-raw` to explicitly disable raw mode.
- Improved `EINTR` handling.
- Improve compatibility with older rust versions.

# 0.1.0

- Initial release
