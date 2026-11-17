//! The bug rules B005 to B021 that are Checkstyle checks with their default properties.

mod common;
use common::{lines_of, messages, violations};

#[test]
fn missing_switch_default() {
    let source = "class A {
    void f(Object o, int k) {
        switch (k) { case 1: break; }
        switch (k) { case 1: default: break; }
        switch (o) { case String s -> {} case Integer i -> {} }
        switch (o) { case null -> {} case Object x -> {} }
        switch (o) { case null, default -> {} }
        switch (o) { case String s when s.isEmpty() -> {} }
        int y = switch (k) { case 1 -> 2; default -> 3; };
        if (k > 0) switch (k) { case 1: break; }
        lbl: switch (k) { case 1: break; }
        Runnable r = () -> { switch (k) { case 2: break; } };
        switch (k) { case 1: switch (k) { case 2: break; } default: break; }
    }
    A(int k) { switch (k) { case 1 -> {} } }
}
";
    assert_eq!(
        violations("B005", source),
        ["3:9 B005", "8:9 B005", "10:20 B005", "11:14 B005", "12:30 B005", "13:30 B005", "15:16 B005"]
    );
    assert_eq!(messages("B005", source)[0], "switch without \"default\" clause");
}

#[test]
fn default_comes_last() {
    let source = "class A {
    void f(int k) {
        switch (k) { default: case 1: break; case 2: break; }
        switch (k) { case 1: break; default: break; case 2: break; }
        switch (k) { case 1 -> {} default -> {} case 2 -> {} }
        switch (k) { case 1: break; default: break; }
        switch (k) { case 1: default: case 2: break; }
        switch (k) {
            default:
            // shares the statements with case 3
            case 3:
                break;
        }
        int y = switch (k) { default -> 1; case 2 -> 2; };
    }
}
";
    assert_eq!(violations("B006", source), ["3:22 B006", "4:37 B006", "5:35 B006", "14:30 B006"]);
}

#[test]
fn equals_avoid_null() {
    let source = "class A {
    String field = \"x\";
    static String constant;
    String[] array;
    java.lang.String qualified;
    void f(Object o, int k, String param) {
        String local = \"a\";
        local.equals(\"b\");
        local.equalsIgnoreCase(\"b\");
        this.field.equals(\"b\");
        A.constant.equals(\"b\");
        array.equals(\"x\");
        qualified.equals(\"x\");
        param.equals(\"lit\" + k);
        param.equals(k + o);
        param.equals((param = \"x\"));
        param.equals(1);
        later.equals(\"x\");
        String later = \"y\";
        o.equals(\"x\");
        \"x\".equals(param);
        if (o instanceof String s && s.equals(\"y\")) {}
        java.util.function.Predicate<String> p = (String c) -> c.equals(\"x\");
        new Object() { String anonymous; void g() { anonymous.equals(\"b\"); } };
    }
    String later;
}
";
    // Line 18 refers to the field: the local variable is declared after it.
    assert_eq!(lines_of("B008", source), [8, 9, 10, 11, 12, 14, 15, 16, 18, 22, 23, 24]);
    assert_eq!(violations("B008", source)[0], "8:21 B008");
    assert_eq!(
        messages("B008", source)[1],
        "String literal expressions should be on the left side of an equalsIgnoreCase comparison"
    );
}

#[test]
fn inner_assignment() {
    let source = "class A {
    int x;
    void f(int a) {
        int c = a = 5;
        a = 1;
        while ((c = read()) != -1) {}
        while ((c = read()) != -1 && !(c = 2 > 3)) {}
        do { } while ((c = read()) >= 0);
        for (; (c = read()) < 0; c = 1) {}
        for (c = 0, a = 1; c < 3; c++, a = 2) {}
        if ((c = read()) == 1) {}
        if (c > 0) c = 2; else c = 3;
        lbl: c = 7;
        Runnable r = () -> x = 3;
        foo(c = 3);
        c = a = 2;
        switch (c) { case 1 -> c = 2; default -> c = 3; }
        int k = switch (c) { case 1 -> c = 2; default -> { c = 3; yield 2; } };
        if (c > 0) switch (c) { case 1 -> c = 2; default -> {} }
        boolean flag;
        if (flag = true) {}
    }
    int read() { return 0; }
    void foo(int i) {}
}
";
    assert_eq!(lines_of("B012", source), [4, 7, 11, 13, 15, 16, 18, 19, 21]);
    assert_eq!(violations("B012", source)[0], "4:19 B012");
}

#[test]
fn parameter_assignment() {
    let source = "class A {
    int x;
    void f(int a, int b, String... rest) {
        a = 1;
        b++;
        --a;
        a += 2;
        this.x = 3;
        rest = null;
        int c = 0;
        c = 1;
        Runnable r = () -> a = 4;
        java.util.function.IntUnaryOperator op = a2 -> a2 = 4;
        java.util.function.IntBinaryOperator op2 = (p, q) -> p++;
        Object o = new Object() { int y = a = 9; void g(int b) { b = 1; a = 2; } };
    }
    A(int z) { z = 1; }
    record R(int v) { R { v = 2; } }
}
";
    assert_eq!(lines_of("B013", source), [4, 5, 6, 7, 9, 13, 14, 15, 15, 17]);
    assert_eq!(violations("B013", source)[1], "5:10 B013");
    assert_eq!(messages("B013", source)[0], "Assignment of parameter 'a' is not allowed");
}

#[test]
fn illegal_catch() {
    let source = "class A {
    void f() {
        try { } catch (Exception e) { }
        try { } catch (java.lang.Throwable | Error t) { }
        try { } catch (IllegalStateException | java.lang.RuntimeException e) { }
        try { } catch (java.io.IOException e) { }
        try { } catch (com.example.Exception e) { }
    }
}
";
    assert_eq!(violations("B020", source), ["3:17 B020", "4:17 B020", "4:17 B020", "5:17 B020"]);
    assert_eq!(messages("B020", source)[1], "Catching 'java.lang.Throwable' is not allowed");
}

#[test]
fn illegal_throws() {
    let source = "class A {
    void f() throws RuntimeException, java.lang.Error, Exception, java.io.IOException {}
    A() throws Throwable {}
    @Override public String toString() throws RuntimeException { return \"\"; }
    protected void finalize() throws Throwable {}
    interface I { void g() throws Error; }
}
";
    assert_eq!(violations("B021", source), ["2:21 B021", "2:48 B021", "3:16 B021", "6:35 B021"]);
    assert_eq!(messages("B021", source)[0], "Throwing 'RuntimeException' is not allowed");
}
