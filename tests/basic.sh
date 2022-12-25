#!/usr/bin/env bash

set -eu

echo "stdout output"
echo "stderr output" >&2

if [ -t 0 ]; then
    echo "stdin: tty"
else
    echo "stdin: no-tty"
fi

if [ -t 1 ]; then
    echo "stdout: tty"
else
    echo "stdout: no-tty"
fi

if [ -t 2 ]; then
    echo "stderr: tty" >&2
else
    echo "stderr: no-tty" >&2
fi

exit 42
