//! Linter rules and the `jtk lint` command line.

use std::path::{Path, PathBuf};

use jtk::cli::run;
use jtk::lint::{Selection, fix, lint};

mod common;
use common::{fixed, lines_of, violations};

#[test]
fn equals_without_hash_code() {
    let source = "class A {
    public boolean equals(Object o) { return true; }
    class B { public int hashCode() { return 1; } }
    class C { public int hashCode() { return 1; } public boolean equals(java.lang.Object o) { return true; } }
    abstract class D { public abstract int hashCode(); public boolean equals(Object o) { return true; } }
    class E { public boolean equals(E e) { return true; } }
}
";
    assert_eq!(violations("B00", source), ["2:20 B001", "3:26 B001", "5:71 B001", "6:30 B002"]);
}

#[test]
fn empty_catch() {
    let source = "class A {
    void f() {
        try { g(); } catch (Exception e) {}
        try { g(); } catch (Exception e) { ; }
        try { g(); } catch (Exception e) { /* g may fail */ }
        try { g(); } catch (Exception ignored) {}
        try { g(); } catch (Exception e) { log(e); }
    }
}
";
    assert_eq!(violations("B003", source), ["3:42 B003", "4:42 B003"]);
}

#[test]
fn missing_braces() {
    let source = "class A {
    void f() {
        if (a) b();
        if (a) { b(); } else if (c) { d(); } else e();
        for (;;) x();
        for (int i : is) x();
        while (y) z();
        do z(); while (y);
        while (y) {}
    }
}
";
    assert_eq!(violations("S001", source), ["3:9 S001", "4:46 S001", "5:9 S001", "6:9 S001", "7:9 S001", "8:9 S001"]);
}

#[test]
fn syntax_error() {
    assert_eq!(
        lint("class A { void f( }", &Selection::default()).unwrap_err().to_string(),
        "1:19: error: syntax error"
    );
}

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
fn lint_command_line() {
    let dir = temp_dir("lint");
    let clean = dir.join("Clean.java");
    let dirty = dir.join("Dirty.java");
    std::fs::write(&clean, "class Clean {}\n").unwrap();
    std::fs::write(&dirty, "class Dirty {\n    void f() {\n        if (a) b();\n    }\n}\n").unwrap();

    assert_eq!(jtk(&["lint"], &[&clean]), (0, String::new()));
    let expected = format!("{}:3:9: S001 'if' body without braces\n", dirty.display());
    assert_eq!(jtk(&["lint"], &[&clean, &dirty]), (1, expected));
    assert_eq!(jtk(&["lint"], &[]).0, 2);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn check_command_line() {
    let dir = temp_dir("check");
    let file = dir.join("A.java");
    let unformatted = "class A { void f() {\n if (a) b(); } }\n";
    std::fs::write(&file, unformatted).unwrap();

    // Without -i, positions refer to the unchanged file.
    let expected = format!("{0}: not formatted\n{0}:2:2: S001 'if' body without braces\n", file.display());
    assert_eq!(jtk(&["check"], &[&file]), (1, expected));

    // With -i, the file is formatted and positions refer to the formatted text.
    let expected = format!("{}:3:9: S001 'if' body without braces\n", file.display());
    assert_eq!(jtk(&["check", "-i"], &[&file]), (1, expected));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "class A {\n    void f() {\n        if (a) b();\n    }\n}\n");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn selection() {
    let source =
        "class A {\n    void f() {\n        try { g(); } catch (Exception e) {}\n        if (a) b();\n    }\n}\n";
    let codes = |select: &[&str], ignore: &[&str]| {
        let to_strings = |codes: &[&str]| codes.iter().map(|c| c.to_string()).collect();
        let selection = Selection::new(to_strings(select), to_strings(ignore)).unwrap();
        lint(source, &selection).unwrap().iter().map(|d| d.rule.code()).collect::<Vec<_>>()
    };
    assert_eq!(codes(&[], &[]), ["B003", "S001"]);
    let all = codes(&["ALL"], &[]);
    assert!(all.contains(&"B003") && all.contains(&"S001"));
    assert_eq!(codes(&["B00"], &[]), ["B003"]);
    assert_eq!(codes(&[], &["S001"]), ["B003"]);
    assert!(Selection::new(vec!["X".to_string()], vec![]).is_err());
    let extended = Selection::new(vec!["S".to_string()], vec![]).unwrap().extended(vec!["B003".to_string()]).unwrap();
    let codes: Vec<_> = lint(source, &extended).unwrap().iter().map(|d| d.rule.code()).collect();
    assert_eq!(codes, ["B003", "S001"]);
}

#[test]
fn suppression() {
    let source = r#"class A {
    void comments() {
        if (a) b(); // jtk:ignore
        if (a) b(); // jtk:ignore[S001]
        if (a) b(); // jtk:ignore[B003]
        if (a) b(); // NOPMD - documented
    }
    @SuppressWarnings("checkstyle:needbraces") void checkstyle() { if (a) b(); }
    @SuppressWarnings("NeedBraces") void checkstylePlain() { if (a) b(); }
    @SuppressWarnings({"unchecked", "PMD.ControlStatementBraces"}) void pmd() { if (a) b(); }
    @SuppressWarnings("PMD") void allPmd() { if (a) b(); }
    @SuppressWarnings(value = "jtk:S001") void jtk() { if (a) b(); }
    @SuppressWarnings("jtk:B003") void otherRule() { if (a) b(); }
}
"#;
    assert_eq!(violations("S001", source), ["5:9 S001", "13:54 S001"]);
}

