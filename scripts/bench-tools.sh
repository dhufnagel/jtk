#!/usr/bin/env bash
# Measures jtk against the tools it replaces on a source tree: formatting against palantir-java-format, linting
# against Checkstyle and PMD with the rules that have a jtk equivalent (as in compare-with-checkstyle.sh and
# compare-with-pmd.sh, whose configurations it reuses), and jtk check against all three together.
#
# Usage: scripts/bench-tools.sh <palantir-binary> <checkstyle-all.jar> <pmd-bin-dir> <source-dir> [runs] [work-dir]
#
# Prints the best of <runs> (default 3) wall-clock and CPU times per tool. Every tool runs as its users run it, on
# all cores where it can (jtk and PMD in parallel, palantir-java-format's native image and Checkstyle on one thread
# per invocation). Formatting uses --dry-run, so that the files stay unchanged.
set -euo pipefail

palantir=$(realpath "$1")
jar=$(realpath "$2")
pmd_dir=$(cd "$3" && pwd)
src=$(cd "$4" && pwd)
runs=${5:-3}
work=${6:-$(mktemp -d)}
scripts=$(cd "$(dirname "$0")" && pwd)
jtk=$(dirname "$scripts")/target/release/jtk

cargo build --release --quiet --manifest-path "$scripts/../Cargo.toml"
mkdir -p "$work"
find "$src" -name '*.java' | sort > "$work/files.txt"

# The configurations and rule selections of the comparison scripts (their comparison output is not needed).
"$scripts/compare-with-checkstyle.sh" "$jar" "$src" "$work/checkstyle" > /dev/null
"$scripts/compare-with-pmd.sh" "$pmd_dir" "$src" "$work/pmd" > /dev/null
checkstyle_codes=$(sed -n '/^mapping=/,/^'"'"'$/p' "$scripts/compare-with-checkstyle.sh" | awk 'NF > 1 && $1 ~ /^[A-Z][0-9]+$/ { print $1 }' | sort -u | paste -sd,)
pmd_codes=$(sed -n '/^mapping=/,/^'"'"'$/p' "$scripts/compare-with-pmd.sh" | awk 'NF > 1 && $1 ~ /^[A-Z][0-9]+$/ { print $1 }' | sort -u | paste -sd,)

# Runs a command <runs> times and prints the best wall-clock time with the CPU time (user + system) of that run.
measure() {
  local name=$1 best=""
  shift
  for _ in $(seq "$runs"); do
    /usr/bin/time -f '%e %U %S' -o "$work/time.txt" "$@" > /dev/null 2>&1 || true
    read -r wall user system < <(tail -n 1 "$work/time.txt")
    if [ -z "$best" ] || awk -v a="$wall" -v b="${best%% *}" 'BEGIN { exit !(a < b) }'; then
      best="$wall $(awk -v u="$user" -v s="$system" 'BEGIN { printf "%.1f", u + s }')"
    fi
  done
  printf '%-44s %8s s %8s s\n' "$name" "${best%% *}" "${best#* }"
}

files=$(wc -l < "$work/files.txt")
echo "$files files in $src, best of $runs runs, $(nproc) cores"
printf '%-44s %10s %10s\n' tool wall cpu
measure "palantir-java-format (native image)" xargs -a "$work/files.txt" "$palantir" --palantir --dry-run
measure "jtk" xargs -a "$work/files.txt" "$jtk" --dry-run
measure "Checkstyle ($(echo "$checkstyle_codes" | tr ',' '\n' | wc -l) rules)" \
  java -jar "$jar" -c "$work/checkstyle/checkstyle.xml" "$src"
measure "jtk lint (the same rules)" xargs -a "$work/files.txt" "$jtk" lint --select "$checkstyle_codes"
measure "PMD ($(echo "$pmd_codes" | tr ',' '\n' | wc -l) rules)" \
  "$pmd_dir/bin/pmd" check --no-cache --no-progress -R "$work/pmd/ruleset.xml" -d "$src" -f text
measure "jtk lint (the same rules)" xargs -a "$work/files.txt" "$jtk" lint --select "$pmd_codes"
measure "jtk lint (default rules)" xargs -a "$work/files.txt" "$jtk" lint
measure "jtk lint (all rules)" xargs -a "$work/files.txt" "$jtk" lint --select ALL
measure "jtk check (format and all rules)" xargs -a "$work/files.txt" "$jtk" check --select ALL
