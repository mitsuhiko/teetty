#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..

NEW_VERSION="${1}"

echo "Bumping version: ${NEW_VERSION}"
perl -pi -e "s/^version = \".*?\"/version = \"$NEW_VERSION\"/" tty-spawn/Cargo.toml
perl -pi -e "s/^version = \".*?\"/version = \"$NEW_VERSION\"/" teetty/Cargo.toml
perl -pi -e "s/^(tty-spawn.*?)version = \".*?\"/\$1version = \"=$NEW_VERSION\"/" teetty/Cargo.toml

cargo check
