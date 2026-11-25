//! Fixes for Eclipse's save actions: unsafe fixes of existing rules and the save-action rules of group M.

use std::path::{Path, PathBuf};

use jtk::cli::run;

mod common;
use common::{fixed, unsafe_fixed, violations};

/// Runs `jtk` with the arguments and file paths; returns the exit code and standard output.
fn jtk(args: &[&str], files: &[&Path]) -> (i32, String) {
    let mut all: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    all.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&all, &mut std::io::empty(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jtk-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn unsafe_fixes_only_when_asked() {
    // S006 has a safe fix, B009 an unsafe one.
    let source = "class A {\n    boolean f(Object o) {\n        return ((o.equals(null)));\n    }\n}\n";
    assert_eq!(
        fixed("B009,S006", source),
        "class A {\n    boolean f(Object o) {\n        return o.equals(null);\n    }\n}\n"
    );
    assert_eq!(
        unsafe_fixed("B009,S006", source),
        "class A {\n    boolean f(Object o) {\n        return o == null;\n    }\n}\n"
    );
}

#[test]
fn unsafe_fixes_command_line() {
    let dir = temp_dir("unsafe-fixes");
    let source = "class A {\n    boolean f(Object o) {\n        return (o.equals(null));\n    }\n}\n";
    let file = dir.join("A.java");
    std::fs::write(&file, source).unwrap();

    // Without --fix, --unsafe-fixes changes nothing.
    let remaining = format!(
        "{0}:3:16: S006 Unnecessary parentheses around a whole value\n{0}:3:17: B009 equals(null) instead of == null\n",
        file.display()
    );
    assert_eq!(jtk(&["lint", "--select", "B009,S006", "--unsafe-fixes"], &[&file]), (1, remaining));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
    let remaining = format!("{}:3:16: B009 equals(null) instead of == null\n", file.display());
    assert_eq!(jtk(&["lint", "--select", "B009,S006", "--fix"], &[&file]), (1, remaining));
    assert_eq!(jtk(&["lint", "--select", "B009,S006", "--fix", "--unsafe-fixes"], &[&file]), (0, String::new()));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "class A {\n    boolean f(Object o) {\n        return o == null;\n    }\n}\n"
    );

    std::fs::write(&file, source).unwrap();
    assert_eq!(jtk(&["check", "--select", "B009", "--fix", "--unsafe-fixes"], &[&file]), (0, String::new()));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "class A {\n    boolean f(Object o) {\n        return (o == null);\n    }\n}\n"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn equals_null_fix() {
    let source = "class A {
    void f(Object o, boolean b) {
        g(o.equals(null), !o.equals(null), b ? o.equals(null) : b, b && o.equals(null));
        o.equals(null);
        super.equals(null);
        o.equals(/* nothing */ null);
    }
}
";
    let expected = "class A {
    void f(Object o, boolean b) {
        g(o == null, !(o == null), b ? (o == null) : b, b && o == null);
        o.equals(null);
        super.equals(null);
        o.equals(/* nothing */ null);
    }
}
";
    assert_eq!(fixed("B009", source), source);
    assert_eq!(unsafe_fixed("B009", source), expected);
}

#[test]
fn string_literal_equality_fix() {
    let source = r#"class A {
    boolean f(String s, String t) {
        return s == "a" || "b" != s || s == "c" + t || (s + t) == ("d") || s /* same */ == "e";
    }
}
"#;
    let expected = r#"class A {
    boolean f(String s, String t) {
        return "a".equals(s) || !"b".equals(s) || ("c" + t).equals(s) || ("d").equals((s + t)) || s /* same */ == "e";
    }
}
"#;
    assert_eq!(unsafe_fixed("B007", source), expected);
}

#[test]
fn equals_avoid_null_fix() {
    let source = r#"class A {
    String s;
    boolean f(String t) {
        return s.equals("a") || this.s.equalsIgnoreCase("b" + t) || s.equals(t = "c") || s.equals(/* c */ "d");
    }
}
"#;
    let expected = r#"class A {
    String s;
    boolean f(String t) {
        return "a".equals(s) || ("b" + t).equalsIgnoreCase(this.s) || (t = "c").equals(s) || s.equals(/* c */ "d");
    }
}
"#;
    assert_eq!(violations("B008", source).len(), 4);
    assert_eq!(unsafe_fixed("B008", source), expected);
}

