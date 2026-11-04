#!/usr/bin/env bash
# Linux counterpart of verify-exe.ps1 for the app binary: ELF machine, size budget, version and the
# headless --self-test. The AppImage itself is smoke-tested later (smoke-appimage.sh), because it
# does not exist yet at this point.
#
# Usage: verify-elf.sh <binary> <x64|arm64> [expected-version]
set -euo pipefail

bin=$1
arch=$2
want_version=${3:-}
# Provisional, generous budget (nothing is measured yet): re-baseline from the first Linux build to
# round_up(measured * 1.10, 0.5 MiB), as verify-exe.ps1 does for Windows.
max_bytes=$((40 * 1024 * 1024))
# The AppImage should run on Ubuntu 22.04 (glibc 2.35) and later.
max_glibc=2.35

case "$arch" in
  x64) machine=3e ;;     # EM_X86_64 (62)
  arm64) machine=b7 ;;   # EM_AARCH64 (183)
  *) echo "::error::unknown arch '$arch'"; exit 1 ;;
esac

[ "$(head -c 4 "$bin" | od -An -c | tr -d ' ')" = '177ELF' ] || { echo "::error::$bin is not an ELF file"; exit 1; }
# e_machine is the little-endian u16 at offset 18.
got=$(od -An -tx1 -j18 -N2 "$bin" | tr -d ' \n')
[ "$got" = "${machine}00" ] || { echo "::error::ELF machine 0x$got, expected 0x${machine}00 ($arch)"; exit 1; }

size=$(stat -c %s "$bin")
line="pr-monitor ($arch, linux): $size bytes = $((size / 1048576)) MiB (budget $((max_bytes / 1048576)) MiB)"
echo "$line"
[ -z "${GITHUB_STEP_SUMMARY:-}" ] || echo "$line" >> "$GITHUB_STEP_SUMMARY"
[ "$size" -le "$max_bytes" ] || { echo "::error::binary exceeds size budget"; exit 1; }

symbols=$(objdump -T "$bin")
needed=$(grep -o 'GLIBC_[0-9][0-9.]*' <<<"$symbols" | sed 's/^GLIBC_//' | sort -V | tail -n 1 || true)
echo "highest glibc symbol version needed: ${needed:-none} (floor $max_glibc)"
if [ -n "$needed" ] && [ "$(printf '%s\n%s\n' "$needed" "$max_glibc" | sort -V | tail -n 1)" != "$max_glibc" ]; then
  echo "::error::needs glibc $needed, above the $max_glibc floor promised in the README"; exit 1
fi

if [ -n "$want_version" ]; then
  out=$("$bin" --version)
  echo "$out"
  [[ "$out" == *"$want_version"* ]] || { echo "::error::--version '$out' does not mention $want_version"; exit 1; }
fi

"$bin" --self-test
