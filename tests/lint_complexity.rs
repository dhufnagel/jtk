//! Linter rules of group C: complexity and size, with Checkstyle's default limits.

mod common;
use common::{lines_of, messages, violations};

/// `n` lines with a statement each.
fn statements(n: usize) -> String {
    (0..n).map(|i| format!("        f({i});\n")).collect()
}

#[test]
fn cyclomatic_complexity() {
    let source = "class A {
    void at(int x) {
        if (a && b || c) {} else if (d) {}
        while (e) {} do {} while (f); for (;;) {} for (int i : is) {}
        x = g ? 1 : 2;
    }
    void over(int x) {
        if (a && b || c) {} else if (d) {}
        while (e) {} do {} while (f); for (;;) {} for (int i : is) {}
        x = g ? 1 : 2;
        try {} catch (E e) {}
    }
    void cases(Object x) {
        switch (x) { case 1: case 2: case 3: case 4: default: }
        switch (x) { case 1, 2 -> a(); case Integer i when i > 3 -> b(); default -> c(); }
        Runnable r = () -> { if (a) {} if (b) {} if (c) {} };
        new Object() { void f() { if (a) {} } };
    }
}
";
    assert_eq!(violations("C001", source), ["7:5 C001", "13:5 C001"]);
    assert_eq!(
        messages("C001", source),
        ["Cyclomatic Complexity is 11 (max allowed is 10).", "Cyclomatic Complexity is 11 (max allowed is 10)."]
    );
}

#[test]
fn boolean_expression_complexity() {
    let source = "class A {
    void f() {
        boolean x = a && b || c & d;
        boolean y = a && b || c & d ^ e;
        g(a & b | c, d && e && f || g || h);
        if (a && b && c && d && e) {}
        x = a & b | c | d ^ e;
    }
    public boolean equals(Object o) { return a && b && c && d && e; }
    boolean field = a && b && c && d && e;
}
";
    // As in Checkstyle, the violation of an argument is reported at the first argument, and that of an expression
    // statement at the enclosing block.
    assert_eq!(violations("C003", source), ["2:14 C003", "4:19 C003", "5:17 C003", "6:9 C003"]);
    assert_eq!(messages("C003", source)[0], "Boolean expression complexity is 4 (max allowed is 3).");
}

#[test]
fn java_ncss() {
    let method = |n| format!("    void f() {{\n{}    }}\n", statements(n));
    let source = format!("class A {{\n{}{}}}\n", method(49), method(50));
    assert_eq!(messages("C004", &source), ["NCSS for this method is 51 (max allowed is 50)."]);
    assert_eq!(lines_of("C004", &source), [53]);
}

#[test]
fn java_ncss_counts_like_checkstyle() {
    // The method counts 1, each of the 46 calls 1, `int a, b;` 1, `if`, `else` and their statements 1 each, the
    // `switch` and the `case` 1 each; the switch rule's expression and the static import count nothing.
    let source = format!(
        "import static a.B.c;\nclass A {{\n    void f(int x) {{\n{}        int a, b;\n        if (x) g(); else h();\n        switch (x) {{ case 1 -> g(); }}\n    }}\n}}\n",
        statements(46)
    );
    assert_eq!(messages("C004", &source), ["NCSS for this method is 54 (max allowed is 50)."]);
}

#[test]
fn executable_statement_count() {
    let method = |n| format!("    void f() {{\n{}    }}\n", statements(n));
    let source = format!("class A {{\n{}{}}}\n", method(30), method(31));
    assert_eq!(messages("C005", &source), ["Executable statement count is 31 (max allowed is 30)."]);
    assert_eq!(lines_of("C005", &source), [34]);
}

#[test]
fn executable_statement_count_halves_nodes() {
    // As in Checkstyle, each statement list counts half its nodes, so 60 `if` statements and a variable count 31;
    // a lambda counts for itself.
    let ifs = "        if (a) {} if (b) {} if (c) {}\n".repeat(20);
    let source = format!(
        "class A {{\n    void f() {{\n{ifs}        Runnable r = () -> {{\n{}        }};\n    }}\n}}\n",
        statements(31)
    );
    assert_eq!(violations("C005", &source), ["2:5 C005", "23:25 C005"]);
}

#[test]
fn method_length() {
    let method = |name, n| format!("    void {name}() {{\n{}    }}\n", statements(n));
    let source = format!("class A {{\n{}{}}}\n", method("f", 148), method("g", 149));
    assert_eq!(messages("C006", &source), ["Method g length is 151 lines (max allowed is 150)."]);
    assert_eq!(lines_of("C006", &source), [152]);
}

#[test]
fn file_length() {
    assert!(violations("C007", &"//\n".repeat(2000)).is_empty());
    assert_eq!(violations("C007", &"//\n".repeat(2001)), ["1:1 C007"]);
    assert_eq!(messages("C007", &"//\r\n".repeat(2001)), ["File length is 2001 lines (max allowed is 2000)."]);
}

