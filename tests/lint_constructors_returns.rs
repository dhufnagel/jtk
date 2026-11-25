mod common;

use common::{fixed, lines_of, messages, unsafe_fixed};

// A020: public constructor in an abstract class.

#[test]
fn public_constructor_in_abstract_class() {
    let source = "\
public abstract class A {
    public A() {}

    @Deprecated
    public A(int x) {}

    protected A(long x) {}

    A(String s) {}

    private A(char c) {}
}
";
    assert_eq!(lines_of("A020", source), [2, 4]);
    assert_eq!(messages("A020", source), ["Public constructor in an abstract class"; 2]);
}

#[test]
fn public_constructor_in_concrete_class_or_method_in_abstract_class() {
    let source = "\
public class A {
    public A() {}

    abstract static class B {
        public void f() {}

        public abstract void g();
    }
}

interface I {
    abstract class C {}
}
";
    assert!(lines_of("A020", source).is_empty());
}

#[test]
fn public_constructor_in_abstract_class_fix_is_unsafe() {
    let source = "\
abstract class A {
    @Deprecated
    public A(int x) {
        this.x = x;
    }
}
";
    assert_eq!(fixed("A020", source), source);
    assert_eq!(
        unsafe_fixed("A020", source),
        "\
abstract class A {
    @Deprecated
    protected A(int x) {
        this.x = x;
    }
}
"
    );
}

#[test]
fn public_constructor_in_nested_abstract_class() {
    let source = "\
class Outer {
    abstract static class A {
        public A() {}
    }
}
";
    assert_eq!(lines_of("A020", source), [3]);
}

// S030: unnecessary constructor.

#[test]
fn unnecessary_constructor() {
    let source = "\
public class A {
    int x;

    public A() {}
}

class B {
    B() {
    }
}

class C {
    private static class D {
        private D() {}
    }

    protected class E {
        protected E() {}
    }
}
";
    assert_eq!(lines_of("S030", source), [4, 8, 14, 18]);
}

#[test]
fn unnecessary_constructor_fix_removes_lines() {
    let source = "\
public class A {
    int x;

    public A() {}
}
";
    assert_eq!(
        fixed("S030", source),
        "\
public class A {
    int x;

}
"
    );
    let source = "class B { B() {} }\n";
    assert_eq!(fixed("S030", source), "class B { }\n");
}

#[test]
fn necessary_constructors() {
    let source = "\
public class A {
    A() {}
}

class B {
    public B() {}
}

class C {
    C() {}

    C(int x) {}
}

class D {
    D(int x) {}
}

class E {
    E() {
        super();
    }
}

class F {
    F() throws Exception {}
}

class G {
    @Inject
    G() {}
}

class H {
    <T> H() {}
}

class I {
    /** Creates an I. */
    I() {}
}

class J {
    // For frameworks.
    J() {}
}

class K {
    K() {
        // Nothing to do.
    }
}

enum L {
    X;

    L() {}
}

record M(int x) {
    M {}
}

interface N {
    class O {
        O() {}
    }
}

@NoArgsConstructor
class P {
    P() {}
}

@lombok.AllArgsConstructor
class Q {
    int x;

    Q() {}
}

@Data
class R {
    R() {}
}
";
    assert!(lines_of("S030", source).is_empty());
}

#[test]
fn unnecessary_constructor_in_local_class() {
    let source = "\
class A {
    void f() {
        class L {
            L() {}
        }
    }
}
";
    assert_eq!(lines_of("S030", source), [4]);
    assert_eq!(
        messages("S030", source),
        ["Unnecessary constructor: the compiler generates the same default constructor"]
    );
}

// B174: null returned instead of an empty collection.

#[test]
fn return_null_collection() {
    let source = "\
import java.util.*;

class A {
    List<String> list() {
        return null;
    }

    Set<String> set() {
        if (x) {
            return (null);
        }
        return set;
    }

    public static Map<String, Object> map(String s) {
        if (s == null) {
            return null;
        }
        return parse(s);
    }

    Collection<String> collection() {
        return null;
    }

    java.util.List<String> qualified() {
        return null;
    }
}
";
    assert_eq!(lines_of("B174", source), [5, 10, 17, 23, 27]);
    assert_eq!(messages("B174", source)[0], "null returned instead of an empty collection");
}

#[test]
fn return_null_collection_fix() {
    let source = "\
import java.util.Collection;
import java.util.List;
import java.util.Map;

class A {
    List<String> list() {
        return null;
    }

    Map<String, Object> map() {
        return (null);
    }

    Collection<String> collection() {
        return null;
    }

    java.util.Set<String> set() {
        return null;
    }

    java.util.Collection<String> qualified() {
        return null;
    }
}
";
    assert_eq!(fixed("B174", source), source);
    assert_eq!(
        unsafe_fixed("B174", source),
        "\
import java.util.Collection;
import java.util.List;
import java.util.Map;

class A {
    List<String> list() {
        return List.of();
    }

    Map<String, Object> map() {
        return Map.of();
    }

    Collection<String> collection() {
        return List.of();
    }

    java.util.Set<String> set() {
        return java.util.Set.of();
    }

    java.util.Collection<String> qualified() {
        return java.util.List.of();
    }
}
"
    );
}

#[test]
fn return_null_collection_without_list_import_has_no_fix() {
    let source = "\
import java.util.Collection;

class A {
    Collection<String> collection() {
        return /* none */ null;
    }

    Collection<String> other() {
        return (/* none */ null);
    }
}
";
    assert_eq!(lines_of("B174", source), [5, 9]);
    assert_eq!(unsafe_fixed("B174", source), source);
}

#[test]
fn return_null_non_collection() {
    let source = "\
import java.util.List;
import java.util.function.Supplier;
import com.example.Set;

class A {
    String string() {
        return null;
    }

    Set<String> otherSet() {
        return null;
    }

    Map<String, String> unresolved() {
        return null;
    }

    String[] array() {
        return null;
    }

    List<String> notNull() {
        return list;
    }

    Supplier<List<String>> lambda() {
        return () -> {
            return null;
        };
    }

    Object anonymous() {
        return new Object() {
            String f() {
                return null;
            }
        };
    }

    List<String> fallsThrough() {
        Runnable r = () -> {
            return;
        };
        return x ? null : list;
    }

    @Nullable
    List<String> nullable() {
        return null;
    }

    @javax.annotation.CheckForNull
    List<String> checkForNull() {
        return null;
    }

    public @Nullable List<String> typeUse() {
        return null;
    }

    void none() {
        return;
    }
}
";
    assert!(lines_of("B174", source).is_empty());
}

#[test]
fn return_null_collection_declared_in_file() {
    let source = "\
import java.util.*;

class A {
    List<String> list() {
        return null;
    }

    static class List<T> {}
}
";
    assert!(lines_of("B174", source).is_empty());
}

#[test]
fn return_null_collection_in_anonymous_class_method() {
    let source = "\
import java.util.List;

class A {
    Object f() {
        return new Object() {
            List<String> g() {
                return null;
            }
        };
    }
}
";
    assert_eq!(lines_of("B174", source), [7]);
}
