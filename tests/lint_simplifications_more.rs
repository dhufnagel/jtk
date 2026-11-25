//! The rules S028, S036, S047, S056, S057, S059, S070, S102, S113, S129 and S137 and their fixes.

mod common;
use common::{fixed, messages, unsafe_fixed, violations};

#[test]
fn extends_object() {
    let source = "class A extends Object {}
class B extends java.lang.Object {}
class C<T extends Object, U extends Object & Comparable<U>, V extends Number> {}
class D extends Base {}
interface E<T extends Object> {}
";
    assert_eq!(violations("S028", source), ["1:9 S028", "2:9 S028", "3:11 S028", "5:15 S028"]);
    assert_eq!(messages("S028", "class A extends Object {}"), ["Unnecessary 'extends Object'"]);
    // A type named `Object` of the file or imported from elsewhere hides java.lang.Object.
    assert!(violations("S028", "import x.Object;\nclass A extends Object {}").is_empty());
    assert!(violations("S028", "class A extends Object {}\nclass Object {}").is_empty());
    assert!(violations("S028", "class A<Object> { class B extends Object {} }").is_empty());
}

#[test]
fn extends_object_fix() {
    let source = "class A extends Object {}
class C<T extends Object> {}
class D extends /* why */ Object {}
";
    assert_eq!(
        fixed("S028", source),
        "class A {}
class C<T> {}
class D extends /* why */ Object {}
"
    );
}

#[test]
fn duplicate_throws() {
    let source = "class C {
    void f() throws IOException, IOException {}
    void g() throws IOException, java.io.IOException, Exception {}
    void h() throws A, B, A, B {}
}
";
    assert_eq!(violations("S036", source), ["2:34 S036", "4:27 S036", "4:30 S036"]);
    assert_eq!(
        fixed("S036", source),
        "class C {
    void f() throws IOException {}
    void g() throws IOException, java.io.IOException, Exception {}
    void h() throws A, B {}
}
"
    );
    assert_eq!(fixed("S036", "class C { C() throws A, /* x */ A {} }"), "class C { C() throws A, /* x */ A {} }");
}

#[test]
fn static_via_instance() {
    let source = "class C {
    Thread worker;
    static int counter;
    int instance;
    static void util() {}
    void f(Thread t, String s, Integer i, C c, Runnable r) throws Exception {
        t.sleep(10);
        worker.currentThread();
        s.valueOf(1);
        i.MAX_VALUE;
        c.counter++;
        c.util();
        t.getName();
        s.length();
        i.toString(1);
        c.instance++;
        r.run();
        this.counter++;
    }
}
";
    assert_eq!(violations("S047", source), ["7:9 S047", "8:9 S047", "9:9 S047", "10:9 S047", "11:9 S047", "12:9 S047"]);
    assert_eq!(
        messages("S047", "class C { void f(Thread t) { t.sleep(1); } }"),
        ["Static member 'sleep' accessed via an instance; use 'Thread'"]
    );
}

#[test]
fn static_via_instance_negatives() {
    // An instance overload may be inherited from a supertype.
    assert!(violations("S047", "class C extends B { static void u() {} void f(C c) { c.u(); } }").is_empty());
    // Static and instance methods of the same name.
    assert!(violations("S047", "class C { static void u() {} void u(int a) {} void f(C c) { c.u(); } }").is_empty());
    // A class `Thread` of the file or imported from elsewhere.
    assert!(violations("S047", "import x.Thread;\nclass C { void f(Thread t) { t.sleep(1); } }").is_empty());
    assert!(violations("S047", "class C { class Thread {} void f(Thread t) { t.sleep(1); } }").is_empty());
    // A type parameter of the name.
    assert!(violations("S047", "class C { static int n; <C> void f(C c) { c.n++; } }").is_empty());
    // Inferred types and unknown declarations.
    assert!(
        violations("S047", "class C { void f() { var t = Thread.currentThread(); t.sleep(1); u.sleep(1); } }")
            .is_empty()
    );
    // Static methods of an interface cannot be called through an instance anyway.
    assert!(violations("S047", "interface I { static void u() {} } class C { void f(I i) {} }").is_empty());
    // A pattern variable of the same name may be in scope.
    assert!(
        violations("S047", "class C { Thread t; void f(Object o) { if (o instanceof String t) { t.valueOf(1); } } }")
            .is_empty()
    );
}