#[test]
fn fix_missing_braces() {
    let source = "class A {\n    void f() {\n        if (a) if (b) c(); else d();\n        while (x) y(); // jtk:ignore\n    }\n}\n";
    let expected = "class A {\n    void f() {\n        if (a) { if (b) { c(); } else { d(); } }\n        while (x) y(); // jtk:ignore\n    }\n}\n";
    assert_eq!(fix(source, &Selection::default()).unwrap(), expected);
}

#[test]
fn covariant_equals() {
    let source = "class A {
    public boolean equals(A a) { return true; }
}
class B {
    public boolean equals(B b) { return true; }
    public boolean equals(Object o) { return true; }
}
abstract class C { public abstract boolean equals(C c); }
";
    assert_eq!(lines_of("B002", source), [2]);
}

#[test]
fn fall_through() {
    let source = "class A {
    int f(int a) {
        switch (a) {
            case 1:
                g();
            case 2:
                g();
                // fall through
            case 3:
            case 4:
                if (a > 0) { return 1; } else { throw new IllegalStateException(); }
            case 5:
                while (true) { g(); }
            case 6:
                while (true) { break; }
            case 7:
                outer: for (;;) { break outer; }
            case 8:
                switch (a) { case 1: return 1; default: throw new IllegalStateException(); }
            case 9:
                g();
        }
        return 0;
    }
}
";
    assert_eq!(lines_of("B004", source), [6, 16, 18]);
}

#[test]
fn string_literal_equality() {
    let source = "class A {
    boolean f(String s) { return s == \"a\" || \"b\" != s || s == \"c\" + s || s.equals(\"d\"); }
}
";
    assert_eq!(lines_of("B007", source), [2, 2, 2]);
}

#[test]
fn equals_null() {
    let source = "class A { boolean f(Object o) { return o.equals(null) || o == null || equals(null); } }\n";
    assert_eq!(lines_of("B009", source), [1]);
}

#[test]
fn finally_exit() {
    let source = "class A {
    int f() {
        try { return 1; } finally {
            return 2;
        }
    }
    void g() {
        try { h(); } finally {
            Runnable r = () -> { return; };
            throw new IllegalStateException();
        }
    }
}
";
    assert_eq!(lines_of("B010", source), [4, 10]);
}

#[test]
fn identical_operands() {
    let source = "class A {
    int x;
    void f(int y) {
        x = x;
        this.x = y;
        boolean b = y == y || this.x != this.x || g() == g() || 1 == 1;
    }
}
";
    assert_eq!(lines_of("B011", source), [4, 6, 6]);
}

