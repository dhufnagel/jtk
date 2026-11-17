//! Linter rules modelled on PMD rules.

mod common;
use common::{lines_of, messages};

#[test]
fn suspicious_octal_escape() {
    let source = r#"class A {
    String a = "\128";
    String b = "\0123 \0127";
    String c = "\12a \123 \\128 \7";
    String d = "\48";
    char e = '\18';
}
"#;
    assert_eq!(lines_of("B018", source), [2, 3, 3, 5]);
    assert_eq!(
        messages("B018", source)[0],
        r"Suspicious decimal characters following octal escape in string literal: \12 + 8"
    );
}

#[test]
fn print_stack_trace() {
    let source = "class A {
    void f(Exception e) {
        e.printStackTrace();
        e.printStackTrace(System.err);
        System.out.println(e);
        System.err.print(1);
        java.lang.System.out.printf(\"%s\", e);
        System.out.flush();
        out.println(e);
    }
}
";
    assert_eq!(lines_of("B022", source), [3, 5, 6, 7]);
    assert_eq!(
        messages("B022", source)[..2],
        ["Avoid printStackTrace(); use a logger call instead.", "Usage of System.out/err"]
    );
}

#[test]
fn unused_parameter() {
    let source = "class A {
    private void unused(int a, int b, int ignoredC, int unusedD, int _) { f(b); }
    private A(int a, String... rest) { this.a = a; }
    public void notPrivate(int a) {}
    private void written(int a, int b, int c, int d) { a = 1; b++; c += 1; g(d++); }
    private void inLambda(int a, int b) { run(() -> a); run(new Object() { int f() { return b; } }); }
    private void readObject(ObjectInputStream in) throws InvalidObjectException {}
    private void receiver(A this) {}
    private native void undeclared(int a);
}
";
    assert_eq!(lines_of("B028", source), [2, 3, 5, 5, 5]);
    assert_eq!(
        messages("B028", source)[..2],
        ["Avoid unused method parameters such as 'a'.", "Avoid unused constructor parameters such as 'rest'."]
    );
}

#[test]
fn throws_exception() {
    let source = "import org.junit.Test;
import org.junit.jupiter.api.*;
class A implements Callable<String> {
    A() throws Exception {}
    void f() throws IOException, Exception {}
    void g() throws java.lang.Exception {}
    void h() throws IOException {}
    @Override public void run() throws Exception {}
    public String call() throws Exception { return null; }
    public boolean equals(Object o) throws Exception { return false; }
    @Test public void test() throws Exception {}
    @BeforeEach void setUp() throws Exception {}
    Object action = new PrivilegedExceptionAction<Object>() { public Object run() throws Exception { return null; } };
}
";
    assert_eq!(lines_of("A010", source), [4, 5, 6, 12]);
    assert_eq!(messages("A010", source)[0], "A method/constructor should not explicitly throw java.lang.Exception");
}

#[test]
fn abstract_without_abstract() {
    let source = "abstract class A {
    void f() {}
    abstract class B { abstract void g(); }
    abstract class C extends A {}
    abstract class D implements Runnable {}
    interface E { void h(); }
    abstract static class F { static abstract class G {} }
}
";
    assert_eq!(lines_of("A011", source), [1, 7, 7]);
    assert_eq!(messages("A011", source)[0], "This abstract class does not have any abstract methods");
}

#[test]
fn cognitive_complexity() {
    let source = "class A {
    int f(int a, boolean b, boolean c) {
        if (a > 0) {                         // +1
            for (int i = 0; i < a; i++) {    // +2 (nesting 1)
                while (b && c || !b) {}      // +3 (nesting 2), +1 for ||, +1 for &&
            }
        } else if (a < 0) {                  // +1
            return a > 1 ? 1 : 2;            // +2 (nesting 1)
        } else {                             // +1, nests again after the else if
            outer: do { continue outer; }    // +3 (nesting 2), +1 for the label
            while (c);
        }
        return f(a - 1, b, c);               // +1 for the recursion
    }
    int g(int a) {
        Runnable r = () -> { if (a > 0) {} };
        try {} catch (RuntimeException e) {}
        switch (a) { case 1 -> f(1, true, false); }
        return a;
    }
}
";
    assert_eq!(lines_of("C022", source), [2]);
    assert_eq!(
        messages("C022", source),
        ["The method 'f(int, boolean, boolean)' has a cognitive complexity of 17, current threshold is 15"]
    );
}

#[test]
fn unused_parameter_suppressed_as_unused() {
    let source =
        "class A {\n    @SuppressWarnings(\"unused\") private void f(int a) {}\n    private void g(int b) {}\n}\n";
    assert_eq!(lines_of("B028", source), [3]);
}