#[test]
fn static_via_instance_fix() {
    let source = "class C {
    enum E { A, B }
    void f(Thread t, E e) {
        t.sleep(10);
        E other = e.A;
    }
}
";
    assert_eq!(
        fixed("S047", source),
        "class C {
    enum E { A, B }
    void f(Thread t, E e) {
        Thread.sleep(10);
        E other = E.A;
    }
}
"
    );
}

#[test]
fn unnecessary_break() {
    let source = "class C {
    void f(int x) {
        switch (x) {
            case 1 -> {
                g();
                break;
            }
            case 2 -> {
                g();
                h();
                break;
            }
            case 3 -> {
                if (y) {
                    break;
                }
                g();
            }
            default -> {
                for (;;) {
                    break;
                }
            }
        }
    }
}
";
    assert_eq!(violations("S056", source), ["6:17 S056", "11:17 S056"]);
    let old_style = "class C { void f(int x) { switch (x) { case 1: g(); break; } } }";
    assert!(violations("S056", old_style).is_empty());
    let expression =
        "class C { int f(int x) { while (true) { int y = switch (x) { default -> { g(); yield 1; } }; } } }";
    assert!(violations("S056", expression).is_empty());
    assert_eq!(
        fixed("S056", source),
        "class C {
    void f(int x) {
        switch (x) {
            case 1 -> g();
            case 2 -> {
                g();
                h();
            }
            case 3 -> {
                if (y) {
                    break;
                }
                g();
            }
            default -> {
                for (;;) {
                    break;
                }
            }
        }
    }
}
"
    );
}

#[test]
fn unnecessary_continue() {
    let source = "class C {
    void f(List<String> list) {
        for (String s : list) {
            g(s);
            continue;
        }
        while (a()) {
            if (b()) {
                g();
                continue;
            } else {
                h();
            }
        }
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                if (c()) continue outer;
                g();
            }
            continue outer;
        }
        for (String s : list) {
            if (b()) {
                continue;
            }
            g();
        }
        while (a()) continue;
        do {
            switch (x) {
                case 1: continue;
            }
        } while (a());
        for (String s : list) {
            list.forEach(t -> { return; });
        }
    }
}
";
    assert_eq!(violations("S057", source), ["5:13 S057", "10:17 S057", "21:13 S057"]);
    assert_eq!(
        fixed("S057", source).lines().filter(|l| l.contains("continue")).collect::<Vec<_>>(),
        [
            "                if (c()) continue outer;",
            "                continue;",
            "        while (a()) continue;",
            "                case 1: continue;"
        ]
    );
    assert_eq!(violations("S057", "class C { void f() { while (a()) { if (b()) continue; } } }"), ["1:45 S057"]);
    assert_eq!(
        fixed("S057", "class C { void f() { while (a()) { if (b()) continue; } } }"),
        "class C { void f() { while (a()) { if (b()) continue; } } }"
    );
    // An outer loop's label continues that loop.
    assert!(
        violations("S057", "class C { void f() { o: while (a()) { while (b()) { continue o; } g(); } } }").is_empty()
    );
}