#[test]
fn unused_local_variable_fix() {
    let source = "class A {
    void f(String s) {
        int unused = 1;
        int written = 1;
        written = 2;
        int a = 1, b = g(), c = s.length;
        String t = s, u;
        Object call = g();
        for (int i = 0; ; ) {}
        int read = 0;
        g(read, c);
        // Kept with its comment.
        int commented /* here */ = 1;
    }
}
";
    let expected = "class A {
    void f(String s) {
        int written = 1;
        written = 2;
        int b = g(), c = s.length;
        Object call = g();
        for (int i = 0; ; ) {}
        int read = 0;
        g(read, c);
        // Kept with its comment.
        int commented /* here */ = 1;
    }
}
";
    assert_eq!(fixed("B026", source), source);
    assert_eq!(unsafe_fixed("B026", source), expected);
}

#[test]
fn unused_private_member_fix() {
    let source = "class A {
    private static final long serialVersionUID = 1L;
    private int unused;
    private int a = 1, b = 2;
    private java.util.Map<String, int[]> map = new java.util.HashMap<>(-1 + 2);
    private int[][] array = new int[][] {{1}, {}}, more = new int[2][];
    private Object registered = register();

    int read = 1;

    /** Javadoc. */
    // More.
    private void unusedMethod() {
    }

    @Override
    private void annotated() {}

    int g() { return b; } private void sameLine() {}

    private void last() {}
}
";
    let expected = "class A {
    private static final long serialVersionUID = 1L;
    private int b = 2;
    private Object registered = register();

    int read = 1;

    @Override
    private void annotated() {}

    int g() { return b; } \n}
";
    assert_eq!(fixed("B027", source), source);
    assert_eq!(unsafe_fixed("B027", source), expected);
}

#[test]
fn unused_private_member_fix_skips_overloads() {
    // JUnit's `@MethodSource` without a name refers to the method with the test's name.
    let source = "class A {
    private static Object[] cases() { return null; }

    @ParameterizedTest @MethodSource void cases(int x) {}
}
";
    assert_eq!(violations("B027", source), ["2:29 B027"]);
    assert_eq!(unsafe_fixed("B027", source), source);
}

#[test]
fn use_var() {
    let source = r#"class A {
    String name = "field";
    void f(Object o) {
        ArrayList<String> a = new ArrayList<String>();
        final Foo b = new Foo(1);
        String c = "c";
        Foo d = (Foo) o;
        String e = """
            e""";
        java.util.List<String> f = new java.util.ArrayList<String>();
        ArrayList<String> g = new ArrayList<>();
        Foo h = new Foo() {};
        long i = 1;
        int[] j = {1};
        int k[] = new int[1];
        Runnable l = () -> {};
        Object m = null;
        String n = "n", n2 = "n2";
        Foo p = (Foo & Bar) o;
        var q = new Foo();
        Object r = "r";
        for (String s = "s"; ; ) {}
        try (Foo t = new Foo()) {}
    }
}
"#;
    assert_eq!(violations("M006", source), ["4:9 M006", "5:15 M006", "6:9 M006", "7:9 M006", "8:9 M006", "22:14 M006"]);
    let fixed = fixed("M006", source);
    assert!(fixed.contains("        var a = new ArrayList<String>();\n        final var b = new Foo(1);\n        var c = \"c\";\n        var d = (Foo) o;\n        var e = \"\"\""));
    assert!(fixed.contains("for (var s = \"s\"; ; ) {}"));
}

