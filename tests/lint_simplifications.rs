//! The rules of group S (simplifications) and their fixes.

mod common;
use common::{fixed, lines_of, messages, violations};

#[test]
fn annotation_after_modifier() {
    let source = "@A public class C {
    public @B class D {}
    public @B void f() {}
    public @B int g() { return 0; }
    public @B int field;
    public @B C() {}
    void h(final @B int p) { final @B int local = 0; }
    final public @B void wrongKeywordOrder() {}
    @A public static @B @C final void i() {}
}
";
    assert_eq!(lines_of("S009", source), [2, 3, 9]);
    assert_eq!(
        messages("S009", "class C { public @Deprecated void f() {} }"),
        ["'@Deprecated' annotation should precede the modifier keywords"]
    );
}

#[test]
fn annotation_after_modifier_fix() {
    let source = "class C {
    @A public static @B @C(1) final void i() {}
    public /* why */ @B void j() {}
}
";
    assert_eq!(
        fixed("S009", source),
        "class C {
    @A @B @C(1) public static final void i() {}
    public /* why */ @B void j() {}
}
"
    );
}

#[test]
fn super_constructor_call() {
    let source = "class C extends B {
    C() { super(); }
    C(int a) { super(a); }
    C(Outer o) { o.super(); }
    C(long a) { <String>super(); }
    C(char c) { this(1); }
}
";
    assert_eq!(violations("S011", source), ["2:11 S011"]);
}

#[test]
fn super_constructor_call_fix() {
    let source = "class C extends B {
    C() {
        super();
        init();
    }
    C(int a) { super(); }
    C(long a) {
        check(a);
        super();
    }
}
";
    assert_eq!(
        fixed("S011", source),
        "class C extends B {
    C() {
        init();
    }
    C(int a) { }
    C(long a) {
        check(a);
        super();
    }
}
"
    );
}

#[test]
fn explicit_initialization() {
    let source = "class C {
    int a = 0;
    long b = 0L, c = 1;
    double d = 0.0, e = 0x0p0;
    float f = 0f;
    boolean g = false, h = true;
    char i = '\\0', j = 0, k = 'k';
    Object l = null;
    int[] m = null, n[] = null;
    int o = (0);
    Integer p = 0;
    final int q = 0;
    static String r = null;
    void f() { int local = 0; }
    interface I { int CONSTANT = 0; }
}
";
    assert_eq!(lines_of("S012", source), [2, 3, 4, 4, 5, 6, 7, 7, 8, 9, 9, 13]);
    assert_eq!(
        messages("S012", "class C { char c = 0; }"),
        ["Variable 'c' explicitly initialized to '\\0' (its default value)"]
    );
}

#[test]
fn explicit_initialization_fix() {
    let source = "class C {
    int a = 0, b = 1;
    String c = null;
    int[] d[] = null;
    Object e = /* none yet */ null;
}
";
    assert_eq!(
        fixed("S012", source),
        "class C {
    int a, b = 1;
    String c;
    int[] d[];
    Object e = /* none yet */ null;
}
"
    );
}

#[test]
fn multiple_declarations() {
    let source = "class C {
    int a, b;
    int c; int d;
    void f() {
        String e, f;
        Object g; Object h;
        for (int i = 0, j = 0; i < j; i++) {}
        int k;
        int l;
    }
    java.lang.String m; String
        n;
}
";
    assert_eq!(violations("S013", source), ["2:5 S013", "5:9 S013"]);
    assert_eq!(messages("S013", "class C { int a, b; }"), ["Each variable declaration must be in its own statement"]);
}

#[test]
fn multiple_declarations_fix() {
    let source = "class C {
    @Nullable private static String a = \"a\", b[] = {}, c;
    void f() {
        final int d = 1, e; e = d;
        int g, /* why */ h;
    }
}
";
    assert_eq!(
        fixed("S013", source),
        "class C {
    @Nullable private static String a = \"a\";
    @Nullable private static String[] b = {};
    @Nullable private static String c;
    void f() {
        final int d = 1;
        final int e; e = d;
        int g, /* why */ h;
    }
}
"
    );
}

#[test]
fn array_type_style() {
    let source = "class C {
    int[] a;
    int b[];
    int[] c[];
    int d[][];
    int e()[] { return null; }
    void f(String args[]) {
        for (String s[] : x) {}
    }
    void g(int h
             []) {}
    void i(int j
                 []) {}
}
";
    assert_eq!(
        violations("S014", source),
        ["3:10 S014", "4:12 S014", "5:10 S014", "5:12 S014", "6:12 S014", "7:23 S014", "8:22 S014", "13:18 S014"]
    );
}

#[test]
fn array_type_style_fix() {
    let source = "class C {
    int a [] = {};
    int[] b[];
    int c()[] { return null; }
    void f(String args[]) {}
    int d[], e;
}
";
    assert_eq!(
        fixed("S014", source),
        "class C {
    int[] a = {};
    int[][] b;
    int[] c() { return null; }
    void f(String[] args) {}
    int d[], e;
}
"
    );
}