#[test]
fn lambda_body_length() {
    let lambda = |n| format!("        Runnable r = () -> {{\n{}        }};\n", statements(n));
    // An expression body counts from its first to its last line.
    let arguments: String = (0..9).map(|i| format!("                {i},\n")).collect();
    let source = format!(
        "class A {{\n    void f() {{\n{}{}        Supplier<String> s = () ->\n            g(\n{arguments}                9);\n    }}\n}}\n",
        lambda(8),
        lambda(9),
    );
    assert_eq!(violations("C008", &source), ["13:25 C008", "24:33 C008"]);
    assert_eq!(messages("C008", &source)[0], "Lambda body length is 11 lines (max allowed is 10).");
}

#[test]
fn anonymous_inner_class_length() {
    let class = |n| format!("        new Object() {{\n{}        }};\n", "            int x;\n".repeat(n));
    let source = format!("class A {{\n    void f() {{\n{}{}    }}\n}}\n", class(18), class(19));
    assert_eq!(violations("C009", &source), ["23:9 C009"]);
    assert_eq!(messages("C009", &source), ["Anonymous inner class length is 21 lines (max allowed is 20)."]);
}

#[test]
fn parameter_number() {
    let source = "class A {
    void f(int a, int b, int c, int d, int e, int f, int g) {}
    void g(int a, int b, int c, int d, int e, int f, int g, int... h) {}
    A(A this, int a, int b, int c, int d, int e, int f, int g) {}
    @Override public void h(int a, int b, int c, int d, int e, int f, int g, int h) {}
}
";
    assert_eq!(violations("C010", source), ["3:10 C010", "4:5 C010", "5:27 C010"]);
    assert_eq!(messages("C010", source)[0], "More than 7 parameters (found 8).");
}

#[test]
fn record_component_number() {
    let source = "record A(int a, int b, int c, int d, int e, int f, int g, int h) {}
record B(int a, int b, int c, int d, int e, int f, int g, int h, int... i) {}
";
    assert_eq!(violations("C011", source), ["2:1 C011"]);
    assert_eq!(messages("C011", source), ["Number of record components is 9 (max allowed is 8)."]);
}

#[test]
fn outer_type_number() {
    assert!(violations("C013", "package p;\nclass A { class B {} }\n").is_empty());
    let source = "// header\npackage p;\nclass A {}\ninterface B {}\n";
    assert_eq!(violations("C013", source), ["2:1 C013"]);
    assert_eq!(messages("C013", source), ["Outer types defined is 2 (max allowed is 1)."]);
}

#[test]
fn nested_if_depth() {
    let source = "class A {
    void f() {
        if (a) {
            if (b) {
                if (c) {}
            } else if (d) {
            } else {
                if (e) {}
            }
            if (f) {} else { if (g) { if (h) {} } }
        }
    }
}
";
    assert_eq!(violations("C014", source), ["5:17 C014", "10:39 C014"]);
    assert_eq!(messages("C014", source)[0], "Nested if-else depth is 2 (max allowed is 1).");
}

#[test]
fn nested_for_and_try_depth() {
    let source = "class A {
    void f() {
        for (;;) { for (int i : is) { for (;;) {} } }
        try { try { try {} finally {} } catch (E e) {} } finally {}
        for (;;) { Runnable r = () -> { for (;;) { for (;;) {} } }; }
    }
}
";
    assert_eq!(violations("C015", source), ["3:39 C015", "5:52 C015"]);
    assert_eq!(violations("C016", source), ["4:21 C016"]);
    assert_eq!(messages("C016", source), ["Nested try depth is 2 (max allowed is 1)."]);
}

#[test]
fn line_length() {
    let long = "x".repeat(81);
    let source = format!(
        "import {long};\nclass A {{\n    // {}\n    // {}\n\t\t\t\t\t\t\t\t\t\t// x\n}}\n",
        "x".repeat(73),
        "x".repeat(74)
    );
    assert_eq!(violations("C020", &source), ["4:1 C020", "5:1 C020"]);
    assert_eq!(
        messages("C020", &source),
        ["Line is longer than 80 characters (found 81).", "Line is longer than 80 characters (found 84)."]
    );
}

#[test]
fn throws_count() {
    let source = "class A {
    void f() throws A, B, C, D {}
    void g() throws A, B, C, D, java.io.E {}
    private void h() throws A, B, C, D, E {}
    @Override public void i() throws A, B, C, D, E {}
    A() throws A, B, C, D, E {}
}
";
    assert_eq!(violations("C021", source), ["3:14 C021", "6:9 C021"]);
    assert_eq!(messages("C021", source)[0], "Throws count is 5 (max allowed is 4).");
}