#[test]
fn modified_control_variable() {
    let source = "class A {
    void f(int[] values) {
        for (int i = 0; i < 10; i++) {
            i++;
        }
        for (int i = 0, j = 0; i < 10; i++) {
            j++;
        }
        for (int v : values) {
            v = 0;
        }
    }
}
";
    assert_eq!(lines_of("B014", source), [4, 10]);
}

#[test]
fn jumbled_incrementer() {
    let source = "class A {
    void f() {
        for (int i = 0; i < 10; i++) {
            for (int j = 0; j < 10; i++) {}
            for (int k = 0; k < 10; k++) {}
        }
    }
}
";
    assert_eq!(lines_of("B015", source), [4]);
}

#[test]
fn null_check() {
    let source = "class A {
    boolean f(String s) {
        return s.isEmpty() && s != null
            || s != null || s.isEmpty()
            || s == null && s.length() > 0
            || s != null && s.isEmpty();
    }
}
";
    assert_eq!(lines_of("B016", source), [3, 4, 5]);
}

#[test]
fn null_check_unboxing_and_other_dereferences() {
    let source = "class A {
    boolean f(Boolean boo, Integer i, String s, A a, Integer[] arr) {
        return boo == null && boo != true
            || boo != null || boo == false
            || boo == null && boo
            || i == null && i < 1
            || i == null && i + 1 > 0
            || s == null && (s + 1).isEmpty()
            || a == null && handle(a::toString)
            || a == null && a.new Inner().ok()
            || a.getB() != null || a.getB().ok()
            || boo == null && boo == other;
    }
}
";
    assert_eq!(lines_of("B016", source), [3, 4, 5, 6, 7, 9, 10, 11]);
}

#[test]
fn null_check_guarded_dereference() {
    let source = "class A {
    boolean f(String value, String old) {
        return (value != null && !value.equals(old)) || value == null
            || (value == null || !value.equals(old)) && value != null
            || value.isEmpty() && value != null;
    }
}
";
    assert_eq!(lines_of("B016", source), [5]);
}

#[test]
fn big_decimal_double() {
    let source = "class A {
    Object a = new BigDecimal(0.1);
    Object b = new java.math.BigDecimal(-1.5);
    Object c = new BigDecimal(\"0.1\");
    Object d = new BigDecimal(1);
}
";
    assert_eq!(lines_of("B017", source), [2, 3]);
}

#[test]
fn big_decimal_double_from_double_values() {
    let source = "class A {
    static final double RATE = 0.1;
    Object f(double d, Float g, int n, double[] values, String text) {
        float local = 0.1f;
        Object[] all = {
            new BigDecimal(d),
            new BigDecimal(g),
            new BigDecimal(local),
            new BigDecimal(RATE),
            new BigDecimal(6 * d),
            new BigDecimal((double) n),
            new BigDecimal(Math.sqrt(n)),
            new BigDecimal(g.doubleValue()),
            new BigDecimal(n),
            new BigDecimal(n * 2),
            new BigDecimal(values[0] + \"\"),
            new BigDecimal(text + d),
            new BigDecimal(Math.abs(n)),
            new BigDecimal(d, MathContext.DECIMAL64),
        };
        return all;
    }
}
";
    assert_eq!(lines_of("B017", source), [6, 7, 8, 9, 10, 11, 12, 13]);
}

#[test]
fn big_decimal_double_shadowed_variables() {
    let source = "class A {
    double d;
    Object f(int d) {
        return new BigDecimal(d);
    }
    Object g(long count) {
        Runnable r = () -> { int d = 1; Object o = new BigDecimal(d); };
        return new BigDecimal(d);
    }
}
class B extends Base {
    Object f() { return new BigDecimal(inherited); }
}
";
    assert_eq!(lines_of("B017", source), [8]);
}

