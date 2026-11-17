//! The rules of group S that follow PMD rules (S004, S005, S010, S019) and their fixes.

mod common;
use common::{fixed, lines_of, messages};

#[test]
fn collapsible_if() {
    let source = "class C {
    void f(boolean a, boolean b) {
        if (a) {
            if (b) { g(); }
        }
        if (a)
            if (b) g();
        if (a) {
            if (b) { g(); } else { h(); }
        }
        if (a) {
            if (b) { g(); }
        } else { h(); }
        if (a) {
            g();
            if (b) { g(); }
        }
        if (a) {
            // why
            if (b) { g(); }
        }
    }
}
";
    assert_eq!(lines_of("S004", source), [4, 7, 20]);
    assert_eq!(
        messages("S004", "class C { void f() { if (a) { if (b) {} } } }"),
        ["This if statement could be combined with its parent"]
    );
}

#[test]
fn collapsible_if_fix() {
    let source = "class C {
    void f() {
        if (a || b) {
            if (c ? d : e) {
                g();
            }
        }
        if (a && b)
            if (x = y) g();
        if ((a || b)) { if (c) { if (d) g(); } }
        if (a) {
            // kept
            if (b) g();
        }
        if (a) { if (b) /* kept */ g(); }
    }
}
";
    assert_eq!(
        fixed("S004", source),
        "class C {
    void f() {
        if ((a || b) && (c ? d : e)) {
                g();
            }
        if (a && b && (x = y)) g();
        if ((a || b) && c && d) g();
        if (a) {
            // kept
            if (b) g();
        }
        if (a) { if (b) /* kept */ g(); }
    }
}
"
    );
}

#[test]
fn local_before_return() {
    let source = "class C {
    int f() {
        int x = g();
        return x;
    }
    int twice() {
        int x = g();
        h(x);
        return x;
    }
    int notNext() {
        int x = g();
        h();
        return x;
    }
    int annotated() {
        @SuppressWarnings(\"unused\") int x = g();
        return x;
    }
    int field() {
        return field;
    }
    int usedByNext() {
        int x = g(), y = x;
        return x;
    }
    int second() {
        int x = g(), y = 2;
        return (y);
    }
    int switched(int k) {
        switch (k) {
            case 1:
                int x = 1;
                return x;
            case 2:
                x = 2;
                return x;
        }
        return 0;
    }
}
";
    assert_eq!(lines_of("S005", source), [3, 28]);
    assert_eq!(
        messages("S005", "class C { int f() { int x = 1; return x; } }"),
        ["Consider simply returning the value vs storing it in local variable 'x'"]
    );
}

#[test]
fn local_before_return_fix() {
    let source = "class C {
    List<String> f() {
        List<String> result = new ArrayList<>();
        return result;
    }
    Object widened() {
        long x = 1;
        return x;
    }
    int[] array() {
        int[] a = {1, 2};
        return a;
    }
    int commented() {
        int x = g(); // why
        return x;
    }
    int several() {
        int y = 1, x = g();
        return x;
    }
    Supplier<String> lambda() {
        return () -> {
            String s = g();
            return s;
        };
    }
    String inferred() {
        var s = g();
        return s;
    }
    final String finalLocal() {
        final String s = g();
        return (s);
    }
}
";
    assert_eq!(
        fixed("S005", source),
        "class C {
    List<String> f() {
        return new ArrayList<>();
    }
    Object widened() {
        long x = 1;
        return x;
    }
    int[] array() {
        int[] a = {1, 2};
        return a;
    }
    int commented() {
        int x = g(); // why
        return x;
    }
    int several() {
        int y = 1, x = g();
        return x;
    }
    Supplier<String> lambda() {
        return () -> {
            String s = g();
            return s;
        };
    }
    String inferred() {
        var s = g();
        return s;
    }
    final String finalLocal() {
        return g();
    }
}
"
    );
}

