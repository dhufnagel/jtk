# jtk

jtk is a Java toolkit written in Rust. Its formatter is a rewrite of
[palantir-java-format](https://github.com/palantir/palantir-java-format) with byte-identical output, limited to formatting
Java source code. No IDE, Gradle or Spotless plugins.

It ports the original pipeline closely:

| Module | Ported from |
|---|---|
| `input.rs` | `JavaInput`, `JavacTokens` (own lexer instead of javac's) |
| `ast.rs` | – (converts the [tree-sitter-java-orchard](https://codeberg.org/grammar-orchard/tree-sitter-java-orchard) tree into a javac-shaped AST) |
| `visitor.rs` | `JavaInputAstVisitor`, `Java14InputAstVisitor`, `Java21InputAstVisitor`, `TypeNameClassifier` |
| `ops.rs` | `OpsBuilder` |
| `doc.rs` | `doc/*` (`Level`, `Break`, `State`, …): the line-breaking algorithm |
| `output.rs` | `JavaOutput` |
| `comments.rs` | `JavaCommentsHelper` |
| `modifiers.rs` | `ModifierOrderer` |
| `string_wrapper.rs` | `StringWrapper` (long string reflow, text block indentation) |
| `imports.rs` | `ImportOrderer` (Palantir/Google order), `RemoveUnusedImports` |
| `javadoc.rs` | `javadoc/*` (`JavadocFormatter`, `JavadocLexer`, `JavadocWriter`) |

## Usage

```sh
cargo build --release
./target/release/jtk Foo.java Bar.java   # print formatted sources
./target/release/jtk -i src/**/*.java    # format in place
./target/release/jtk - < Foo.java        # stdin to stdout
./target/release/jtk --dry-run --set-exit-if-changed src/**/*.java   # CI check
```

The command line is compatible with the original's (`--help` lists all options): `-i`, `-`, `--assume-filename`,
`--fix-imports-only`, `--skip-sorting-imports`, `--skip-removing-unused-imports`, `--skip-reflowing-long-strings`,
`--format-javadoc`,
`--dry-run`, `--set-exit-if-changed`, `--lines`, `--character-ranges`, `--offset`/`--length`,
`--output-replacements`, `--version` and `@argfile`. Like the original, it formats, removes unused imports, sorts
imports and reflows long strings, and processes files in parallel.

An extension for Visual Studio Code is in [`editors/vscode`](editors/vscode).

Differences from the original command line:

- Only the Palantir style exists. `--palantir` is accepted and implied; `--aosp` is rejected. (The original uses the
  Google style unless `--palantir` is given.)
- Syntax error messages come from this port's parser, not javac (same `file:line:column: error:` shape).
- `--format-javadoc` is an addition: the original offers javadoc formatting only through its Java API (and plugins
  built on it), not on the command line.
- As in the original, usage errors (unknown flags, missing files, conflicting options) print the usage and exit with
  status 0.

## Tests

`tests/cli.rs` checks the command line against outputs of the original. `tests/testdata` is the upstream formatter
integration corpus; `tests/imports` holds the upstream import tests
(`GoogleImportStyleTest`, `RemoveUnusedImportsTest`, `RemoveUnusedImportsCaseLabelsTest`), all of which pass. `cargo test` checks that the output matches upstream
byte for byte, and that formatting the expected output with LF, CR and CRLF separators changes nothing.

```sh
cargo test --release -- --nocapture             # summary of all cases
PJF_TEST=B20128760 cargo test --release -- --nocapture   # one case, prints the actual output
cargo run --release --example corpus_check -- <dir>     # crash/idempotency check over a source tree
```

Status: 223 of 226 cases pass. The 3 failures are invalid Java that javac's parser accepts but the tree-sitter
grammar rejects:

- `this(...)` / `super(...)` outside a constructor body (`B26952926`, `palantir-expression-lambda-2`)
- annotations mixing positional and named arguments (`B38352414`)

The parser is [tree-sitter-java-orchard](https://codeberg.org/grammar-orchard/tree-sitter-java-orchard), a maintained
fork of tree-sitter-java (which no longer merges contributions). It supports Java 21–25 syntax that tree-sitter-java
does not, such as qualified record patterns, several patterns in one `case`, statements before `this()`/`super()`,
and annotated varargs.

## Comparison with the original

`scripts/compare-with-original.sh` formats a source tree with the original tool (`--palantir`) and with this port,
and lists the files whose output differs:

```sh
scripts/compare-with-original.sh ./palantir-java-format-native-2.103.0-nativeImage-linux-glibc_x86-64.bin <dir>
```

Against palantir-java-format 2.103.0 (native image from Maven Central), with the full default pipeline:

| Corpus | Files | Identical | Differing |
|---|---|---|---|
| JDK 25 sources (`lib/src.zip`) | 15,227 | 15,227 | 0 |
| palantir-java-format sources + test inputs | 363 | 361 | 2 – the invalid `this(...)` test inputs listed above |

Both report the same 18 "Imports not contiguous" errors on the JDK sources.

Javadoc formatting (`--format-javadoc`, which the original only offers through its Java API as
`formatJavadoc(true)`) produces identical output on all 15,227 JDK files, compared with the original library's
`Formatter.formatSource`. It changes 1,302 of the 1,456 files in `java.base` and other modules that the smaller
comparisons use. The upstream `JavadocFormattingTest` cases are in `tests/javadoc`.

Performance on the same JDK sources (one command per tool, which formats files in parallel on all cores): 11 s for
this port, 107 s for the original native image. On a single core, 1,456 files from `java.base` and others take 6.9 s
(`scripts/bench.sh`).

## Continuous integration and releases

`.github/workflows/ci.yml` checks formatting and Clippy and runs all tests (including the VS Code extension's) on
Linux, macOS and Windows for every push and pull request.

`.github/workflows/release.yml` builds jtk for Linux (glibc 2.17 or later), macOS and Windows on x64 and ARM64,
packages the VS Code extension once per platform, and creates a draft GitHub release. To release:

1. Set the version in `Cargo.toml` (and `editors/vscode/package.json`), commit, and push.
2. Tag the commit: `git tag v0.1.0 && git push origin v0.1.0` (the tag must match the version in `Cargo.toml`).
3. Review the draft release on GitHub and publish it.

Each archive contains the binary, `LICENSE`, `NOTICE`, `README.md` and `THIRD-PARTY-LICENSES.md` (the licenses of
the Rust crates compiled into the binary, generated by `scripts/third-party-licenses.py`). Running the workflow
manually builds the same artifacts without creating a release.

## Not ported

- the Google and AOSP styles

## License

Apache License 2.0, like the original. See `LICENSE` and `NOTICE`.
