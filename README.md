# palantir-java-format (Rust port)

A Rust rewrite of [palantir-java-format](https://github.com/palantir/palantir-java-format), limited to formatting
Java source code. No IDE, Gradle or Spotless plugins.

It ports the original pipeline closely:

| Module | Ported from |
|---|---|
| `input.rs` | `JavaInput`, `JavacTokens` (own lexer instead of javac's) |
| `ast.rs` | – (converts the [tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) tree into a javac-shaped AST) |
| `visitor.rs` | `JavaInputAstVisitor`, `Java14InputAstVisitor`, `Java21InputAstVisitor`, `TypeNameClassifier` |
| `ops.rs` | `OpsBuilder` |
| `doc.rs` | `doc/*` (`Level`, `Break`, `State`, …): the line-breaking algorithm |
| `output.rs` | `JavaOutput` |
| `comments.rs` | `JavaCommentsHelper` |
| `modifiers.rs` | `ModifierOrderer` |
| `string_wrapper.rs` | `StringWrapper` (long string reflow, text block indentation) |
| `imports.rs` | `ImportOrderer` (Palantir/Google order), `RemoveUnusedImports` |

## Usage

```sh
cargo build --release
./target/release/palantir-java-format Foo.java        # print formatted source
./target/release/palantir-java-format -i Foo.java     # format in place
./target/release/palantir-java-format < Foo.java      # stdin to stdout
```

Like the original tool, it formats, removes unused imports, sorts imports and reflows long strings. Each step after
formatting can be switched off with `--skip-removing-unused-imports`, `--skip-sorting-imports` and
`--skip-reflowing-long-strings`.

## Tests

`tests/testdata` is the upstream formatter integration corpus; `tests/imports` holds the upstream import tests
(`GoogleImportStyleTest`, `RemoveUnusedImportsTest`, `RemoveUnusedImportsCaseLabelsTest`), all of which pass. `cargo test` checks that the output matches upstream
byte for byte, and that formatting the expected output with LF, CR and CRLF separators changes nothing.

```sh
cargo test --release -- --nocapture             # summary of all cases
PJF_TEST=B20128760 cargo test --release -- --nocapture   # one case, prints the actual output
cargo run --release --example corpus_check -- <dir>     # crash/idempotency check over a source tree
```

Status: 220 of 226 cases pass. The 6 failures are inputs that tree-sitter-java cannot parse:

- `this(...)` / `super(...)` outside a constructor body (`B26952926`, `palantir-expression-lambda-2`)
- type annotations on varargs, e.g. `String @A... args` (`B38203081`, `i282`, `i95`)
- annotations mixing positional and named arguments (`B38352414`, not valid Java but accepted by javac)

## Comparison with the original

`scripts/compare-with-original.sh` formats a source tree with the original tool (`--palantir`) and with this port,
and lists the files whose output differs:

```sh
scripts/compare-with-original.sh ./palantir-java-format-native-2.103.0-nativeImage-linux-glibc_x86-64.bin <dir>
```

Against palantir-java-format 2.103.0 (native image from Maven Central), with the full default pipeline:

| Corpus | Files | Identical | Differing |
|---|---|---|---|
| JDK 25 sources (`lib/src.zip`) | 15,227 | 15,224 | 3 – qualified record patterns (`case A.B(...)`, `case A.B _`), which tree-sitter-java cannot parse |
| palantir-java-format sources + test inputs | 363 | 358 | 5 – the parse failures listed above |

Both report the same 18 "Imports not contiguous" errors on the JDK sources.

Performance on the same JDK sources (one command per tool, which formats files in parallel on all cores): 11 s for
this port, 107 s for the original native image. On a single core, 1,456 files from `java.base` and others take 6.9 s
(`scripts/bench.sh`).

## Not ported

- javadoc formatting (off by default in the Palantir style)
- the `--lines`, `--offset`/`--length` and `--character-ranges` options (the partial formatting they need is
  ported, see `format_ranges`)

## License

Apache License 2.0, like the original. See `LICENSE`.
