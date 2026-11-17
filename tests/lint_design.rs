//! The rules of group A: design and structure of types and files.

mod common;
use common::{file_violations, lines_of, messages};

#[test]
fn utility_class_constructor() {
    let source = r#"class Utility {
    public static void f() {}
}
class WithPrivateConstructor {
    private WithPrivateConstructor() {}
    static int x;
}
class WithProtectedConstructor {
    protected WithProtectedConstructor() {}
    public static void f() {}
}
class WithPublicConstructor {
    public WithPublicConstructor() {}
    static void f() {}
}
class Instance {
    int x;
    static void f() {}
}
class OnlyPrivateStatic {
    private static void f() {}
}
abstract class Abstract {
    static void f() {}
}
class Extends extends Object {
    static void f() {}
}
class Outer {
    void g() {}
    static class Nested {
        static void f() {}
    }
    @Deprecated
    class Inner {
        static final int X = 1;
    }
}
"#;
    assert_eq!(lines_of("A001", source), [1, 12, 34]);
}

#[test]
fn final_class() {
    let source = r#"class A {
    private A() {}
    class B {
        private B() {}
    }
    class C {
        private C() {}
    }
    class D extends C {}
    private class E {}
    private class F {}
    Object f = new F() {};
    final class G {
        private G() {}
    }
    abstract class H {
        private H() {}
    }
    class I {
        private I() {}
        I(int x) {}
    }
    enum J {
        X;
        private J() {}
    }
}
"#;
    assert_eq!(lines_of("A002", source), [1, 3, 10]);
    assert_eq!(messages("A002", source)[0], "Class A should be declared as final");
}

#[test]
fn visibility_modifier() {
    let source = r#"import org.junit.Rule;

class A {
    public int a;
    protected int b;
    int c, d;
    private int e;
    public static final int F = 1;
    static final int G = 1;
    public static int h;
    public final int i = 1;
    public static final long serialVersionUID = 1L;
    public long serialVersionUID2;
    @Rule public int rule;
    @com.google.common.annotations.VisibleForTesting int visible;
    @VisibleForTesting int visible2;
    Object o = new Object() { int anonymous; };
    void f() { int local; }
    interface I { int X = 1; }
    record R(int x) { static int y; }
}
"#;
    assert_eq!(lines_of("A003", source), [4, 5, 6, 6, 10, 11, 13, 17, 17, 20]);
    let other_rule = "import my.Rule;\nclass A {\n    @Rule int a;\n}\n";
    assert_eq!(lines_of("A003", other_rule), [3]);
}

#[test]
fn mutable_exception() {
    let source = r#"class FooException extends Exception {
    private int code;
    private final int finalCode = 1;
    static int count;
    int a, b;
    class Nested {
        int notChecked;
    }
    interface Inner {
        int CONSTANT = 1;
    }
    void f() {
        int local = 1;
        Object o = new Object() { int anonymous; };
    }
}
class BarError extends java.lang.Error {
    int mutable;
}
class NotAnExceptionClass extends Exception {
    int mutable;
}
class GenericException extends Base<String> {
    int mutable;
}
class Base<T> {}
class OtherException extends RuntimeException {
    class InnerException extends Exception {
        int mutable;
    }
}
"#;
    assert_eq!(lines_of("A004", source), [2, 4, 5, 5, 10, 14, 18, 29]);
}

#[test]
fn constant_interface() {
    let source = r#"interface Constants {
    int A = 1;
}
interface Marker {
}
interface Type {
    int A = 1;
    void f();
}
interface WithType {
    int A = 1;
    class Nested {}
}
@interface Annotation {
    int A = 1;
}
"#;
    assert_eq!(lines_of("A005", source), [1, 10]);
}

#[test]
fn one_top_level_class() {
    let source = "class First {}\ninterface Second {}\nenum Third { X }\n";
    assert_eq!(lines_of("A006", source), [2, 3]);
    let with_public = "class First {}\npublic class Second {}\nrecord Third() {}\n";
    assert_eq!(lines_of("A006", with_public), [1, 3]);
    assert_eq!(messages("A006", with_public)[0], "Top-level class First has to reside in its own source file");
}

#[test]
fn package_directory() {
    let source = "package a.b;\nclass C {}\n";
    assert_eq!(file_violations("A007", "src/a/b/C.java", source), Vec::<String>::new());
    assert_eq!(file_violations("A007", "./src/x/y/C.java", source), ["1:1 A007"]);
    let missing = "// comment\nimport java.util.List;\nclass C {}\n";
    assert_eq!(file_violations("A007", "src/C.java", missing), ["2:1 A007"]);
    let module = "module m {}\n";
    assert_eq!(file_violations("A007", "src/module-info.java", module), Vec::<String>::new());
    let annotated = "@Deprecated\npackage a.b;\n";
    assert_eq!(file_violations("A007", "x/package-info.java", annotated), ["2:1 A007"]);
}

