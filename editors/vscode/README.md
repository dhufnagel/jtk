# jtk for Visual Studio Code

Formats and lints Java with [jtk](https://github.com/dhufnagel/jtk): the same formatting as
[palantir-java-format](https://github.com/palantir/palantir-java-format), without a JVM, and rules from Checkstyle,
PMD and the Eclipse save actions. The extension runs `jtk lsp` as a language server.

- **Problems:** jtk's rule violations in open Java files, updated while typing.
- **Quick fixes:** the fix of a violation where jtk has one, and **Suppress … on this line**
  (`// jtk:ignore[CODE]`).
- **Fix all jtk violations** (`source.fixAll.jtk`) applies all safe fixes, also on save.
- **Format Document** and **Format Selection**, also on save.

The nearest `jtk.toml` of a file applies: the selected rules, their parameters and the formatting steps (see the
[jtk README](https://github.com/dhufnagel/jtk#configuration)). Changes to it apply right away.

## Setup

Make jtk the formatter for Java, and optionally format and fix on save (in your user or workspace `settings.json`):

```json
"[java]": {
  "editor.defaultFormatter": "dhufnagel.jtk",
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": { "source.fixAll.jtk": "explicit" }
}
```

If another Java extension (such as Red Hat's "Language Support for Java") is installed, VS Code asks which formatter
to use until `editor.defaultFormatter` is set. Both extensions' problems are shown side by side.

## Settings

| Setting | Default | |
|---|---|---|
| `jtk.path` | `""` | Path to the `jtk` executable. Empty: the binary bundled with the extension, else `jtk` on the `PATH`. |
| `jtk.sortImports` | `true` | Sort imports when formatting a whole document. |
| `jtk.removeUnusedImports` | `true` | Remove unused imports when formatting a whole document. |
| `jtk.reflowLongStrings` | `true` | Reflow string literals that exceed the column limit. |
| `jtk.formatJavadoc` | `false` | Reflow javadoc comments. |
| `jtk.unsafeFixes` | `false` | Also apply the unsafe fixes in **Fix all jtk violations**. |

A `jtk.toml` can also switch formatting steps off. Changed settings restart the server (also: **jtk: Restart
Server**). Formatting errors (usually a syntax error in the code being edited) and the server's log are in the "jtk"
output channel (**jtk: Show Output**); files with syntax errors show no jtk problems.

## Development

Requires Node.js and a Rust toolchain.

```sh
cd editors/vscode
npm install
npm test          # tests against the language server of target/release/jtk
npm run package   # builds jtk for this platform and creates jtk-<version>.vsix
```

To try the package, install it with **Extensions: Install from VSIX…** in VS Code, or:

```sh
code --install-extension jtk-0.5.0.vsix
```

The package contains the jtk binary for the platform it was built on only. With VS Code's remote development (SSH,
containers, WSL) the extension runs on the remote machine, so build the package there. Otherwise, set `jtk.path` to
a jtk executable built for your machine.

To debug the extension, open `editors/vscode` in VS Code and run **Debug: Start Debugging** (F5) with the
"Extension Development Host" launch configuration.
