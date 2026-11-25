//! Linter rules of group B (likely bugs) from batch 2: B035, B140, B142, B143, B150, B168, B205, B219, B241, B280.

mod common;
use common::{fixed, messages, unsafe_fixed, violations};

#[test]
fn equals_self_call() {
    let source = "class A {
    public boolean equals(Object o) { return equals(o); }
    class B { public boolean equals(Object other) { return o() && this.equals(other); } }
    class C { public boolean equals(java.lang.Object o) { return o instanceof C && equals((C) o); }
        boolean equals(C c) { return true; } }
    class D { public boolean equals(Object o) { return super.equals(o); } }
    class E { public boolean equals(Object o) { Runnable r = () -> equals(o); return false; } }
    class F { public boolean equals(Object o) { return new Object() { boolean f() { return equals(o); } }.f(); } }
    class G { public boolean equals(Object o) { o = unwrap(o); return equals(o); } }
    class H { public boolean equals(Foo o) { return equals(o); } }
    class I { public boolean equals(Object o) { return other.equals(o); } }
    class J { public boolean same(Object o) { return equals(o); } }
}
";
    assert_eq!(violations("B035", source), ["2:46 B035", "3:67 B035"]);
    assert_eq!(messages("B035", source)[0], "equals() calls itself");
}

