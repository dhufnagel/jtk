//! The import rules (group I).

mod common;
use common::{fixed, messages, violations};

#[test]
fn star_import() {
    let source = "import java.util.*;
import java.util.List;
import static java.lang.Math.*;
import java.
    io.*;
class A {}
";
    assert_eq!(violations("I001", source), ["1:17 I001", "3:29 I001", "5:7 I001"]);
    assert_eq!(messages("I001", source)[0], "Using the '.*' form of import should be avoided - java.util.*");
    assert_eq!(messages("I001", source)[2], "Using the '.*' form of import should be avoided - java.io.*");
}

#[test]
fn illegal_import() {
    let source = "import sun.misc.Unsafe;
import sun.misc.*;
import static sun.misc.Unsafe.getUnsafe;
import sunny.Foo;
import com.sun.Bar;
class A {}
";
    assert_eq!(violations("I002", source), ["1:1 I002", "2:1 I002", "3:1 I002"]);
    assert_eq!(messages("I002", source)[2], "Illegal import - sun.misc.Unsafe.getUnsafe");
}

#[test]
fn static_import() {
    let source = "import static java.lang.Math.max;
import static java.lang.Math.*;
import java.util.List;
class A {}
";
    assert_eq!(violations("I003", source), ["1:29 I003", "2:29 I003"]);
    assert_eq!(messages("I003", source)[0], "Using a static member import should be avoided - java.lang.Math.max");
}

#[test]
fn fully_qualified_type() {
    let source = "package p;
import java.util.Optional;
class A<T extends java.io.Closeable> extends java.util.ArrayList<java.util.List<? extends java.io.File>> {
    java.util.Map<String, Optional<String>> map;
    java.lang.String name;
    java.util.Map.Entry<String, String> entry;
    Object c = java.util.Random.class;
    void f(Object o) throws java.io.IOException {
        Object x = new java.util.HashMap<String, String>();
        Object y = (java.util.Set<String>) o;
        boolean b = o instanceof java.util.Collection;
        java.lang.System.out.println(java.util.Collections.emptyList());
        try { } catch (java.lang.IllegalStateException | java.io.UncheckedIOException e) { }
        if (o instanceof p.Point(int px)) { }
    }
    @java.lang.Deprecated void g() {}
}
";
    assert_eq!(
        violations("I005", source),
        [
            "3:55 I005",
            "3:75 I005",
            "4:14 I005",
            "5:14 I005",
            "8:36 I005",
            "9:33 I005",
            "10:30 I005",
            "11:43 I005",
            "13:33 I005",
            "13:65 I005",
            "14:27 I005",
        ]
    );
    assert_eq!(messages("I005", source)[0], "Unnecessary fully qualified type - java.util.ArrayList");
}

#[test]
fn fully_qualified_type_with_the_simple_name_taken() {
    let source = "package p;
import java.util.List;
import java.util.*;
class A {
    java.awt.List awtList;
    java.util.Date date;
    com.example.Set set;
    Set otherSet;
    java.util.function.Function<String, String> f;
    com.example.Function g;
    java.lang.Integer i;
    p.Logger logger;
    com.example.Logger otherLogger;
    Logger simpleLogger;
    com.example.A a;
}
";
    // `java.util.*` imports this `Date`; `Logger` is `p.Logger` and `com.example.Logger`.
    assert_eq!(violations("I005", source), ["6:14 I005", "11:14 I005"]);
}

#[test]
fn fully_qualified_type_with_an_unknown_jdk_package_imported_on_demand() {
    // Unlike Checkstyle, which does not find `javax.sql` types on its class path.
    let source = "import javax.sql.*;
import org.example.*;
class A {
    java.util.List list;
}
";
    assert!(violations("I005", source).is_empty());
}

#[test]
fn fully_qualified_type_fix() {
    let source = "package p;
import java.util.List;
import java.io.IOException;
class A<IOException> {
    java.util.List<String> list = new java.util.ArrayList<>();
    java.util.@Deprecated List<String> other;
    void f() throws java.io.IOException {}
    p.B b;
    java.lang.String s;
}
";
    assert_eq!(
        fixed("I005", source),
        "package p;
import java.util.List;
import java.io.IOException;
class A<IOException> {
    List<String> list = new java.util.ArrayList<>();
    java.util.@Deprecated List<String> other;
    void f() throws java.io.IOException {}
    B b;
    java.lang.String s;
}
"
    );
}