#[test]
fn double_checked_locking() {
    let source = "class A {
    private Object instance;
    private volatile Object safe;
    Object get() {
        if (instance == null) {
            synchronized (this) {
                if (instance == null) { instance = new Object(); }
            }
        }
        if (safe == null) {
            synchronized (this) {
                if (safe == null) { safe = new Object(); }
            }
        }
        return instance;
    }
}
";
    assert_eq!(lines_of("B019", source), [5]);
}

#[test]
fn double_checked_locking_negated_check() {
    let source = "class A {
    private Object instance;
    Object get() {
        if (!(instance != null)) {
            synchronized (this) {
                if (instance == null) { instance = new Object(); }
            }
        }
        if (!(!(instance == null))) {
            synchronized (this) {
                if (null == instance) { instance = new Object(); }
            }
        }
        if (!(instance == null)) {
            synchronized (this) {
                if (instance == null) { instance = new Object(); }
            }
        }
        return instance;
    }
}
";
    assert_eq!(lines_of("B019", source), [4, 9]);
}

#[test]
fn unused_local_variable() {
    let source = "class A {
    void f(Object o, int[] values) {
        int unused = 1;
        int written = 1;
        written = 2;
        int compound = 0;
        compound += 1;
        int read = 1;
        g(read);
        for (int i = 0; g(0); i++) {}
        for (int v : values) {}
        if (o instanceof String s) {}
        if (!(o instanceof Integer n)) { return; }
        g(n);
        Object shadowed = new Object() { int shadowed = 1; };
        int _ = 1;
    }
    A() {
        if (new Object() instanceof final String t) { g(t); }
    }
}
";
    assert_eq!(lines_of("B026", source), [3, 4, 10, 11, 12, 15]);
}

#[test]
fn unused_private_member() {
    let source = r#"class A {
    private static final long serialVersionUID = 1L;
    private int unused;
    private int read;
    private int viaThis;
    @Inject private Object injected;
    int g() { return read + this.viaThis; }
    private void unusedMethod() {}
    private void called() {}
    private void referenced() {}
    private static Object source() { return null; }
    private void writeObject(java.io.ObjectOutputStream out) {}
    @MethodSource("source") void h() { called(); Runnable r = this::referenced; }
}
"#;
    assert_eq!(lines_of("B027", source), [3, 8]);
}

#[test]
fn simplify_boolean_return() {
    let source = "class A {
    boolean f(int a) { if (a > 0) { return true; } else { return false; } }
    boolean g(boolean a, boolean b) { if (a || b) return false; else return true; }
    boolean h(boolean a) { if (a) return true; else return true; }
}
";
    assert_eq!(lines_of("S002", source), [2, 3, 4]);
    let expected = "class A {
    boolean f(int a) { return a > 0; }
    boolean g(boolean a, boolean b) { return !(a || b); }
    boolean h(boolean a) { if (a) return true; else return true; }
}
";
    assert_eq!(fixed("S002", source), expected);
}

#[test]
fn simplify_boolean_expression() {
    let source = "class A {
    boolean f(boolean a, boolean b) {
        return a == true && (a || b) != false && !false && b ? true : false;
    }
    boolean g(boolean a) { return a || true; }
}
";
    assert_eq!(lines_of("S003", source), [3, 3, 3, 3, 5]);
    let expected = "class A {
    boolean f(boolean a, boolean b) {
        return a && (a || b) && b;
    }
    boolean g(boolean a) { return a || true; }
}
";
    assert_eq!(fixed("S003", source), expected);
}

#[test]
fn unnecessary_parentheses() {
    let source = "class A {
    int f(int a, boolean b, Object o) {
        int x = (a + 1);
        if ((b)) { x = (a); }
        switch (a) { case(1): return (x); default: }
        boolean c = (a > 0 && b) || (!b) || (a + 1) > 2;
        java.util.function.IntUnaryOperator op = (y) -> y + 1;
        return ((String) o).length() + (a - 1) * 2;
    }
}
";
    assert_eq!(lines_of("S006", source), [3, 4, 4, 5, 5, 6, 6, 7]);
    let expected = "class A {
    int f(int a, boolean b, Object o) {
        int x = a + 1;
        if (b) { x = a; }
        switch (a) { case 1: return x; default: }
        boolean c = a > 0 && b || !b || (a + 1) > 2;
        java.util.function.IntUnaryOperator op = y -> y + 1;
        return ((String) o).length() + (a - 1) * 2;
    }
}
";
    assert_eq!(fixed("S006", source), expected);
}

