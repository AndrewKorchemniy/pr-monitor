#!/usr/bin/env bash
# Runs the packaged AppImage's headless --self-test. Runners have no FUSE, so the AppImage extracts
# itself first (--appimage-extract-and-run) instead of mounting.
#
# Usage: smoke-appimage.sh <file.AppImage>
set -euo pipefail

image=$1
chmod +x "$image"
"$image" --appimage-extract-and-run --self-test
