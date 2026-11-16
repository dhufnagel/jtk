#!/usr/bin/env bash
# Measures the CPU time of formatting every file listed in <file-list> (paths relative to <dir>), printing to stdout.
# Usage: scripts/bench.sh <dir> <file-list> [runs]
set -euo pipefail
bin=$(cd "$(dirname "$0")/.." && pwd)/target/release/jtk
dir=$1 list=$2 runs=${3:-3}
cd "$dir"
for _ in $(seq "$runs"); do
    # Pinned to one core so that the numbers are comparable (the tool uses one worker per available core).
    /usr/bin/time -f "%e s wall, %U s user" taskset -c 0 xargs -a "$list" "$bin" > /dev/null
done 2>&1 | sort -t' ' -k4 -n | head -1