#[test]
fn unnecessary_return() {
    let source = "class C {
    void f() {
        g();
        return;
    }
    void branches(boolean a) {
        if (a) {
            g();
            return;
        } else if (b) {
            return;
        }
    }
    void tries() {
        try {
            return;
        } catch (Exception e) {
            return;
        }
    }
    void loop() {
        while (a) {
            return;
        }
    }
    void notLast(boolean a) {
        if (a) {
            return;
        }
        g();
    }
    int value() {
        return 1;
    }
    void switches(int k) {
        switch (k) {
            case 1:
                return;
            case 2:
                g();
                return;
        }
    }
    void arrows(int k) {
        switch (k) {
            case 1 -> { return; }
            default -> g();
        }
    }
    Runnable lambda = () -> { return; };
    void expressionSwitch(int k) {
        int j = switch (k) {
            case 1 -> { return; }
            default -> 1;
        };
    }
}
";
    assert_eq!(lines_of("S010", source), [4, 9, 11, 16, 18, 41, 46, 50]);
    assert_eq!(messages("S010", "class C { void f() { return; } }"), ["Unnecessary return statement"]);
}

#[test]
fn unnecessary_return_fix() {
    let source = "class C {
    void f() {
        g();
        return;
    }
    void g(boolean a) {
        if (a) h(); else return;
    }
    void h(boolean a) {
        if (a) { h(); return; }
    }
    void i() {
        h();
        return; // done
    }
    void j() {
        return /* why */;
    }
    void k(int x) {
        switch (x) {
            case 1:
                return;
        }
    }
}
";
    assert_eq!(
        fixed("S010", source),
        "class C {
    void f() {
        g();
    }
    void g(boolean a) {
        if (a) h(); else return;
    }
    void h(boolean a) {
        if (a) { h(); }
    }
    void i() {
        h();
        // done
    }
    void j() {
        return /* why */;
    }
    void k(int x) {
        switch (x) {
            case 1:
        }
    }
}
"
    );
}

#[test]
fn unnecessary_instantiation() {
    let source = "class C {
    void f(String s, char[] chars, String[] names, int i, Object o) {
        a(new String());
        a(new String(\"x\"));
        a(new String(s));
        a(new String(\"x\" + i));
        a(new String(names[0]));
        a(new String(s.substring(1)));
        a(new String(chars));
        a(new String(o));
        a(new Integer(1));
        a(new java.lang.Long(1L));
        a(new Boolean(true));
        a(Boolean.valueOf(\"false\"));
        a(Boolean.valueOf(\"yes\"));
        a(Boolean.valueOf(o));
        a(new StringBuilder());
    }
}
";
    assert_eq!(lines_of("S019", source), [3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15]);
    assert_eq!(
        messages("S019", "class C { Object a = new Integer(1), b = new Boolean(x), c = Boolean.valueOf(true); }"),
        [
            "Do not use `new Integer(...)`, prefer `Integer.valueOf(...)`",
            "Do not use `new Boolean(...)`, prefer `Boolean.valueOf`",
            "Do not use `Boolean.valueOf(true)`, prefer `Boolean.TRUE`",
        ]
    );
}

#[test]
fn unnecessary_instantiation_of_other_classes() {
    let source = "import my.own.Boolean;
class Integer {
    Object a = new Integer(1);
    Object b = new Boolean(true);
    Object c = new java.lang.Integer(1);
}
";
    assert_eq!(lines_of("S019", source), [5]);
}

#[test]
fn unnecessary_instantiation_fix() {
    let source = "class C {
    void f(String s, double d) {
        a(new String());
        a(new String(\"x\").length());
        a(new String(s));
        a(new Integer(1).toString());
        a(new java.lang.Long(1L));
        a(new Boolean(true));
        a(new Boolean(\"false\"));
        a(new Boolean(s));
        a(Boolean.valueOf(true));
        a(Boolean.valueOf(\"yes\"));
        a(new Float(1.5f));
        a(new Float(d));
        a(new Float(1.5));
        a(x == new Integer(1));
        new String(\"statement\");
        a(new /* why */ Integer(1));
    }
}
";
    assert_eq!(
        fixed("S019", source),
        "class C {
    void f(String s, double d) {
        a(\"\");
        a(\"x\".length());
        a(new String(s));
        a(Integer.valueOf(1).toString());
        a(java.lang.Long.valueOf(1L));
        a(Boolean.TRUE);
        a(Boolean.FALSE);
        a(Boolean.valueOf(s));
        a(Boolean.TRUE);
        a(Boolean.valueOf(\"yes\"));
        a(Float.valueOf(1.5f));
        a(new Float(d));
        a(new Float(1.5));
        a(x == new Integer(1));
        new String(\"statement\");
        a(new /* why */ Integer(1));
    }
}
"
    );
}