#[test]
fn enhanced_for() {
    let source = r#"import java.util.*;
class E {
    private String[] names;
    void f(int[] values, List<String> items, String... args) {
        for (int i = 0; i < values.length; i++) {
            g(values[i]);
        }
        for (int i = 0; i <= args.length - 1; ++i) g(args[i], args[i]);
        for (int i = 0; i < items.size(); i += 1) { g(items.get(i)); }
        for (int i = 0; i < this.names.length; i = i + 1) { g(this.names[i]); }
        for (int i = 0; i < values.length; i++) { values[i] = 0; }
        for (int i = 0; i < values.length; i++) { g(i, values[i]); }
        for (int i = 0; i < values.length; i++) { g(values[i]); values = null; }
        int m[][] = new int[2][2];
        for (int i = 0; i < m.length; i++) { g(m[i]); }
        List raw = items;
        for (int i = 0; i < raw.size(); i++) { g(raw.get(i)); }
        for (int i = 0; i < items.size(); i++) { g(items.get(i)); items.add("x"); }
        for (Iterator<String> it = items.iterator(); it.hasNext();) { g(it.next()); }
        for (int i = 0; i < unknown.length; i++) { g(unknown[i]); }
    }
}
"#;
    assert_eq!(
        violations("M007", source),
        [
            "5:9 M007",
            "8:9 M007",
            "9:9 M007",
            "10:9 M007",
            "13:9 M007",
            "15:9 M007",
            "17:9 M007",
            "18:9 M007",
            "19:9 M007"
        ]
    );
    let fixed = fixed("M007", source);
    assert!(fixed.contains("        for (int value : values) {\n            g(value);\n"));
    assert!(fixed.contains("        for (String arg : args) g(arg, arg);\n"));
    assert!(fixed.contains("        for (String item : items) { g(item); }\n"));
    assert!(fixed.contains("        for (String name : this.names) { g(name); }\n"));
    assert!(fixed.contains("        for (int[] element : m) { g(element); }\n"));
    // Not fixed: the array is reassigned, the list raw or modified, an iterator loop.
    assert!(fixed.contains("{ g(values[i]); values = null; }"));
    assert!(fixed.contains("{ g(raw.get(i)); }"));
    assert!(fixed.contains("{ g(items.get(i)); items.add(\"x\"); }"));
    assert!(fixed.contains("for (Iterator<String> it"));
}

#[test]
fn enhanced_for_nested() {
    let source = r#"class F {
    void f(String[][] rows, int[] cells) {
        for (int i = 0; i < rows.length; i++) {
            for (int j = 0; j < rows[i].length; j++) { g(rows[i][j]); }
            for (int j = 0; j < cells.length; j++) { g(cells[j], rows[i]); }
        }
    }
}
"#;
    let expected = "class F {
    void f(String[][] rows, int[] cells) {
        for (String[] row : rows) {
            for (String element : row) { g(element); }
            for (int cell : cells) { g(cell, row); }
        }
    }
}
";
    assert_eq!(fixed("M007", source), expected);
}

#[test]
fn text_block() {
    let source = r#"class A {
    String a = "SELECT *\n" +
            "  FROM t\n" +
            "WHERE x = \"y\"\n";
    String b = "one\n"
        + "two  \n"
        + "\"\"\"\"\\\n" + "\n"
        + "\ttab\t\n"
        + "last \\";
    String c = "a\n" + "b\n";
    String d = "a" +
        "b";
    String e = "a\n" + x +
        "b";
    String f = "a\n" + /* b */
        "b";
}
"#;
    let expected = r#"class A {
    String a = """
            SELECT *
              FROM t
            WHERE x = "y"
            """;
    String b = """
        one
        two \s
        ""\""\\

        \ttab\t
        last \\\
        """;
    String c = "a\n" + "b\n";
    String d = "a" +
        "b";
    String e = "a\n" + x +
        "b";
    String f = "a\n" + /* b */
        "b";
}
"#;
    assert_eq!(violations("M009", source), ["2:16 M009", "5:16 M009"]);
    assert_eq!(fixed("M009", source), expected);
}

/// A concatenation of thousands of literals takes linear time, and two files in a row do not affect each other.
#[test]
fn long_text_block_concatenation() {
    let lines = "\"a\\n\" +\n".repeat(30_000);
    let source = format!("class A {{\n    String s = {lines}\"b\";\n}}\n");
    let started = std::time::Instant::now();
    assert_eq!(violations("M009", &source), ["2:16 M009"]);
    assert!(started.elapsed().as_secs() < 5, "took {:?}", started.elapsed());
    let shorter = "class A {\n    String s = \"a\\n\" +\n\"b\";\n}\n";
    assert_eq!(violations("M009", shorter), ["2:16 M009"]);
}

