//! Linter rules B023 to B034 of group B (likely bugs).

mod common;
use common::{messages, violations};

#[test]
fn lost_cause() {
    let source = "class A {
    void f() {
        try {
        } catch (IllegalStateException e) {
            try {
            } catch (RuntimeException f) {
                throw new IllegalStateException(e.getMessage());
            }
            throw new java.lang.IllegalStateException(\"x\" + e.getMessage().trim());
        } catch (java.lang.RuntimeException | Error e) {
            throw (new RuntimeException(e.getMessage(), e));
        }
        try { } catch (Exception e) { Runnable r = () -> { throw new Exception(e.getMessage()); }; }
        try { } catch (Exception e) { throw new RuntimeException(e.getMessage()); }
        try { } catch (Exception e) { throw new Exception(e); }
        try { } catch (Exception e) { throw new Exception(cause.getMessage()); }
        try { } catch (Exception e) { throw new Exception(this.e.getMessage()); }
        throw new Exception(e.getMessage());
    }
}
";
    assert_eq!(violations("B023", source), ["7:17 B023", "9:13 B023", "11:13 B023", "13:60 B023"]);
}

#[test]
fn missing_super_call() {
    let source = "class A {
    public Object clone() { Runnable r = () -> { try { super.clone(); } catch (Exception e) { } }; return null; }
    protected void finalize() throws Throwable {
        new Object() { { try { A.super.finalize(); } catch (Throwable t) { } } };
    }
    native Object clone();
    Object clone(A this) { return null; }
    void finalize(int a) { }
    abstract static class B { protected abstract Object clone(); }
    static class C { protected Object clone() { java.util.function.Supplier<Object> s = super::toString; return s; } }
    static class D { protected Object clone() { return super.clone(1); } }
    static class E { protected void finalize() { super.finalize(); } }
    interface I { Object clone(); }
    static class F { protected Object clone() { new Object() { void g() { super.clone(); } }; return null; } }
}
";
    assert_eq!(violations("B024", source), ["3:20 B024", "9:57 B024", "10:39 B024", "11:39 B024", "14:39 B024"]);
    assert_eq!(messages("B024", source)[0], "Method 'finalize' should call 'super.finalize'");
}

#[test]
fn double_brace_initialization() {
    let source = "class A {
    List<Integer> a = new ArrayList<>() {{ add(1); }};
    List<Integer> b = new ArrayList<>() {{ add(1); }; ;};
    List<Integer> c = new ArrayList<>() {{ add(1); } int q; };
    List<Integer> d = new ArrayList<>() { };
    List<Integer> e = new ArrayList<>();
    List<Integer> f = new ArrayList<>() { /* c */ { add(1); } { add(2); } };
}
";
    assert_eq!(violations("B025", source), ["2:41 B025", "3:41 B025", "7:41 B025"]);
}

/// Checkstyle 14.4.0 does not report `++` and `--` yet (newer versions do).
#[test]
fn pattern_variable_assignment() {
    let source = "class A {
    void f(Object o, Object p) {
        if (o instanceof String a) { } else { a = \"\"; }
        if (!(o instanceof String b)) { return; } else if (p instanceof String c) { c = \"\"; b = \"\"; }
        b = \"\";
        Object t = o instanceof String d ? (d = \"\") : \"x\";
        while (o instanceof String e) { e = \"\"; }
        if (p != null) { boolean z = o instanceof String g; g = \"\"; }
        if (o instanceof String h) { h += \"\"; this.h = \"\"; h++; }
        if (!(o instanceof String k)) return;
        foo();
        k = \"1\";
        int unrelated;
        k = \"2\";
        if (!(o instanceof String m)) return; ; m = \"\";
        if (o instanceof P(Q(var n), int i)) { n = null; --i; }
    }
}
";
    assert_eq!(
        violations("B031", source),
        [
            "4:85 B031",
            "5:9 B031",
            "6:45 B031",
            "8:61 B031",
            "9:38 B031",
            "9:60 B031",
            "12:9 B031",
            "16:48 B031",
            "16:60 B031",
        ]
    );
    assert_eq!(messages("B031", source)[0], "Assignment of pattern variable 'c' is not allowed");
}

#[test]
fn missing_null_case() {
    let source = "class A {
    void f(Object o, String str, int i) {
        switch (str) { case \"a\": break; }
        switch (str) { case \"a\", null: break; }
        switch (str) { case null, \"a\": break; }
        switch (o) { case String s when s.isEmpty() -> { } case null, default -> { } }
        switch (o) { case String s -> { } default -> { } }
        int r = switch (i) { case 1 -> 2; default -> 3; };
        switch (str) { case \"\"\"
            a\"\"\" -> { } default -> { } }
        switch (str) { case \"a\" -> { } case null -> { } default -> { } }
        Object x = switch (o) { case Point(int a, int b) -> a; default -> 0; };
    }
}
";
    assert_eq!(violations("B032", source), ["3:9 B032", "4:9 B032", "7:9 B032", "12:20 B032"]);
}

#[test]
fn finalizer_and_clone() {
    let source = "class A {
    @Override
    protected void finalize() { }
    public Object clone() { return null; }
    void finalize(int a) { }
    Object clone(int a) { return null; }
    interface I { Object clone(); }
}
";
    assert_eq!(violations("B033", source), ["2:5 B033"]);
    assert_eq!(violations("B034", source), ["4:5 B034", "7:19 B034"]);
}
