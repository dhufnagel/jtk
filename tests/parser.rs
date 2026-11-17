//! The hand-written parser builds the same trees as converting tree-sitter's syntax trees.

use std::fs;
use std::path::Path;

/// Parses `text` both ways: the trees (or failures) must agree.
fn assert_same_tree(name: &str, text: &str) {
    let converted = jtk::parse::parse(text)
        .and_then(|tree| jtk::ast::Converter { src: text }.compilation_unit(&tree.root_node()).ok())
        .map(|t| format!("{t:?}"));
    let own = jtk::parser::parse(text).ok().map(|t| format!("{t:?}"));
    assert_eq!(converted, own, "{name}");
}

#[test]
fn same_trees_as_tree_sitter_on_the_formatter_tests() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata");
    let mut count = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "input" || e == "output") {
            assert_same_tree(&path.display().to_string(), &fs::read_to_string(&path).unwrap());
            count += 1;
        }
    }
    assert!(count > 400);
}

#[test]
fn same_trees_for_constructs_with_tree_sitter_quirks() {
    let cases = [
        // `(a) + b` is an addition; casts to other types stay casts.
        "class A { int f() { return (a) + b - (c & D) - (int) -e + (a.B) - f * (G[]) h; } }",
        // A for-each variable without modifiers starts at `for`.
        "class A { void f() { for (String s : l) {} for (final var t : l) {} for (int u[] : l) {} } }",
        // Each label is a case of its own; `case null, default`.
        "class A { void f() { switch (x) { case 1: case 2: g(); default: case 3: } switch (o) { case null, default -> {} } } }",
        "class A { int f() { return switch (x) { case (int) A, (int) B -> 1; case C when c -> 2; default -> 3; }; } }",
        "class A { void f() { switch (o) { case L l when l.stream().anyMatch(x -> x > 0) -> {} default -> {} } } }",
        // Receivers: `T this` and `@A T Outer.this` are receiver parameters, `@A T this` is a parameter.
        "class T { T(T this) {} T(@A T this, int x) {} void f(@A T B.this) {} }",
        // Explicit constructor invocations anywhere in a constructor body, and their `;`.
        "class A { A() { int x; this(1); } A(int x) { outer.super(); } A(long y) { <T>super(); } }",
        // An annotation as default value is also a return type annotation.
        "@interface A { B b() default @B(1); String[] c() default {}; }",
        // Records, enums, modules, patterns and the unnamed pattern.
        "record R<T>(@A int a, T... b) implements I { R { } }",
        "enum E implements I { A(1) { void f() {} }, B, ; int x; E() {} E(int x) {} }",
        "@Deprecated open module m { requires transitive static a.b; exports c to d, e; uses f.G; provides H with I; }",
        "class A { void f() { if (o instanceof P(Q(var a), _) && o instanceof final S s) {} switch (o) { case P(int a, _) when a > 0 -> {} default -> {} } } }",
        // Types: generics with `>>`, annotations, arrays and wildcards.
        "class A { Map<String, List<? extends @N B>> m; int @A [] @B [] a; java.lang.@C String s; List<int[]>[] l; }",
        "class A { void f() { x >>= 1; y = a >> b >>> c >= d ? -1 : -1L; z = List.<String>of(); w = String[]::new; v = int[].class; u = List<String>::size; } }",
        // Lambdas, casts of lambdas, anonymous and inner classes, array creation.
        "class A { void f() { g((a, b) -> a, (int c) -> { }, () -> d, (Runnable) () -> {}, x -> x); new B<>() {}; o.new C(); new int[3][]; new @A int @B [] {1}; } }",
        // Empty input and comments only.
        "",
        "  // comment\n",
    ];
    for (i, case) in cases.iter().enumerate() {
        assert_same_tree(&format!("case {i}: {case}"), case);
    }
}

#[test]
fn rejects_what_tree_sitter_rejects() {
    // Invalid Java that tree-sitter rejects: the formatter then reports tree-sitter's syntax error.
    for case in [
        "interface I implements J {}",
        "class A { void f() { this(1); } }",
        "class A { void f() { switch (x) { case 1: break; default -> {} } } }",
        "class A { Object o = new <String>Object(); }",
        "class A { Object o = (G[]) -h; }",
        "class A { Object o = (G<T>) +h; }",
    ] {
        assert!(jtk::parser::parse(case).is_err(), "{case}");
    }
}