#[test]
fn unused_label() {
    let source = "class C {
    void f() {
        unused:
        for (;;) {
            break;
        }
        used: for (;;) {
            for (;;) {
                continue used;
            }
        }
        alsoUsed: {
            break alsoUsed;
        }
        inline: g();
    }
}
";
    assert_eq!(violations("S059", source), ["3:9 S059", "15:9 S059"]);
    assert_eq!(messages("S059", "class C { void f() { a: g(); } }"), ["Unused label 'a'"]);
    assert_eq!(
        fixed("S059", source),
        "class C {
    void f() {
        for (;;) {
            break;
        }
        used: for (;;) {
            for (;;) {
                continue used;
            }
        }
        alsoUsed: {
            break alsoUsed;
        }
        g();
    }
}
"
    );
}

#[test]
fn negated_comparison() {
    let source = "class C {
    void f(int i, long l, double d, Integer boxed, float f, Object o, boolean b) {
        g(!(i == 1));
        g(!(o != null));
        g(!!b);
        g(!(i < l));
        g(!(boxed >= 2 + i));
        g(!(d == 1.0));
        g(!(d < 1.0));
        g(!(i < f));
        g(!(i < unknown));
        g(!(b == true));
        g(!b);
        g(!(o instanceof String));
    }
}
";
    assert_eq!(
        violations("S070", source),
        ["3:11 S070", "4:11 S070", "5:11 S070", "6:11 S070", "7:11 S070", "8:11 S070"]
    );
    assert_eq!(
        fixed("S070", source),
        "class C {
    void f(int i, long l, double d, Integer boxed, float f, Object o, boolean b) {
        g(i != 1);
        g(o == null);
        g(b);
        g(i >= l);
        g(boxed < 2 + i);
        g(d != 1.0);
        g(!(d < 1.0));
        g(!(i < f));
        g(!(i < unknown));
        g(!(b == true));
        g(!b);
        g(!(o instanceof String));
    }
}
"
    );
    assert_eq!(messages("S070", "class C { boolean f(boolean b) { return !!b; } }"), ["Double negation"]);
}

#[test]
fn negated_comparison_precedence() {
    let source = "class C {
    void f(Object a, Object b, boolean c, String s) {
        g(c == !(a == b));
        g(c && !(a == b));
        g(s + !(a == b));
        g(!!!c);
        g(!!(a == b));
        g(!(/* why */ a == b));
    }
}
";
    assert_eq!(
        fixed("S070", source),
        "class C {
    void f(Object a, Object b, boolean c, String s) {
        g(c == (a != b));
        g(c && a != b);
        g(s + (a != b));
        g(!c);
        g((a == b));
        g(!(/* why */ a == b));
    }
}
"
    );
}

#[test]
fn cast_to_pattern_variable() {
    let source = "class C {
    Object field;
    void f(Object o, Object p) {
        if (o instanceof String s) {
            g(((String) o).length());
        }
        if (o instanceof String s && ((String) o).isEmpty()) {}
        int n = o instanceof Integer i ? (Integer) o + 1 : 0;
        if (o instanceof String s) {
            g((Integer) o);
        }
        if (!(o instanceof String s)) {
            g((String) o);
        }
        if (o instanceof String s || ((String) o).isEmpty()) {}
        if (field instanceof String s) {
            g((String) field);
        }
        if (p instanceof String s) {
            p = 1;
            g((String) p);
        }
        if (o instanceof String s) {
            new Runnable() { public void run() { g((String) o); } };
        }
    }
}
";
    assert_eq!(violations("S102", source), ["5:16 S102", "7:39 S102", "8:42 S102"]);
    assert_eq!(
        messages("S102", "class C { void f(Object o) { if (o instanceof String s) g((String) o); } }"),
        ["Cast can be replaced with pattern variable 's'"]
    );
    assert_eq!(
        fixed("S102", source).lines().skip(3).take(5).collect::<Vec<_>>(),
        [
            "        if (o instanceof String s) {",
            "            g(s.length());",
            "        }",
            "        if (o instanceof String s && s.isEmpty()) {}",
            "        int n = o instanceof Integer i ? i + 1 : 0;"
        ]
    );
}

