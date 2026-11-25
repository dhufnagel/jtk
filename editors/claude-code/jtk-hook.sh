#!/usr/bin/env bash
# Claude Code hook: fixes and formats the Java files Claude changes, and reports the remaining violations back to
# Claude (exit code 2), so that it resolves them in the same task. Needs jtk and jq on the PATH.
#
#   jtk-hook.sh edit   PostToolUse hook for Edit, MultiEdit and Write: checks the edited file.
#   jtk-hook.sh stop   Stop hook: checks all changed and new Java files of the Git working tree, which also covers
#                      files changed through shell commands.
set -euo pipefail

input=$(cat)

# Fixes and formats the files; on violations or syntax errors, prints them for Claude and exits with 2.
check() {
    local out
    if ! out=$(jtk check --fix "$@" 2>&1); then
        printf 'jtk reports problems it cannot fix. Fix them, or suppress a false alarm with // jtk:ignore[CODE] and the reason:\n%s\n' "$out" >&2
        exit 2
    fi
}

case "${1:-}" in
    edit)
        file=$(jq -r '.tool_input.file_path // empty' <<<"$input")
        [[ $file == *.java && -f $file ]] || exit 0
        check "$file"
        ;;
    stop)
        # A second stop after a blocked one lets Claude finish, so that a violation it cannot fix does not loop;
        # CI still reports it.
        [[ $(jq -r '.stop_hook_active // false' <<<"$input") == true ]] && exit 0
        cd "${CLAUDE_PROJECT_DIR:-.}"
        mapfile -t files < <(
            git diff --name-only --diff-filter=d HEAD -- '*.java'
            git ls-files --others --exclude-standard -- '*.java'
        )
        ((${#files[@]})) || exit 0
        check "${files[@]}"
        ;;
    *)
        echo "usage: jtk-hook.sh edit|stop" >&2
        exit 1
        ;;
esac
