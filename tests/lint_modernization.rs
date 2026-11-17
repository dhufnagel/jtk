//! Rules of group M (modernization). The expected positions are those Checkstyle 14.4.0 reports.

mod common;
use common::{fixed, messages, violations};

#[test]
fn enhanced_switch() {
    let source = "class A {
    int f(int x, boolean a) {
        switch (x) {
        case 1: return 1;
        case 2: case 3: throw new IllegalStateException();
        default: g();
        }
        switch (x) {
        case 1: g();
        case 2: return 2;
        }
        switch (x) {
        case 1 -> g();
        default -> {}
        }
        switch (x) {
        case 1:
            if (a) { return 1; } else { break; }
        case 2:
            while (true) { g(); }
        case 3:
            g();
        }
        switch (x) {
        case 1:
            for (;;) { if (a) { g(); } break; }
        default:
        }
        int y = switch (x) {
        case 1: yield 1;
        default: yield 2;
        };
        switch (x) {
        case 1:
            try { return 1; } finally { g(); }
        case 2:
            g(); // fall through
        default:
        }
        return y;
    }
}
";
    assert_eq!(violations("M002", source), ["3:9 M002", "16:9 M002", "29:17 M002"]);
    assert_eq!(fixed("M002", source), source);
}

#[test]
fn when_guard() {
    let source = "class A {
    void f(Object o) {
        switch (o) {
            case String s -> {
                if (s.isEmpty()) { g(); }
            }
            case Integer i -> {
                if (i > 0) { g(); } else { g(); }
            }
            case Long l -> {
                g();
                if (l > 0) { g(); }
            }
            case Double d when d > 0 -> {
                if (d > 1) { g(); }
                break;
            }
            case Float f -> g();
            default -> {}
        }
        switch (o) {
            case String s -> { if (s.isEmpty()) g(); }
            default -> {}
        }
    }
}
";
    assert_eq!(violations("M003", source), ["4:13 M003", "14:13 M003", "22:13 M003"]);
}

#[test]
fn fix_when_guard() {
    // Only where no other case follows (elsewhere a false guard would go on to the next cases), and the condition only
    // names pattern variables (`limit` may not be effectively final).
    let source = "class A {
    void f(Object o) {
        switch (o) {
            case String s -> {
                if (s.isEmpty()) { g(); }
            }
            case Integer i -> { if (i > 0) g(); }
            default -> {}
        }
        switch (o) {
            case Integer i -> { if (i > limit) g(); }
            default -> {}
        }
    }
}
";
    let expected = "class A {
    void f(Object o) {
        switch (o) {
            case String s -> {
                if (s.isEmpty()) { g(); }
            }
            case Integer i when i > 0 -> { g(); }
            default -> {}
        }
        switch (o) {
            case Integer i -> { if (i > limit) g(); }
            default -> {}
        }
    }
}
";
    assert_eq!(fixed("M003", source), expected);
}

#[test]
fn record_pattern_type_arguments() {
    let source = "class A {
    void f(Object o) {
        if (o instanceof Box<String>(var s)) {}
        if (o instanceof Box<?>(var s)) {}
        switch (o) {
            case Box<String>(String s) -> {}
            default -> {}
        }
    }
}
";
    assert_eq!(violations("M005", source), ["3:29 M005", "6:21 M005"]);
    let expected = source.replace("Box<String>", "Box");
    assert_eq!(fixed("M005", source), expected);
}

#[test]
fn unnamed_variable() {
    let source = "class A {
    void f() {
        try { g(); } catch (Exception e) { }
        try { g(); } catch (Exception e) { log(e); }
        try { g(); } catch (Exception e) { e = null; }
        try { g(); } catch (Exception _) { }
        try { g(); } catch (Exception e) { this.e.e(); }
        BiFunction<Integer, Integer, Integer> add = (a, b) -> a;
        Runnable r = () -> {};
        Function<String, String> id = x -> x;
        Function<String, String> c = (String x) -> \"\";
        try (var r1 = open(); var r2 = r1.open()) { }
        try (var r3 = open()) { r3.g(); }
        try { g(); } catch (Exception e) {
            new Runnable() { public void run() { try { } catch (Exception e) { } } };
            throw e;
        }
    }
}
";
    assert_eq!(
        violations("M008", source),
        ["3:29 M008", "5:29 M008", "7:29 M008", "8:57 M008", "11:46 M008", "12:35 M008", "15:65 M008"]
    );
    assert_eq!(messages("M008", source)[0], "Unused catch parameter 'e' should be unnamed");
}

#[test]
fn fix_unnamed_variable() {
    // An assigned variable is not renamed: the assignment would no longer compile.
    let source = "class A {
    void f() {
        try { g(); } catch (Exception e) { }
        try { g(); } catch (Exception e) { e = null; }
        BiFunction<Integer, Integer, Integer> add = (a, b) -> a;
        try (var r = open()) { }
    }
}
";
    let expected = "class A {
    void f() {
        try { g(); } catch (Exception _) { }
        try { g(); } catch (Exception e) { e = null; }
        BiFunction<Integer, Integer, Integer> add = (a, _) -> a;
        try (var _ = open()) { }
    }
}
";
    assert_eq!(fixed("M008", source), expected);
}

