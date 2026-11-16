#!/usr/bin/env bash
# Formats a copy of a Java source tree with the original palantir-java-format and with this port, and lists the
# files whose output differs.
#
# Usage: scripts/compare-with-original.sh <original-binary-or-command> <source-dir> [work-dir]
#   e.g. scripts/compare-with-original.sh ./palantir-java-format-native-2.103.0-nativeImage-linux-glibc_x86-64.bin \
#            /path/to/java/sources
set -euo pipefail

original=$1
src=$(cd "$2" && pwd)
work=${3:-$(mktemp -d)}
port=$(cd "$(dirname "$0")/.." && pwd)/target/release/jtk

cargo build --release --quiet --manifest-path "$(dirname "$0")/../Cargo.toml"
mkdir -p "$work"
rm -rf "$work/original" "$work/port"
cp -r "$src" "$work/original"
cp -r "$src" "$work/port"

(cd "$work/original" && find . -name '*.java' -print0 | xargs -0 -n 50 -P 8 $original --palantir -i 2>"$work/original-errors.txt") || true
(cd "$work/port" && find . -name '*.java' -print0 | xargs -0 -n 50 -P 8 "$port" -i 2>"$work/port-errors.txt") || true

total=$(find "$src" -name '*.java' | wc -l)
different=$(diff -rq "$work/original" "$work/port" | sed "s|^Files $work/original/||; s| and .*||" || true)
echo "files:     $total"
echo "differing: $(printf '%s' "$different" | grep -c . || true)"
printf '%s\n' "$different" | sed '/^$/d'
echo "errors (original): $(grep -c . "$work/original-errors.txt" || true), errors (port): $(grep -c . "$work/port-errors.txt" || true)"
echo "outputs in $work"