#[test]
fn size_equals_zero() {
    let source = "import java.util.*;
import com.example.Bag;
class C {
    void f(List<String> list, Map<String, Integer> map, String s, Bag bag, StringBuilder sb, int[] array) {
        g(list.size() == 0);
        g(map.size() != 0);
        g(0 < list.size());
        g(s.length() == 0);
        g(list.size() >= 1);
        g(list.size() <= 0);
        g(list.size() == 1);
        g(bag.size() == 0);
        g(sb.length() == 0);
        g(s.size() == 0);
        g(list.size() > 1);
    }
}
";
    assert_eq!(
        violations("S113", source),
        ["5:11 S113", "6:11 S113", "7:11 S113", "8:11 S113", "9:11 S113", "10:11 S113"]
    );
    assert_eq!(
        fixed("S113", source).lines().skip(4).take(6).collect::<Vec<_>>(),
        [
            "        g(list.isEmpty());",
            "        g(!map.isEmpty());",
            "        g(!list.isEmpty());",
            "        g(s.isEmpty());",
            "        g(!list.isEmpty());",
            "        g(list.isEmpty());"
        ]
    );
    assert_eq!(
        messages("S113", "import java.util.List;\nclass C { boolean f(List<String> l) { return l.size() == 0; } }"),
        ["Use isEmpty() instead of comparing size() with 0"]
    );
    // A `List` of another package or of the same one.
    assert!(violations("S113", "import x.List;\nclass C { boolean f(List l) { return l.size() == 0; } }").is_empty());
    assert!(violations("S113", "class C { boolean f(List l) { return l.size() == 0; } }").is_empty());
    assert_eq!(
        violations("S113", "class C { boolean f(java.util.List<String> l) { return l.size() == 0; } }"),
        ["1:56 S113"]
    );
}

#[test]
fn identity_conversion() {
    let source = "class C {
    void f(String s, Integer i, Boolean b, int p, Object o) {
        g(String.valueOf(s));
        g(s.toString());
        g(Integer.valueOf(i));
        g(Boolean.valueOf(b));
        g(String.valueOf(p));
        g(Integer.valueOf(p));
        g(o.toString());
        g(Long.valueOf(i));
        g(i.toString());
    }
}
";
    assert_eq!(violations("S129", source), ["3:11 S129", "4:11 S129", "5:11 S129", "6:11 S129"]);
    assert_eq!(
        messages("S129", "class C { String f(String s) { return s.toString(); } }"),
        ["Unnecessary toString() of a value that is a String already"]
    );
    // Unsafe: null values.
    assert_eq!(fixed("S129", source), source);
    assert_eq!(
        unsafe_fixed("S129", source).lines().skip(2).take(4).collect::<Vec<_>>(),
        ["        g(s);", "        g(s);", "        g(i);", "        g(b);"]
    );
    assert!(violations("S129", "class C { class String {} void f(String s) { g(s.toString()); } }").is_empty());
}

#[test]
fn array_creation_in_initializer() {
    let source = "class C {
    int[] a = new int[] {1, 2};
    String[][] b = new String[][] {{\"x\"}};
    Object[] c = new String[] {\"x\"};
    void f() {
        var d = new int[] {1};
        int[] e = new int[3];
        g(new int[] {1});
        int[] h = new int[] {1}, i = {2};
        int j[] = new int[] {1};
        int[] k = new /* why */ int[] {1};
    }
}
";
    // A comment in the creation is not compared.
    assert_eq!(violations("S137", source), ["2:15 S137", "3:20 S137", "9:19 S137"]);
    assert_eq!(
        fixed("S137", source),
        "class C {
    int[] a = {1, 2};
    String[][] b = {{\"x\"}};
    Object[] c = new String[] {\"x\"};
    void f() {
        var d = new int[] {1};
        int[] e = new int[3];
        g(new int[] {1});
        int[] h = {1}, i = {2};
        int j[] = new int[] {1};
        int[] k = new /* why */ int[] {1};
    }
}
"
    );
}