#[test]
fn string_builder_char() {
    let source = "class A {
    StringBuilder a = new StringBuilder('a');
    StringBuffer b = new StringBuffer('\\n');
    Object c = new java.lang.StringBuilder('\"');
    Object d = new StringBuilder('\\'');
    Object e = new StringBuilder(\"a\");
    Object f = new StringBuilder(16);
    Object g = new StringBuilder(c);
    Object h = new StringBuilder('\\u0022');
}
";
    assert_eq!(violations("B140", source), ["2:23 B140", "3:22 B140", "4:16 B140", "5:16 B140", "9:16 B140"]);
    assert_eq!(messages("B140", source)[0], "new StringBuilder(char) sets the capacity, not the content");
    let expected = "class A {
    StringBuilder a = new StringBuilder(\"a\");
    StringBuffer b = new StringBuffer(\"\\n\");
    Object c = new java.lang.StringBuilder(\"\\\"\");
    Object d = new StringBuilder(\"'\");
    Object e = new StringBuilder(\"a\");
    Object f = new StringBuilder(16);
    Object g = new StringBuilder(c);
    Object h = new StringBuilder('\\u0022');
}
";
    assert_eq!(fixed("B140", source), expected);
    // Another StringBuilder.
    assert!(violations("B140", "import a.StringBuilder; class A { Object a = new StringBuilder('a'); }").is_empty());
    assert!(
        violations(
            "B140",
            "class A { class StringBuilder { StringBuilder(int i) {} } Object a = new StringBuilder('a'); }"
        )
        .is_empty()
    );
}

#[test]
fn regex_dot() {
    let source = "class A {
    void f(String s, Foo foo) {
        s.split(\".\");
        s.trim().split(\".\", 2);
        s.replaceAll(\".\", \"*\");
        s.replaceFirst(\".\", \"\");
        \"a.b\".split(\".\");
        s.split(\"\\\\.\");
        s.split(\",\");
        s.matches(\".\");
        foo.split(\".\");
        Splitter.split(\".\");
        s.replace(\".\", \"\");
    }
}
";
    assert_eq!(violations("B142", source), ["3:17 B142", "4:24 B142", "5:22 B142", "6:24 B142", "7:21 B142"]);
    assert_eq!(messages("B142", source)[0], "\".\" as the regular expression of split() matches any character");
    assert_eq!(fixed("B142", source), source);
    let fixed = unsafe_fixed("B142", source);
    assert!(fixed.contains("s.split(\"\\\\.\");\n        s.trim().split(\"\\\\.\", 2);"));
    assert!(fixed.contains("s.replaceAll(\"\\\\.\", \"*\");"));
}

#[test]
fn invalid_regex() {
    let source = "import java.util.regex.Pattern;
class A {
    void f(String s, Foo foo) {
        Pattern.compile(\"[\");
        Pattern.compile(\"(a|b\", Pattern.CASE_INSENSITIVE | Pattern.MULTILINE);
        Pattern.matches(\"*.txt\", s);
        s.split(\"\\\\\");
        s.replaceAll(\"${name}\", \"x\");
        s.replaceFirst(\"a)\", \"\");
        s.matches(\"a**\");
        java.util.regex.Pattern.compile(\"[z-a]\");
        Pattern.compile(\"[\", Pattern.LITERAL);
        Pattern.compile(\"[\", flags);
        Pattern.compile(\"[]a]|\\\\p{L}+|a{2}{3}|(?i)x|(?<n>a)\\\\k<n>|\\\\Q[\\\\E\");
        Pattern.compile(\"\"\"
            [\"\"\");
        foo.split(\"[\");
        foo.matches(\"*\");
        Other.compile(\"[\");
    }
}
";
    assert_eq!(
        violations("B143", source),
        ["4:25 B143", "5:25 B143", "6:25 B143", "7:17 B143", "8:22 B143", "9:24 B143", "10:19 B143", "11:41 B143"]
    );
    assert_eq!(
        messages("B143", source)[..4],
        [
            "Invalid regular expression: unclosed character class",
            "Invalid regular expression: unclosed group",
            "Invalid regular expression: dangling quantifier",
            "Invalid regular expression: unescaped trailing backslash"
        ]
    );
    // Pattern from another package.
    assert!(violations("B143", "import a.Pattern; class A { Object p = Pattern.compile(\"[\"); }").is_empty());
    assert!(violations("B143", "class A { Object p = Pattern.compile(\"[\"); }").is_empty());
    assert_eq!(violations("B143", "import java.util.regex.*; class A { Object p = Pattern.compile(\"(\"); }").len(), 1);
}

#[test]
fn suspicious_date_pattern() {
    let source = "import java.text.SimpleDateFormat;
import java.time.format.DateTimeFormatter;
class A {
    Object a = new SimpleDateFormat(\"YYYY-MM-dd\");
    Object b = DateTimeFormatter.ofPattern(\"dd.MM.YYYY 'Year' HH:mm\");
    Object c = new SimpleDateFormat(\"yyyy-MM-DD\", locale);
    Object d = DateTimeFormatter.ofPattern(\"YYYY-MM-DD\");
    Object e = new SimpleDateFormat(\"YYYY-'W'ww\");
    Object f = new SimpleDateFormat(\"yyyy-DDD\");
    Object g = new SimpleDateFormat(\"yyyy-MM-dd'Y''D'\");
    Object h = Other.ofPattern(\"YYYY\");
    Object i = new SimpleDateFormat(\"YYYY\\tMM\");
}
";
    assert_eq!(violations("B150", source), ["4:37 B150", "5:44 B150", "6:37 B150", "7:44 B150", "12:37 B150"]);
    assert_eq!(messages("B150", source)[0], "Week year \"Y\" in a date pattern without week \"w\"");
    assert_eq!(messages("B150", source)[2], "Day of year \"D\" in a date pattern with month \"M\"");
    let expected = "import java.text.SimpleDateFormat;
import java.time.format.DateTimeFormatter;
class A {
    Object a = new SimpleDateFormat(\"yyyy-MM-dd\");
    Object b = DateTimeFormatter.ofPattern(\"dd.MM.yyyy 'Year' HH:mm\");
    Object c = new SimpleDateFormat(\"yyyy-MM-dd\", locale);
    Object d = DateTimeFormatter.ofPattern(\"yyyy-MM-dd\");
    Object e = new SimpleDateFormat(\"YYYY-'W'ww\");
    Object f = new SimpleDateFormat(\"yyyy-DDD\");
    Object g = new SimpleDateFormat(\"yyyy-MM-dd'Y''D'\");
    Object h = Other.ofPattern(\"YYYY\");
    Object i = new SimpleDateFormat(\"YYYY\\tMM\");
}
";
    assert_eq!(fixed("B150", source), expected);
    assert!(violations("B150", "class A { Object a = new SimpleDateFormat(\"YYYY\"); }").is_empty());
}

#[test]
fn collection_with_itself() {
    let source = "class A {
    void f(java.util.List<String> list, java.util.Map<String, String> map, java.math.BigDecimal x) {
        list.addAll(list);
        list.removeAll(list);
        this.items.retainAll(this.items);
        list.containsAll((list));
        map.putAll(map);
        x.add(x);
        list.addAll(other);
        list.addAll(this.list);
        a.list.addAll(a.list);
    }
}
";
    assert_eq!(violations("B168", source), ["3:9 B168", "4:9 B168", "5:9 B168", "6:9 B168", "7:9 B168"]);
    assert_eq!(messages("B168", source)[0], "addAll() with its own receiver as argument");
}

#[test]
fn label_in_switch() {
    let source = "class A {
    void f(int k) {
        switch (k) {
            case 1:
                foo();
            caes2:
                bar();
                break;
            defualt:
                break;
            case 3:
                outer:
                for (;;) { for (;;) { break outer; } }
            case 4:
                unused:
                for (;;) {}
        }
        int y = switch (k) { case 1: lbl: yield 1; default: yield 2; };
        lbl: switch (k) { case 1 -> {} default -> {} }
        switch (k) { case 1 -> { inner: foo(); } default -> {} }
    }
}
";
    assert_eq!(violations("B205", source), ["6:13 B205", "9:13 B205", "15:17 B205", "18:38 B205"]);
    assert_eq!(messages("B205", source)[0], "Label \"caes2\" in a switch, a misspelt case or default?");
}

#[test]
fn exception_not_thrown() {
    let source = "class A {
    void a(int k) {
        if (k > 0) new IllegalArgumentException(\"k\");
        if (k < 0) {
            log();
            new IllegalStateException();
        }
        new MyException();
        foo();
    }
    void b() throws MyException {
        new MyException();
    }
    void c() {
        new java.io.IOException();
    }
    void d(int k) {
        if (k > 0) new RuntimeException(); else return;
        new AssertionError();
    }
    void e() {
        new ValidationError();
        new Exceptional();
        new IllegalStateException().printStackTrace();
        throw new IllegalStateException();
    }
    @Test void f() { new MyException(null); }
    void g() { try { new MyException(null); fail(); } catch (NullPointerException e) {} }
    void h() { Runnable r = () -> { new IllegalStateException(); }; }
    { new IllegalStateException(); }
}
";
    assert_eq!(
        violations("B219", source),
        ["3:20 B219", "6:13 B219", "8:9 B219", "12:9 B219", "15:9 B219", "18:20 B219", "19:9 B219", "30:7 B219"]
    );
    assert_eq!(messages("B219", source)[0], "IllegalArgumentException created but not thrown");
    assert_eq!(fixed("B219", source), source);
    let expected = "class A {
    void a(int k) {
        if (k > 0) throw new IllegalArgumentException(\"k\");
        if (k < 0) {
            log();
            throw new IllegalStateException();
        }
        new MyException();
        foo();
    }
    void b() throws MyException {
        throw new MyException();
    }
    void c() {
        new java.io.IOException();
    }
    void d(int k) {
        if (k > 0) new RuntimeException(); else return;
        throw new AssertionError();
    }
    void e() {
        new ValidationError();
        new Exceptional();
        new IllegalStateException().printStackTrace();
        throw new IllegalStateException();
    }
    @Test void f() { new MyException(null); }
    void g() { try { new MyException(null); fail(); } catch (NullPointerException e) {} }
    void h() { Runnable r = () -> { new IllegalStateException(); }; }
    { new IllegalStateException(); }
}
";
    assert_eq!(unsafe_fixed("B219", source), expected);
    // In a JUnit 3 test class, and an `Error` that is not `java.lang.Error`.
    assert!(violations("B219", "class T extends TestCase { void f() { new MyException(); } }").is_empty());
    assert!(violations("B219", "class Error {} class A { void f() { new Error(); } }").is_empty());
}

#[test]
fn serial_version_uid() {
    let source = "class A implements java.io.Serializable {
    private static final long serialVersionUID = 1L;
    static final long serialVersionUID = 1L;
    public static final long serialVersionUID = 1L;
    @Deprecated final static long serialVersionUID = 1L;
    private long serialVersionUID = 1L;
    static long serialVersionUID = -1;
    private static final int serialVersionUID = 1;
    long serialVersionUID = computed();
    transient long serialVersionUID = 1L;
    static final long /* id */ serialVersionUID = 1L;
    interface I { long serialVersionUID = 1L; }
    record R() { static final long serialVersionUID = 1L; }
    enum E { X; static final long serialVersionUID = 1L; }
    void f() { long serialVersionUID = 1L; }
}
";
    assert_eq!(
        violations("B241", source),
        ["3:5 B241", "4:5 B241", "5:5 B241", "6:5 B241", "7:5 B241", "8:5 B241", "9:5 B241", "10:5 B241", "11:5 B241"]
    );
    assert_eq!(messages("B241", source)[0], "serialVersionUID is not private static final long");
    let expected_safe = "class A implements java.io.Serializable {
    private static final long serialVersionUID = 1L;
    private static final long serialVersionUID = 1L;
    private static final long serialVersionUID = 1L;
    @Deprecated private static final long serialVersionUID = 1L;
    private long serialVersionUID = 1L;
    static long serialVersionUID = -1;
";
    assert!(fixed("B241", source).starts_with(expected_safe));
    let expected_unsafe = "    private static final long serialVersionUID = 1L;
    private static final long serialVersionUID = -1;
    private static final int serialVersionUID = 1;
    long serialVersionUID = computed();
    transient long serialVersionUID = 1L;
";
    assert!(unsafe_fixed("B241", source).contains(expected_unsafe));
    let assigned = "class A { long serialVersionUID = 1L; void f() { serialVersionUID = 2L; } }";
    assert_eq!(unsafe_fixed("B241", assigned), assigned);
}

#[test]
fn throw_null() {
    let source = "class A {
    void f() {
        throw null;
    }
    void g() {
        throw (null);
    }
    void h() {
        throw new NullPointerException();
    }
}
";
    assert_eq!(violations("B280", source), ["3:9 B280", "6:9 B280"]);
    assert_eq!(messages("B280", source)[0], "throw null throws a NullPointerException");
}