#[test]
fn unnecessary_semicolon() {
    let source = "class A {
    enum E { X, Y; }
    enum F { X; void f() {} }
    void f() throws Exception {
        try (java.io.InputStream in = null;) {}
        ;
        while (g()) ;
    };
};
";
    assert_eq!(lines_of("S007", source), [2, 5, 6, 7, 8, 9]);
    let expected = "class A {
    enum E { X, Y }
    enum F { X; void f() {} }
    void f() throws Exception {
        try (java.io.InputStream in = null) {}
        
        while (g()) {}
    }
}
";
    assert_eq!(fixed("S007", source), expected);
}

#[test]
fn redundant_modifier() {
    let source = "interface I {
    public static final int X = 1;
    public abstract void f(final int a);
    static interface J {}
}
final class A {
    public A() {}
    final void f() {}
    private final void g() {}
    @SafeVarargs private final void h(Object... values) {}
    static enum E { X; private E() {} }
    strictfp void s() {}
}
public class B {
    public B() {}
}
";
    assert_eq!(lines_of("S008", source), [2, 2, 2, 3, 3, 3, 4, 7, 8, 9, 11, 11, 12]);
    let expected = "interface I {
    int X = 1;
    void f(int a);
    interface J {}
}
final class A {
    A() {}
    void f() {}
    private void g() {}
    @SafeVarargs private final void h(Object... values) {}
    enum E { X; E() {} }
    void s() {}
}
public class B {
    public B() {}
}
";
    assert_eq!(fixed("S008", source), expected);
}

#[test]
fn instanceof_pattern() {
    let source = "class A {
    String s;
    void f(Object o) {
        if (o instanceof String) {
            String text = (String) o;
            g(text);
        }
        if (o != null && o instanceof Integer && g(o)) {
            var number = (Integer) o;
        }
        if (o instanceof String && s != null) {
            String s = (String) o;
        }
        if (o instanceof String) {
            g(o);
            String late = (String) o;
        }
    }
}
";
    assert_eq!(lines_of("M001", source), [4, 8]);
    let expected = "class A {
    String s;
    void f(Object o) {
        if (o instanceof String text) {
            g(text);
        }
        if (o != null && o instanceof Integer number && g(o)) {
        }
        if (o instanceof String && s != null) {
            String s = (String) o;
        }
        if (o instanceof String) {
            g(o);
            String late = (String) o;
        }
    }
}
";
    assert_eq!(fixed("M001", source), expected);
    let java_11 = Selection::new(vec!["M001".to_string()], vec![]).unwrap().with_java_version(11);
    assert!(lint(source, &java_11).unwrap().is_empty());
}

#[test]
fn diamond_operator() {
    let source = "class A {
    java.util.List<String> a = new java.util.ArrayList<String>();
    java.util.Map<String, Integer> b = new java.util.HashMap<>();
    var c = new java.util.ArrayList<String>();
    Comparable<String> d = new Comparable<String>() { public int compareTo(String o) { return 0; } };
}
";
    assert_eq!(lines_of("M004", source), [2]);
    assert!(fixed("M004", source).contains("java.util.List<String> a = new java.util.ArrayList<>();"));
}

