# jtk-parser

The Java lexer and parser of [jtk](../../README.md): a hand-written recursive-descent parser for Java up to 25,
including compact source files and module imports, that builds a javac-shaped AST. It does not recover from errors;
it reports the byte offset where parsing stopped.

- `parser::parse` parses a compilation unit into an `ast::Tree`.
- `lexer::lex` splits source text into whitespace, comment and token ranges.

The crate is not published on crates.io. Other projects depend on it through Git, pinned to a jtk release tag:

```toml
[dependencies]
jtk-parser = { git = "https://github.com/dhufnagel/jtk", tag = "v0.5.0" }
```

Its tests check that the parser builds the same trees as converting the syntax trees of
[tree-sitter-java-orchard](https://codeberg.org/grammar-orchard/tree-sitter-java-orchard):

```sh
cargo test -p jtk-parser
cargo run --release -p jtk-parser --example parser_check -- <dir>
```
