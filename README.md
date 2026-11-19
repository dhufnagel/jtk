# jtk

jtk is a Java toolkit written in Rust. Its formatter is a rewrite of
[palantir-java-format](https://github.com/palantir/palantir-java-format) with byte-identical output, limited to formatting
Java source code. No IDE, Gradle or Spotless plugins.

It ports the original pipeline closely:

| Module | Ported from |
|---|---|
| `input.rs` | `JavaInput`, `JavacTokens` (own lexer instead of javac's) |
| `parser.rs`, `ast.rs` | – (a hand-written recursive-descent parser that builds a javac-shaped AST) |
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

An extension for Visual Studio Code is in [`editors/vscode`](editors/vscode). It runs `jtk lsp`, a language server
that other editors (IntelliJ with LSP4IJ, Neovim, Zed, ...) can use too: it reports the lint violations of open files,
offers their fixes, suppression comments and `source.fixAll.jtk` as code actions, and formats documents and ranges.
The nearest `jtk.toml` applies; the client's initialization options `sortImports`, `removeUnusedImports`,
`reflowLongStrings`, `formatJavadoc` and `unsafeFixes` (all optional) set the rest.

Differences from the original command line:

- Only the Palantir style exists. `--palantir` is accepted and implied; `--aosp` is rejected. (The original uses the
  Google style unless `--palantir` is given.)
- Syntax error messages come from this port's parser, not javac (same `file:line:column: error:` shape).
- `--format-javadoc` is an addition: the original offers javadoc formatting only through its Java API (and plugins
  built on it), not on the command line.
- `--config FILE` is an addition: the `[format]` settings of a `jtk.toml` (see [Configuration](#configuration)) apply,
  from the given file or the nearest one in each file's directory or above.
- As in the original, usage errors (unknown flags, missing files, conflicting options) print the usage and exit with
  status 0.

## Linter (in development)

```sh
./target/release/jtk lint src                  # file:line:column: CODE message; exit 1 on violations, 2 on usage errors
./target/release/jtk check src                 # CI: unformatted files and violations in one pass
./target/release/jtk check -i src              # format in place, then lint the formatted files
./target/release/jtk check --fix src           # apply automatic fixes, format in place, lint
./target/release/jtk check --fix --unsafe-fixes src  # also apply the unsafe fixes (Eclipse save actions)
```

`jtk lint` checks what the formatter leaves alone. The rules come from Checkstyle, PMD, Error Prone and the Eclipse
save actions; [docs/regel-analyse.md](docs/regel-analyse.md) lists them all. Without `--select`, the default set
runs, chosen for few false alarms:

| Codes | Rules |
|---|---|
| `B001`–`B004`, `B007`, `B009`–`B011`, `B014`–`B017`, `B019` | likely bugs: equals/hashCode, empty catch, fall-through, `==` on strings, null checks, ... |
| `B026`, `B027` | unused local variables, private fields and methods |
| `S001`–`S003`, `S006`–`S008` | simplifications: braces, boolean expressions, parentheses, semicolons, modifiers |
| `M001`, `M004` | modernizations: `instanceof` patterns (Java 16), diamond operator (Java 7) |
| `N001`–`N012` | naming conventions, with Checkstyle's default formats |

The other rules are the remaining Checkstyle checks, with Checkstyle's default properties and limits, and PMD rules
(B016–B019, B022, B028, S004, S005, S010, S019, A010, A011, C022) with PMD's defaults:

| Codes | Rules |
|---|---|
| `I001`–`I005` | star, illegal and static imports, unnecessarily qualified types |
| `B005`–`B034` | missing `default`, inner assignments, parameter assignment, illegal catch/throws, `clone`/`finalize`, ... |
| `S009`–`S021` | annotation order, `super()`, explicit initialization, one variable per declaration, array style, literal case, empty and nested blocks, block lambdas (most with fixes) |
| `M002`–`M016` | switch expressions, `when` guards, unnamed variables (`_`), `final` parameters and locals, `@Deprecated`, `permits` |
| `M006`, `M007`, `M009`, `M010`, `M012` | the remaining Eclipse save actions: `var`, enhanced `for`, text blocks, multi-catch, `final` fields |
| `N013`, `N014` | abbreviations in names, illegal identifiers |
| `A001`–`A017` | design: utility classes, final classes, visibility, overload order, magic numbers, file names, ... |
| `C001`–`C022` | complexity and size: cyclomatic and cognitive complexity, lengths, counts, nesting depths |

Each module in `src/lint/` documents its rule. `scripts/compare-with-checkstyle.sh` compares the rules with their
Checkstyle equivalents; on the Checkstyle test inputs, they report 99% of Checkstyle's violations on the same line.
`scripts/compare-with-pmd.sh` does the same with PMD 7, on the code of PMD's rule tests
(`scripts/extract-pmd-test-code.py`).

`jtk lint` and `jtk check` take files and directories; in directories they check the `.java` files that no
`.gitignore` file ignores.

Options:

- `--fix` applies automatic fixes (the S and M rules), leaving out fixes that could drop side effects;
  `jtk lint --fix` writes the fixed files unformatted.
- `--unsafe-fixes` with `--fix` also applies the unsafe fixes, which may change the behaviour in edge cases, as in
  ruff: removing unused variables and private members (B026, B027), `"a".equals(s)` for `==` and literal-first
  `equals` (B007, B008), `x == null` for `x.equals(null)` (B009), switch rules (M002), text blocks (M009) and
  multi-catch (M010). Review their changes.
- `--select` and `--ignore` take comma-separated code prefixes (`--select B,S001`); `--select ALL` checks all rules,
  `--extend-select` adds rules to the selected or default set.
- `--java VERSION` skips rules that suggest newer language features than the project's Java version.
- `--config FILE` uses the given `jtk.toml` for all files.

### Configuration

The nearest `jtk.toml` in a file's directory or above applies to it; paths in it are relative to its directory.
All keys are optional:

```toml
java = 17                          # as --java
exclude = ["src/generated"]        # not checked when a directory is given (glob patterns, as in .gitignore)

[format]                           # also for `jtk` without `check`; the command line can only switch steps off
sort-imports = true
remove-unused-imports = true
reflow-long-strings = true
format-javadoc = false

[lint]
select = ["ALL"]                   # as --select, which replaces it
extend-select = ["C022"]           # --extend-select adds to it
ignore = ["A015"]                  # --ignore adds to it

[lint.per-file-ignores]
"src/test/**" = ["B022", "C"]

[lint.options]                     # rule parameters, named after Checkstyle's (PMD's for C022)
C001 = { max = 15 }
```

Rules with parameters, with their defaults:

| Code | Parameters |
|---|---|
| `I002` | `illegal-pkgs = ["sun"]` |
| `B020` | `illegal-class-names = ["Exception", "Error", "RuntimeException", "Throwable"]` |
| `B021` | `illegal-class-names = ["Error", "RuntimeException", "Throwable"]` |
| `N013` | `allowed-abbreviation-length = 3` |
| `A014` | `allowed-distance = 3` |
| `A015` | `ignore-numbers = [-1, 0, 1, 2]` |
| `C004` | `method-maximum = 50`, `class-maximum = 1500`, `record-maximum = 150`, `file-maximum = 2000` |
| `C022` | `report-level = 15` |
| other `C` rules | `max`: C001 10, C003 3, C005 30, C006 150, C007 2000, C008 10, C009 20, C010 7, C011 8, C013 1, C014–C016 1, C021 4 |

Violations are suppressed by `// jtk:ignore` or `// jtk:ignore[S001]` on their line, and by
`@SuppressWarnings("jtk:S001")` on an enclosing declaration. Existing Checkstyle and PMD suppressions apply to the
equivalent rules: `@SuppressWarnings` with `"checkstyle:needbraces"`, `"NeedBraces"`, `"PMD.ControlStatementBraces"`,
`"PMD"` or `"unused"`, and `// NOPMD`.

`jtk check` lints each file on the formatter's thread, sharing the parse of the file with the formatter. With the rules
that Checkstyle and PMD also have, `jtk lint` is 17 to 23 times faster than Checkstyle and 2.6 to 16 times faster than
PMD ([docs/benchmark.md](docs/benchmark.md)).

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

Status: 223 of 226 cases pass. The 3 failures are invalid Java that javac's parser accepts but jtk's parser
rejects:

- `this(...)` / `super(...)` outside a constructor body (`B26952926`, `palantir-expression-lambda-2`)
- annotations mixing positional and named arguments (`B38352414`)

The parser is hand-written (`src/parser.rs`) and supports Java up to 25, including compact source files and module
imports. It builds the same trees as converting the syntax tree of
[tree-sitter-java-orchard](https://codeberg.org/grammar-orchard/tree-sitter-java-orchard), which jtk used before:
`tests/parser.rs` and `cargo run --release --example parser_check -- <dir>` check this, with tree-sitter as a
development dependency only. Formatting and linting use the same tree.

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

Performance on the same JDK sources (one command per tool, which formats files in parallel on all cores): 12 s for
this port, 244 s for the original native image ([docs/benchmark.md](docs/benchmark.md), `scripts/bench-tools.sh`). On a
single core, 1,456 files from `java.base` and others take 6.9 s (`scripts/bench.sh`).

## Continuous integration and releases

`.github/workflows/ci.yml` checks formatting and Clippy and runs all tests (including the VS Code extension's) on
Linux, macOS and Windows for every push and pull request.

`.github/workflows/release.yml` builds jtk for Linux (glibc 2.17 or later), macOS and Windows on x64 and ARM64,
packages the VS Code extension once per platform, and creates a draft GitHub release. To release:

1. Set the version in `Cargo.toml` (and `editors/vscode/package.json`), commit, and push.
2. Tag the commit: `git tag v0.3.1 && git push origin v0.3.1` (the tag must match the version in `Cargo.toml`).
3. Review the draft release on GitHub and publish it.

Each archive contains the binary, `LICENSE`, `NOTICE`, `README.md` and `THIRD-PARTY-LICENSES.md` (the licenses of
the Rust crates compiled into the binary, generated by `scripts/third-party-licenses.py`). Running the workflow
manually builds the same artifacts without creating a release.

## Not ported

- the Google and AOSP styles

## License

Apache License 2.0, like the original. See `LICENSE` and `NOTICE`.