#[test]
fn diamond_operator_assignments_and_returns() {
    let source = "import java.util.*;
class A {
    List<String> field;
    Map<String, List<Integer>> map;
    List<String> f(List<String> param, Collection<String> other) {
        field = new ArrayList<String>();
        this.map = new HashMap<String, List<Integer>>();
        param = new ArrayList<String>(other);
        List<String> local;
        local = new LinkedList<String>();
        var inferred = new ArrayList<String>();
        inferred = new ArrayList<String>();
        unknown = new ArrayList<String>();
        use(new ArrayList<String>());
        local = true ? new ArrayList<String>() : null;
        Supplier<List<String>> s = () -> { return new ArrayList<String>(); };
        return new ArrayList<String>();
    }
    Object g() {
        return new ArrayList<String>();
    }
}
";
    assert_eq!(lines_of("M004", source), [6, 7, 8, 10, 17]);
}

#[test]
fn diamond_operator_left_out_where_inference_may_differ() {
    let source = "import java.util.*;
class A {
    Comparator<String> c = new Comparator<String>() { public int compare(String a, String b) { return 0; } };
    Collection<Number> n = new ArrayList<Number>(List.of(1));
    WeakReference<Class<?>> r = new WeakReference<Class<?>>(String.class);
    Collector<CharSequence, ?, String> j = new CollectorImpl<CharSequence, StringBuilder, String>(a, b);
    List<String> e = new <Object> Generic<String>();
    List<String> d = new ArrayList<String>(10);
}
";
    assert_eq!(lines_of("M004", source), [4, 8]);
}

#[test]
fn diamond_operator_raw_types() {
    let source = "import java.util.ArrayList;
import java.util.HashMap;
import java.util.concurrent.*;
// import java.util.Vector;
class StackUser {
    List a = new ArrayList();
    List<String> b = new ArrayList();
    List<String> c = new ArrayList(other);
    List d = new ArrayList(10);
    Map e = new java.util.TreeMap();
    Map f = new ConcurrentHashMap();
    Vector g = new Vector();
    Object h = new Foo();
    var i = new HashMap();
    Map j = new HashMap() {};
}
";
    assert_eq!(lines_of("M004", source), [6, 7, 9, 10, 11]);
    let fixed = fixed("M004", source);
    assert!(fixed.contains("List<String> b = new ArrayList<>();"));
    assert!(fixed.contains("Map e = new java.util.TreeMap<>();"));
    assert!(fixed.contains("List<String> c = new ArrayList(other);"));
}

#[test]
fn diamond_operator_raw_type_declared_in_file() {
    let source = "import java.util.ArrayList;
class ArrayList {}
class A { Object a = new ArrayList(); }
";
    assert_eq!(lines_of("M004", source), Vec::<usize>::new());
}

#[test]
fn diamond_operator_fix() {
    let source = "import java.util.*;
class A {
    List<String> field;
    List<String> f() {
        field = new ArrayList<String>();
        this.field = new ArrayList</* kept */ String>();
        return new ArrayList<String>();
    }
}
";
    assert_eq!(
        fixed("M004", source),
        "import java.util.*;
class A {
    List<String> field;
    List<String> f() {
        field = new ArrayList<>();
        this.field = new ArrayList</* kept */ String>();
        return new ArrayList<>();
    }
}
"
    );
}

#[test]
fn naming() {
    let source = "package Com.example;
class bad_Type<Tx, T> {
    private static final long serialVersionUID = 1L;
    static final int lowerConstant = 1;
    static final int GOOD_CONSTANT = 1;
    static int Counter;
    int Member;
    void Method(int Param, String... Rest) {
        int Local = 0;
        for (int I = 0; I < 1; I++) {}
        try { g(); } catch (Exception problem) {} catch (RuntimeException x) {}
        java.util.function.IntUnaryOperator op = Value -> Value;
        if (Local instanceof Integer Pattern) {}
    }
    @Override public String ToString() { return null; }
    record Point(int X, int y) {}
}
interface I { int lower = 1; }
";
    let codes: Vec<String> =
        violations("N", source).into_iter().map(|v| v.split(' ').nth(1).unwrap().to_string()).collect();
    assert_eq!(
        codes,
        [
            "N012", "N001", "N011", "N004", "N005", "N003", "N002", "N007", "N007", "N006", "N006", "N008", "N007",
            "N009", "N010", "N004"
        ]
    );
}