#[test]
fn identical_catch() {
    let source = "class A {
    void f() {
        try {
            g();
        } catch (IllegalStateException e) {
            log(e);
        } catch (IllegalArgumentException | NullPointerException e) {
            log(e); // the same
        } catch (UnsupportedOperationException e) {
            log(e);
        } catch (Error error) {
            log(error);
        } catch (RuntimeException e) {
            other(e);
        } catch (Exception e) {
            log(e);
        }
        try {
            g();
        } catch (IllegalStateException e) {
            e = null;
        } catch (IllegalArgumentException e) {
            e = null;
        }
        try {
            g();
        } catch (IllegalStateException e) {
            wrap(e);
        } catch (IllegalArgumentException e) {
            wrap(e);
        }
    }
    void wrap(IllegalStateException e) {}
    void wrap(IllegalArgumentException e) {}
}
";
    assert_eq!(violations("M010", source), ["7:11 M010", "9:11 M010", "11:11 M010", "15:11 M010", "22:11 M010"]);
    let unsafe_result = unsafe_fixed("M010", source);
    assert!(unsafe_result.contains(
        "        } catch (IllegalStateException e) {
            log(e);
        } catch (IllegalArgumentException | NullPointerException | UnsupportedOperationException e) {
            log(e); // the same
        } catch (Error error) {"
    ));
    assert!(unsafe_result.contains(
        "} catch (IllegalStateException e) {\n            e = null;\n        } catch (IllegalArgumentException e) {"
    ));
    assert_eq!(fixed("M010", source), source);
}

#[test]
fn final_field() {
    let source = r#"class A {
    private int value = 1;
    private int assigned;
    private int both = 1;
    private int written = 1;
    private static int counter = 1;
    int packageField = 1;
    private int a = 1, b = 2;
    private int early;
    private String local = "";
    @Setter private int injected = 1;
    private int inLambda;
    private int twice;

    A(int assigned, int b) {
        this.assigned = assigned;
        both = 2;
        this.b = b;
        g(early);
        early = 1;
        String local = "x";
        local = "y";
        Runnable r = () -> inLambda = 1;
        twice = 1;
        twice = 2;
    }

    A() {
        this(0, 0);
    }

    void set() {
        written = 2;
    }

    private static class Inner {
        int count = 0;
        Object o = new Object() { int hidden = 1; };
    }
}
"#;
    assert_eq!(
        violations("M012", source),
        ["2:17 M012", "3:17 M012", "8:17 M012", "9:17 M012", "10:20 M012", "37:13 M012", "38:16 M012", "38:39 M012"]
    );
    let fixed = fixed("M012", source);
    assert!(fixed.contains("    private final int value = 1;\n    private final int assigned;\n"));
    assert!(fixed.contains("    private final String local = \"\";\n"));
    assert!(
        fixed.contains(
            "        final int count = 0;\n        final Object o = new Object() { final int hidden = 1; };\n"
        )
    );
    // `b` cannot be final, nor `early`, which is read before it is assigned.
    assert!(fixed.contains("    private int a = 1, b = 2;\n    private int early;\n"));
}

#[test]
fn enhanced_switch_fix() {
    let source = r#"class S {
    int f(int x, Object o) {
        for (int i = 0; i < 3; i++) {
            switch (x) {
            case 1:
            case 2:
                g();
                break;
            case 3:
                g();
                h(); // two
                break;
            case 4:
                throw new IllegalStateException();
            case 5: {
                g();
                break;
            }
            case 6:
                continue;
            default:
                for (;;) { break; }
                return 1;
            }
        }
        switch (x) {
        case 1:
            int y = 1;
            break;
        default:
            y = 2;
            g(y);
        }
        switch (x) {
        case 1:
            if (o == null) break;
            g();
            break;
        default:
        }
        switch (x) {
        case 1: // comment
            g();
            break;
        case 2:
        default:
            g();
        }
        switch (o) {
        case String s when s.isEmpty():
            g();
            break;
        default:
            h();
        }
        return 0;
    }
}
"#;
    let expected_first = "            switch (x) {
case 1, 2 -> g();
case 3 -> {
g();
                h(); // two
}
case 4 -> throw new IllegalStateException();
case 5 -> g();
case 6 -> {
continue;
}
default -> {
for (;;) { break; }
                return 1;
}
}
";
    let expected_last = "        switch (o) {
case String s when s.isEmpty() -> g();
default -> h();
}
";
    assert_eq!(violations("M002", source), ["4:13 M002", "26:9 M002", "34:9 M002", "41:9 M002", "49:9 M002"]);
    assert_eq!(fixed("M002", source), source);
    let unsafe_result = unsafe_fixed("M002", source);
    assert!(unsafe_result.contains(expected_first));
    assert!(unsafe_result.contains(expected_last));
    // Not converted: a variable used in another group, a nested `break`, a comment after a label, `default` with
    // another label.
    assert!(unsafe_result.contains("        case 1:\n            int y = 1;\n"));
    assert!(unsafe_result.contains("            if (o == null) break;\n"));
    assert!(unsafe_result.contains("        case 1: // comment\n"));
    assert!(unsafe_result.contains("        case 2:\n        default:\n"));
}