#[test]
fn literal_case() {
    let source = "class C {
    long a = 1l;
    long b = 1L;
    int c = 0xff;
    int d = 0XFF;
    int e = 0B1;
    double f = 1E5;
    double g = 1e5D;
    float h = 0x1.8P1f;
    double i = 0x1.8p1D;
    double j = 0xAp1d;
    long k = 0xFFL;
    int l = 0xFF;
}
";
    assert_eq!(lines_of("S015", source), [2, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(
        messages("S015", "class C { long a = 0xffl; }"),
        ["Should use uppercase 'L'", "Should use uppercase hexadecimal letters"]
    );
}

#[test]
fn literal_case_fix() {
    let source = "class C { long a = 0xffl + 1l + 0X1fL + 0B1L; double b = 1E5D + 0x1.aP1F; }";
    assert_eq!(fixed("S015", source), "class C { long a = 0xFFL + 1L + 0x1FL + 0b1L; double b = 1e5d + 0x1.Ap1f; }");
}

#[test]
fn empty_block() {
    let source = "class C {
    static {}
    {}
    void f() {
        if (a) {} else {}
        if (a) { /* nothing */ } else if (b) { ; }
        while (a) {}
        do {} while (a);
        for (;;) {}
        for (String s : x) {}
        try {} catch (Exception e) {} finally {}
        try (r) {}
        synchronized (this) {}
        switch (a) {}
        switch (a) { case 1: }
        Runnable r = () -> {};
        { }
    }
}
";
    assert_eq!(
        violations("S017", source),
        [
            "2:12 S017",
            "3:5 S017",
            "5:16 S017",
            "5:24 S017",
            "6:16 S017",
            "7:19 S017",
            "8:12 S017",
            "9:18 S017",
            "10:28 S017",
            "11:13 S017",
            "11:47 S017",
            "12:17 S017",
            "13:29 S017",
            "14:20 S017"
        ]
    );
}

#[test]
fn nested_block() {
    let source = "class C {
    C() { { } }
    void f() {
        { int a; }
        if (x) { { } }
        switch (x) {
            case 1: { break; }
        }
        switch (x) {
            case 2 -> { }
        }
        l: { }
        Runnable r = () -> { { } };
    }
    { }
}
";
    assert_eq!(lines_of("S018", source), [2, 4, 5, 7, 13]);
}

#[test]
fn null_check_before_instanceof() {
    let source = "class C {
    void f(Object x, Object y) {
        if (x != null && x instanceof String) {}
        if (null != x && x instanceof String s) {}
        if (x instanceof String && x != null) {}
        if (x != null && a && x instanceof String) {}
        if (x != null && (x instanceof String || x instanceof Integer)) {}
        if (x != null && x.isFoo() && x instanceof String) {}
        if (x != null || x instanceof String) {}
        if (y != null && x instanceof String) {}
        if (x != null && (a || x instanceof String)) {}
        if (x == null || !(x instanceof String)) {}
        if ((x != null) && x instanceof String) {}
        if (x != null && f(x instanceof String)) {}
    }
}
";
    assert_eq!(
        violations("S020", source),
        ["3:13 S020", "4:13 S020", "5:36 S020", "6:13 S020", "7:13 S020", "11:13 S020", "13:14 S020"]
    );
}

#[test]
fn null_check_before_instanceof_fix() {
    let source = "class C {
    boolean f(Object x) {
        boolean a = x != null && x instanceof String;
        boolean b = x instanceof Integer && x != null;
        boolean c = a && (x != null) && x instanceof Long;
        boolean d = x != null && a && x instanceof Short;
        return x != null && (x instanceof Byte || x instanceof Character);
    }
}
";
    assert_eq!(
        fixed("S020", source),
        "class C {
    boolean f(Object x) {
        boolean a = x instanceof String;
        boolean b = x instanceof Integer;
        boolean c = a && x instanceof Long;
        boolean d = x != null && a && x instanceof Short;
        return x != null && (x instanceof Byte || x instanceof Character);
    }
}
"
    );
}

#[test]
fn expression_lambda() {
    let source = "class C {
    void f() {
        g(x -> { return x + 1; });
        g(() -> { run(); });
        g(x -> {
            return x;
        });
        g(x -> { return; });
        g(x -> { int y = x; });
        g(x -> { a(); b(); });
        g(x -> { ; return x; });
        g(x -> x);
    }
}
";
    assert_eq!(violations("S021", source), ["3:13 S021", "4:14 S021", "5:13 S021"]);
}

#[test]
fn expression_lambda_fix() {
    let source = "class C {
    void f() {
        g(x -> { return x + 1; });
        g(() -> { return create(); });
        g((String s) -> { return s.trim(); });
        g(() -> { run(); });
        g(x -> { return x.trim(); });
        g(x -> { return x; /* same */ });
        g(x -> {
            return x + 2;
        });
    }
}
";
    assert_eq!(
        fixed("S021", source),
        "class C {
    void f() {
        g(x -> x + 1);
        g(() -> create());
        g((String s) -> s.trim());
        g(() -> { run(); });
        g(x -> { return x.trim(); });
        g(x -> { return x; /* same */ });
        g(x -> x + 2);
    }
}
"
    );
}
