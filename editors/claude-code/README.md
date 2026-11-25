# jtk for Claude Code

[`jtk-hook.sh`](jtk-hook.sh) runs `jtk` on the Java files Claude changes. jtk applies its safe fixes and
formats the files; the violations it cannot fix go back to Claude (exit code 2), which resolves them in the same
task instead of leaving them to a later clean-up.

- **After each edit** (PostToolUse on `Edit`, `MultiEdit`, `Write`): the edited file.
- **Before Claude finishes** (Stop): all changed and new Java files of the Git working tree, which also covers files
  changed through shell commands. If Claude stops again after a blocked stop, the hook lets it finish, so that a
  violation it cannot resolve does not loop; CI still reports it.

The nearest `jtk.toml` applies, including `unfixable` and `extend-safe-fixes`. Needs `jtk` and `jq` on the `PATH`.

## Setup

Copy `jtk-hook.sh` to `.claude/hooks/` in the project and add the hooks to `.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|MultiEdit|Write",
        "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/jtk-hook.sh edit" }]
      }
    ],
    "Stop": [
      {
        "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/jtk-hook.sh stop" }]
      }
    ]
  }
}
```

Files with existing violations block Claude as soon as it touches them. Before enabling the hooks in an existing
project, run `jtk` once on the whole project and resolve or suppress what remains.
