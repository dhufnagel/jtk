# jtk for Visual Studio Code

Formats Java with [jtk](https://github.com/dhufnagel/jtk): the same output as
[palantir-java-format](https://github.com/palantir/palantir-java-format), without a JVM.

- **Format Document** formats the whole file, removes unused imports, sorts imports and reflows long strings.
- **Format Selection** formats the statements and declarations touching the selection (imports are left alone).
- Works with **format on save**.

## Setup

Make jtk the formatter for Java, and optionally format on save (in your user or workspace `settings.json`):

```json
"[java]": {
  "editor.defaultFormatter": "dhufnagel.jtk",
  "editor.formatOnSave": true
}
```

If another Java extension (such as Red Hat's "Language Support for Java") is installed, VS Code asks which formatter
to use until `editor.defaultFormatter` is set.

## Settings

| Setting | Default | |
|---|---|---|
| `jtk.path` | `""` | Path to the `jtk` executable. Empty: the binary bundled with the extension, else `jtk` on the `PATH`. |
| `jtk.sortImports` | `true` | Sort imports when formatting a whole document. |
| `jtk.removeUnusedImports` | `true` | Remove unused imports when formatting a whole document. |
| `jtk.reflowLongStrings` | `true` | Reflow string literals that exceed the column limit. |
| `jtk.timeout` | `10000` | Maximum time in milliseconds for formatting a document. |

Formatting errors (usually a syntax error in the code being edited) are shown in the status bar and in the "jtk"
output channel (command **jtk: Show Output**); the document is left unchanged.

## Development

Requires Node.js and a Rust toolchain.

```sh
cd editors/vscode
npm install
npm test          # tests the integration against target/release/jtk
npm run package   # builds jtk for this platform and creates jtk-<version>.vsix
```

To try the package, install it with **Extensions: Install from VSIX…** in VS Code, or:

```sh
code --install-extension jtk-0.1.0.vsix
```

The package contains the jtk binary for the platform it was built on only. With VS Code's remote development (SSH,
containers, WSL) the extension runs on the remote machine, so build the package there. Otherwise, set `jtk.path` to
a jtk executable built for your machine.

To debug the extension, open `editors/vscode` in VS Code and run **Debug: Start Debugging** (F5) with the
"Extension Development Host" launch configuration.