#[test]
fn overload_order() {
    let source = r#"class A {
    A() {}
    void f() {}
    void f(int x) {}
    void g() {}
    void f(String s) {}
    A(int x) {}
    void h() {}
    ;
    void h(int x) {}
    A(String s) {}
    Object o = new Object() {
        void i() {}
        int x;
        void i(int y) {}
    };
    interface I {
        void j();
        void k();
        void j(int x);
    }
    enum E {
        X {
            void l() {}
            void m() {}
            void l(int x) {}
        };
        E() {}
        void n() {}
        E(int x) {}
    }
    record R(int x) {
        R {}
        void o() {}
        R(String s) { this(1); }
    }
}
"#;
    assert_eq!(lines_of("A008", source), [6, 7, 10, 11, 15, 20, 30, 35]);
    assert_eq!(
        messages("A008", source)[0],
        "All overloaded methods should be placed next to each other. Previous overloaded method located at line 4"
    );
    assert_eq!(
        messages("A008", source)[1],
        "Constructors should be grouped together. The last grouped constructor is declared at line 2"
    );
}

#[test]
fn declaration_usage_distance() {
    let source = r#"class A {
    void f() {
        int a = 1;
        g();
        g();
        g();
        g(a);
    }
    void g() {
        int a = 1;
        g();
        g();
        g();
        g();
        g(a);
        final int b = 1;
        g();
        g();
        g();
        g();
        g(b);
    }
    void h() {
        int a = 1;
        int x = 2;
        int y = 3;
        g();
        g();
        g();
        if (a > 0) {}
    }
    void i() {
        int a = 1;
        g();
        g();
        g();
        g();
        { g(a); }
    }
    void j(Object o) {
        int a = 1;
        o.g();
        o.g();
        o.g();
        o.g();
        o.h(a);
    }
    void k() {
        int a = 1, b = 2;
        g();
        g();
        g();
        g();
        g(a, b);
    }
    void g(int... x) {}
}
"#;
    assert_eq!(lines_of("A014", source), [3, 10, 24, 49, 49]);
    assert_eq!(
        messages("A014", source)[0],
        "Distance between variable 'a' declaration and its first usage is 4, but allowed 3"
    );
}

#[test]
fn magic_number() {
    let source = r#"class A {
    static final int OK = 3 * 4 + (5 << 2);
    static final int[] ARRAY = new int[] {3, 4};
    static final int[] SIZED = new int[5];
    static final Runnable LAMBDA = () -> g(6);
    static final int CALL = Math.max(7, 8);
    static final int CHAINED = foo(9).length();
    static final boolean AND = OK > 10 && OK < 11;
    int field = 12;
    final int finalField = -13;
    int ignored = -1 + 0 + 1 + 2 + 2L + 2.0 + 0x1 + -0.0;
    int hex = 0xffffffff;
    long hexLong = 0x80000000;
    void f(int x) {
        final int local = 14;
        int local2 = 15;
        switch (x) { case 16: break; default: }
        int[] array = {17};
        g(x + 18);
    }
    enum E { X(19), Y(20) { void f() { g(21); } }; E(int x) {} void f() {} }
    interface I { int X = 22; default void f() { int y = 23; } }
    @interface Q { int value() default 24; }
    @Size(max = 25) String annotated;
    static String foo(int x) { return ""; }
    static void g(int x) {}
}
"#;
    assert_eq!(lines_of("A015", source), [4, 5, 7, 8, 8, 9, 11, 13, 16, 17, 18, 19, 21, 24]);
    assert_eq!(messages("A015", source)[6], "'-0.0' is a magic number");
}

#[test]
fn outer_type_filename() {
    let source = "class Other {}\n";
    assert_eq!(file_violations("A017", "src/a/A.java", source), ["1:1 A017"]);
    assert_eq!(file_violations("A017", "src/a/Other.java", source), Vec::<String>::new());
    let with_public = "class First {}\npublic class Second {}\n";
    assert_eq!(file_violations("A017", "src/a/A.java", with_public), Vec::<String>::new());
    let nested = "class A {}\nclass B { public class C {} }\n";
    assert_eq!(file_violations("A017", "src/a/C.java", nested), ["1:1 A017"]);
}