#[test]
fn final_parameters() {
    let source = "abstract class A {
    A(int a, final int b) {}
    void f(String s, String... rest) {}
    abstract void g(int x);
    void h(A this, int y) {}
    void i(@Deprecated final int y) {}
    Runnable r = (/* lambda */) -> {};
    void j(Iterable<String> l) {
        try { } catch (Exception e) { }
        for (String t : l) { }
    }
    void k(int _) {}
}
interface I {
    void k(int z);
    default void m(int w) {}
}
record R(int a) {
    R {}
}
";
    assert_eq!(
        violations("M011", source),
        ["2:7 M011", "3:12 M011", "3:22 M011", "5:20 M011", "8:12 M011", "16:20 M011"]
    );
}

#[test]
fn fix_final_parameters() {
    let source = "class A {\n    void f(String s, @Nullable Object o, int... rest) {}\n}\n";
    let expected = "class A {\n    void f(final String s, @Nullable final Object o, final int... rest) {}\n}\n";
    assert_eq!(fixed("M011", source), expected);
}

#[test]
fn missing_deprecated() {
    let source = "class A {
    /** @deprecated use g */
    void f() {}
    /** Doc. */
    @Deprecated
    void g() {}
    /**
     * @deprecated old
     */
    @Deprecated
    void h() {}
    /** Not {@deprecated} a tag. */
    void i() {}
    @Deprecated
    void j() {}
    /**
     * @deprecated one
     * @deprecated two
     */
    @java.lang.Deprecated
    void k() {}
    /** @deprecated */
    // comment
    int l;
    /** @deprecated first */
    /** second */
    int m;
}
";
    assert_eq!(violations("M013", source), ["3:5 M013", "5:5 M013", "18:8 M013", "24:5 M013"]);
    assert_eq!(messages("M013", source)[2], "Duplicate @deprecated tag");
}

#[test]
fn fix_missing_deprecated() {
    let source =
        "class A {\n    /** @deprecated use g */\n    void f() {}\n    /** Doc. */ @Deprecated void g() {}\n}\n";
    let expected = "class A {\n    /** @deprecated use g */\n    @Deprecated\n    void f() {}\n    /** Doc. */ @Deprecated void g() {}\n}\n";
    assert_eq!(fixed("M013", source), expected);
}

#[test]
fn sealed_permits() {
    let source = "sealed interface S permits X, Y {}
final class X implements S {}
final class Y implements S {}
sealed class T {}
final class U extends T {}
sealed interface V
    permits java.util.List {}
non-sealed class W extends T {}
sealed interface Z permits Q.P {}
class Q { final class P implements Z {} }
";
    assert_eq!(violations("M014", source), ["4:1 M014"]);
    assert_eq!(violations("M016", source), ["1:20 M016", "9:20 M016"]);
}

#[test]
fn fix_unnecessary_permits() {
    let source = "sealed interface S // the shapes\n    permits X, Y {}\nfinal class X implements S {}\nrecord Y() implements S {}\n";
    let expected = "sealed interface S // the shapes\n {}\nfinal class X implements S {}\nrecord Y() implements S {}\n";
    assert_eq!(fixed("M016", source), expected);
}

#[test]
fn final_local_variable() {
    // Only variables declared with a value: unlike Checkstyle, one assigned once later (in each branch) is not
    // reported. An assignment in a nested class does not count: it cannot mean the variable (`v`, `w2`).
    let source = "class A {
    void f(boolean c, Iterable<String> l, Object o) throws Exception {
        int a = 1;
        int b = 1;
        b++;
        int d;
        d = 2;
        int e = 1;
        e += 2;
        int g = 1;
        if (c) { (g) = 2; }
        final int i = 1;
        for (int j = 0; j < 1; j++) { }
        for (String s : l) { }
        int k = 1, m = 2;
        Runnable r = () -> { int n = 1; };
        int p = 1;
        Runnable u = () -> p = 2;
        int x = 1;
        switch (a) {
            case 1: int y = 1; break;
            default: y = 2;
        }
        var _ = g();
        try (var w = g()) { } catch (Exception z) { }
        if (o instanceof String t) { }
        int v = 1;
        new Object() { int v; void h() { v = 2; } };
        int w2 = 1;
        new Thread() { void h() { w2 = 2; } };
        { int q = 1; }
        int q = 1;
        q--;
    }
}
";
    assert_eq!(
        violations("M015", source),
        [
            "3:13 M015",
            "15:13 M015",
            "15:20 M015",
            "16:18 M015",
            "16:34 M015",
            "18:18 M015",
            "19:13 M015",
            "27:13 M015",
            "29:13 M015",
            "31:15 M015"
        ]
    );
    assert_eq!(messages("M015", source)[0], "Variable 'a' should be declared final");
}

#[test]
fn fix_final_local_variable() {
    // Only variables declared with a value and alone in their declaration.
    let source = "class A {
    void f() {
        int a = 1;
        @SuppressWarnings(\"x\") var b = g();
        int c;
        c = 2;
        int k = 1, m = 2;
    }
}
";
    let expected = "class A {
    void f() {
        final int a = 1;
        @SuppressWarnings(\"x\") final var b = g();
        int c;
        c = 2;
        int k = 1, m = 2;
    }
}
";
    assert_eq!(fixed("M015", source), expected);
}

#[test]
fn final_parameters_fix_skips_reassigned_parameters() {
    let source = "class A {\n    int f(int a, int b) {\n        b++;\n        return a + b;\n    }\n}\n";
    assert_eq!(violations("M011", source), ["2:11 M011", "2:18 M011"]);
    assert_eq!(fixed("M011", source), source.replace("int a,", "final int a,"));
}
