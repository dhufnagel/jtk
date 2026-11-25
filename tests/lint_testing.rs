//! The rules of group T: test code (JUnit, TestNG, AssertJ, Mockito).

mod common;
use common::{fixed, lines_of, messages, unsafe_fixed};

#[test]
fn test_without_annotation() {
    let source = r#"import org.junit.Test;

public class FooTest {
    @Test
    public void annotated() {}

    public void testFoo() {}

    public void testHelper() {}

    void testPackagePrivate() {}

    public void test_underscore() {}

    public void testing() {}

    public int testReturns() { return 1; }

    public void testWithParameter(int x) {}

    public static void testStatic() {}

    @Deprecated
    public void testAnnotated() {}

    private void check() {
        testHelper();
    }
}
"#;
    assert_eq!(lines_of("T001", source), [7, 13]);
    assert_eq!(messages("T001", source)[0], "Test method without @Test is not run");
    let result = unsafe_fixed("T001", source);
    assert!(result.contains("@Test public void testFoo() {}"), "{result}");
    assert!(result.contains("@Test public void test_underscore() {}"), "{result}");
    assert_eq!(fixed("T001", source), source);
}

#[test]
fn test_without_annotation_junit5_and_negatives() {
    let source = r#"import org.junit.jupiter.api.Test;

class FooTest {
    @Test
    void annotated() {}

    void testFoo() {}

    private void testPrivate() {}
}
"#;
    assert_eq!(lines_of("T001", source), [7]);
    let no_tests = r#"import org.junit.Test;

public class Helper {
    public void testFoo() {}
}
"#;
    assert!(lines_of("T001", no_tests).is_empty());
    let local_annotation = r#"public class FooTest {
    @Test
    public void annotated() {}

    public void testFoo() {}
}
"#;
    assert!(lines_of("T001", local_annotation).is_empty());
    let junit3 = r#"import junit.framework.TestCase;
import org.junit.Test;

public class FooTest extends TestCase {
    @Test
    public void annotated() {}

    public void testFoo() {}
}
"#;
    assert!(lines_of("T001", junit3).is_empty());
    let qualified = r#"public class FooTest {
    @org.junit.jupiter.api.Test
    void annotated() {}

    void testFoo() {}
}
"#;
    assert_eq!(lines_of("T001", qualified), [5]);
    assert!(unsafe_fixed("T001", qualified).contains("@org.junit.jupiter.api.Test void testFoo() {}"));
}

#[test]
fn non_static_class_fixture() {
    let source = r#"import org.junit.AfterClass;
import org.junit.BeforeClass;

public class FooTest {
    private static int counter;
    private int instance;

    @BeforeClass
    public void init() {
        counter = 1;
    }

    @AfterClass
    public void done() {
        instance = 0;
    }

    @BeforeClass
    public static void fine() {}

    @org.junit.jupiter.api.BeforeAll
    void perClass() {}
}
"#;
    assert_eq!(lines_of("T004", source), [8, 13]);
    assert_eq!(messages("T004", source), ["@BeforeClass method is not static", "@AfterClass method is not static"]);
    let result = fixed("T004", source);
    assert!(result.contains("public static void init()"), "{result}");
    assert!(result.contains("public void done()"), "{result}");
    let other_annotation = r#"import com.example.BeforeClass;

public class FooTest {
    @BeforeClass
    public void init() {}
}
"#;
    assert!(lines_of("T004", other_annotation).is_empty());
}

#[test]
fn malformed_test() {
    let source = r#"import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;

class FooTest {
    @Test
    private void hidden() {}

    @Test
    static void shared() {}

    @Test
    int returns() { return 1; }

    @Test
    void withParameter(org.junit.jupiter.api.TestInfo info) {}

    @BeforeEach
    private void setUp() {}

    @BeforeAll
    static void setUpAll() {}

    @TestFactory
    java.util.stream.Stream<Object> factory() { return null; }

    @Test
    void fine() {}
}
"#;
    assert_eq!(lines_of("T005", source), [7, 10, 13, 19]);
    assert_eq!(
        messages("T005", source),
        [
            "@Test method is private",
            "@Test method is static",
            "@Test method does not return void",
            "@BeforeEach method is private"
        ]
    );
    let result = unsafe_fixed("T005", source);
    assert!(result.contains("@Test\n    void hidden() {}"), "{result}");
    assert!(result.contains("@Test\n    void shared() {}"), "{result}");
    assert_eq!(fixed("T005", source), source);
}

#[test]
fn malformed_test_junit4() {
    let source = r#"import org.junit.Before;
import org.junit.Test;

public class FooTest {
    @Test
    void packagePrivate() {}

    @Test
    protected void isProtected() {}

    @Test
    public void withParameter(int x) {}

    @Before
    public static void setUp() {}

    @Test
    public void fine() {}
}
"#;
    assert_eq!(lines_of("T005", source), [5, 8, 11, 14]);
    let result = unsafe_fixed("T005", source);
    assert!(result.contains("public void packagePrivate()"), "{result}");
    assert!(result.contains("public void isProtected()"), "{result}");
    assert!(result.contains("public void setUp()"), "{result}");
    let runner = r#"import org.junit.Test;
import org.junit.runner.RunWith;

@RunWith(JUnitParamsRunner.class)
public class FooTest {
    @Test
    public void withParameter(int x) {}
}
"#;
    assert!(lines_of("T005", runner).is_empty());
}

#[test]
fn mixed_junit_versions() {
    let source = r#"import org.junit.Before;
import org.junit.Rule;
import org.junit.jupiter.api.Test;

class FooTest {
    @Rule
    public TemporaryFolder folder = new TemporaryFolder();

    @Before
    void setUp() {}

    @Test
    void test() {}
}
"#;
    assert_eq!(lines_of("T009", source), [6, 9]);
    assert_eq!(messages("T009", source)[0], "JUnit 4 annotation in a JUnit 5 test class");
    let junit4 = r#"import org.junit.Test;
import org.junit.jupiter.api.BeforeEach;

public class FooTest {
    @BeforeEach
    public void setUp() {}

    @Test
    public void test() {}
}
"#;
    assert_eq!(messages("T009", junit4), ["JUnit 5 annotation in a JUnit 4 test class"]);
    let junit3 = r#"import junit.framework.TestCase;
import org.junit.Test;

public class FooTest extends TestCase {
    @Test
    public void test() {}
}
"#;
    assert_eq!(messages("T009", junit3), ["JUnit 4 annotation in a JUnit 3 test class"]);
    let migration = r#"import org.junit.Rule;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.migrationsupport.rules.EnableRuleMigrationSupport;

@EnableRuleMigrationSupport
class FooTest {
    @Rule
    public TemporaryFolder folder = new TemporaryFolder();

    @Test
    void test() {}
}
"#;
    assert!(lines_of("T009", migration).is_empty());
    let consistent = r#"import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;

@RunWith(JUnitPlatform.class)
class FooTest {
    @BeforeEach
    void setUp() {}

    @Test
    void test() {}
}
"#;
    assert!(lines_of("T009", consistent).is_empty());
}

#[test]
fn swallowed_assertion() {
    let source = r#"import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

public class FooTest {
    void a() {
        try {
            f();
            fail("expected an exception");
        } catch (Throwable t) {
        }
    }

    void b() {
        try {
            assertEquals(1, f());
        } catch (AssertionError | RuntimeException e) {
            log(e.getMessage());
        }
    }

    void exception() {
        try {
            f();
            fail();
        } catch (Exception e) {
        }
    }

    void rethrows() {
        try {
            fail();
        } catch (AssertionError e) {
            throw new IllegalStateException(e);
        }
    }

    void keeps() {
        try {
            fail();
        } catch (AssertionError e) {
            last = e;
        }
    }

    void retries() {
        for (int i = 0; i < 3; i++) {
            try {
                fail();
            } catch (AssertionError e) {
                sleep();
            }
        }
    }

    void lambda() {
        try {
            run(() -> fail());
        } catch (Error e) {
        }
    }
}
"#;
    assert_eq!(lines_of("T025", source), [8, 15]);
    assert_eq!(messages("T025", source)[0], "Assertion failure caught by the enclosing catch");
    let project_fail = r#"public class FooTest {
    void a() {
        try {
            fail();
        } catch (Throwable t) {
        }
    }

    void fail() {}
}
"#;
    assert!(lines_of("T025", project_fail).is_empty());
    let assertj = r#"import static org.assertj.core.api.Assertions.assertThat;

class FooTest {
    void a() {
        try {
            assertThat(f()).isEqualTo(1);
        } catch (AssertionError e) {
        }
    }
}
"#;
    assert_eq!(lines_of("T025", assertj), [6]);
}

#[test]
fn assert_argument_order() {
    let source = r#"import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

class FooTest {
    void test() {
        assertEquals(result, 42);
        assertEquals(list.size(), 3, "size");
        assertEquals(42, result);
        assertSame(value, Color.RED);
        assertEquals(1, 2);
        assertEquals(expectedSize, 3);
        assertEquals(a, b);
        assertEquals(result, -1);
    }
}
"#;
    assert_eq!(lines_of("T032", source), [6, 7, 9, 13]);
    assert_eq!(messages("T032", source)[0], "assertEquals() with the expected and actual values swapped");
    let result = fixed("T032", source);
    assert!(result.contains("assertEquals(42, result);\n        assertEquals(3, list.size(), \"size\");"), "{result}");
    assert!(result.contains("assertSame(Color.RED, value);"), "{result}");
    assert!(result.contains("assertEquals(-1, result);"), "{result}");
}

#[test]
fn assert_argument_order_junit4_and_testng() {
    let junit4 = r#"import org.junit.Assert;

public class FooTest {
    public void test() {
        Assert.assertEquals("message", result, 42);
        Assert.assertEquals(result, 1.5, 0.01);
        Assert.assertEquals(message, result, 42);
    }
}
"#;
    assert_eq!(lines_of("T032", junit4), [5, 6]);
    assert!(fixed("T032", junit4).contains("Assert.assertEquals(\"message\", 42, result);"));
    let testng = r#"import static org.testng.Assert.assertEquals;

public class FooTest {
    public void test() {
        assertEquals(42, result);
        assertEquals(result, 42);
    }
}
"#;
    assert_eq!(lines_of("T032", testng), [5]);
    assert!(fixed("T032", testng).contains("assertEquals(result, 42);\n        assertEquals(result, 42);"));
    let project = r#"public class FooTest {
    public void test() {
        assertEquals(result, 42);
    }

    static void assertEquals(Object a, Object b) {}
}
"#;
    assert!(lines_of("T032", project).is_empty());
    let unknown_wildcard = r#"import static com.example.Checks.*;

public class FooTest {
    public void test() {
        assertEquals(result, 42);
    }
}
"#;
    assert!(lines_of("T032", unknown_wildcard).is_empty());
    let comment = r#"import static org.junit.Assert.assertEquals;

public class FooTest {
    public void test() {
        assertEquals(result /* actual */, 42);
    }
}
"#;
    assert_eq!(lines_of("T032", comment), [5]);
    assert_eq!(fixed("T032", comment), comment);
}

#[test]
fn assert_equals_on_arrays() {
    let source = r#"import static org.junit.jupiter.api.Assertions.assertEquals;

class FooTest {
    int[] field;

    void test(String[] parameter) {
        int[] local = compute();
        assertEquals(new int[] {1, 2}, local);
        assertEquals(field, local, "message");
        assertEquals(new String[] {"a"}, parameter);
        assertEquals(new long[] {1}, local);
        assertEquals(compute(), local);
        assertEquals(1, local.length);
    }
}
"#;
    assert_eq!(lines_of("T033", source), [8, 9, 10]);
    let result = unsafe_fixed("T033", source);
    assert!(result.contains("assertArrayEquals(new int[] {1, 2}, local);"), "{result}");
    assert!(
        result.contains("import static org.junit.jupiter.api.Assertions.assertEquals;\nimport static org.junit.jupiter.api.Assertions.assertArrayEquals;"),
        "{result}"
    );
    assert_eq!(fixed("T033", source), source);
    let junit4 = r#"import org.junit.Assert;

public class FooTest {
    public void test() {
        int[] a = compute();
        String[] s = names();
        double[] d = values();
        Assert.assertEquals(new int[] {1}, a);
        Assert.assertEquals(new String[] {"x"}, s);
        Assert.assertEquals(new double[] {1}, d);
    }
}
"#;
    assert_eq!(lines_of("T033", junit4), [8, 10]);
    let result = unsafe_fixed("T033", junit4);
    assert!(result.contains("Assert.assertArrayEquals(new int[] {1}, a);"), "{result}");
    assert!(result.contains("Assert.assertEquals(new double[] {1}, d);"), "{result}");
}

#[test]
fn self_assertion() {
    let source = r#"import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

class FooTest {
    void test() {
        assertSame(a, a);
        assertSame(this.a, this . a);
        assertThat(x).isEqualTo(x);
        assertThat(x).as("x").isSameAs(x);
        assertEquals(a, a);
        assertSame(next(), next());
        assertThat(x).isEqualTo(y);
        assertThat(x).usingComparator(c).isEqualTo(x);
        assertThat(x).isEqualByComparingTo(x);
    }
}
"#;
    assert_eq!(lines_of("T035", source), [7, 8, 9, 10]);
    assert_eq!(messages("T035", source)[0], "Assertion compares a value with itself");
    let hamcrest = r#"import static org.hamcrest.MatcherAssert.assertThat;

class FooTest {
    void test() {
        assertThat(x).isEqualTo(x);
    }
}
"#;
    assert!(lines_of("T035", hamcrest).is_empty());
}

#[test]
fn simplifiable_assertion() {
    let source = r#"import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class FooTest {
    void test(boolean flag) {
        assertEquals(true, a == b);
        assertEquals(false, flag, "message");
        assertEquals(true, map.get(key));
        assertEquals(null, value);
        assertTrue(!done);
        assertTrue(x == null, "x");
        assertTrue(x != null);
        assertTrue("x".equals(name));
        assertTrue(a.equals(b));
        assertTrue(a == b);
        assertTrue(flag);
    }
}
"#;
    assert_eq!(lines_of("T037", source), [6, 7, 9, 10, 11, 12, 13, 14]);
    assert_eq!(messages("T037", source)[0], "assertEquals() can be assertTrue()");
    let result = fixed("T037", source);
    for expected in [
        "assertTrue(a == b);",
        "assertFalse(flag, \"message\");",
        "assertEquals(true, map.get(key));",
        "assertNull(value);",
        "assertFalse(done);",
        "assertNull(x, \"x\");",
        "assertNotNull(x);",
        "assertEquals(\"x\", name);",
        "assertTrue(a.equals(b));",
        "import static org.junit.jupiter.api.Assertions.assertFalse;",
        "import static org.junit.jupiter.api.Assertions.assertNull;",
    ] {
        assert!(result.contains(expected), "{expected} in {result}");
    }
    assert!(unsafe_fixed("T037", source).contains("assertEquals(a, b);"));
}

#[test]
fn simplifiable_assertion_junit4_and_testng() {
    let junit4 = r#"import org.junit.Assert;

public class FooTest {
    public void test() {
        Assert.assertTrue("message", !done);
        Assert.assertFalse(x.equals(EXPECTED));
        Assert.assertEquals("message", null, x);
    }
}
"#;
    let result = unsafe_fixed("T037", junit4);
    assert!(result.contains("Assert.assertFalse(\"message\", done);"), "{result}");
    assert!(result.contains("Assert.assertNotEquals(EXPECTED, x);"), "{result}");
    assert!(result.contains("Assert.assertNull(\"message\", x);"), "{result}");
    let testng = r#"import static org.testng.Assert.*;

public class FooTest {
    public void test() {
        assertTrue(x.equals("a"), "message");
        assertEquals(x == y, true);
    }
}
"#;
    let result = unsafe_fixed("T037", testng);
    assert!(result.contains("assertEquals(x, \"a\", \"message\");"), "{result}");
    assert!(result.contains("assertTrue(x == y);"), "{result}");
    assert!(!result.contains("import static org.testng.Assert.assertTrue;"), "{result}");
    let comment = r#"import static org.junit.Assert.*;

public class FooTest {
    public void test() {
        assertTrue(/* not done */ !done);
    }
}
"#;
    assert_eq!(lines_of("T037", comment), [5]);
    assert_eq!(fixed("T037", comment), comment);
}

#[test]
fn assert_statement_in_test() {
    let source = r#"import org.junit.jupiter.api.Test;

class FooTest {
    @Test
    void test() {
        assert x == null;
        assert !done : "not done";
        assert size > 0;
        assert ok : size;
    }

    void helper() {
        assert x != null;
    }
}
"#;
    assert_eq!(lines_of("T040", source), [6, 7, 8, 9]);
    assert_eq!(messages("T040", source)[0], "assert statement in a test");
    let result = unsafe_fixed("T040", source);
    for expected in [
        "assertNull(x);",
        "assertFalse(done, \"not done\");",
        "assertTrue(size > 0);",
        "assert ok : size;",
        "import static org.junit.jupiter.api.Assertions.assertNull;",
        "import static org.junit.jupiter.api.Assertions.assertFalse;",
        "import static org.junit.jupiter.api.Assertions.assertTrue;",
        "assert x != null;",
    ] {
        assert!(result.contains(expected), "{expected} in {result}");
    }
    assert_eq!(fixed("T040", source), source);
    let junit4 = r#"import static org.junit.Assert.*;

import org.junit.Test;

public class FooTest {
    @Test
    public void test() {
        assert ok : "message";
    }
}
"#;
    let result = unsafe_fixed("T040", junit4);
    assert!(result.contains("assertTrue(\"message\", ok);"), "{result}");
    assert!(!result.contains("import static org.junit.Assert.assertTrue;"), "{result}");
}

#[test]
fn assertj_method_result() {
    let source = r#"import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import java.util.Map;
import java.util.Optional;

class FooTest {
    void test(List<String> list, Map<String, Integer> map, Optional<String> optional, List<?> wildcard) {
        String text = compute();
        assertThat(list.contains("a")).isTrue();
        assertThat(list.isEmpty()).isFalse();
        assertThat(text.startsWith("x")).as("prefix").isTrue();
        assertThat(map.containsKey("k")).isFalse();
        assertThat(optional.isPresent()).isTrue();
        assertThat(list.size()).isEqualTo(3);
        assertThat(text.equals("x")).isTrue();
        assertThat(wildcard.contains("a")).isTrue();
        assertThat(wildcard.isEmpty()).isTrue();
        assertThat(unknown.contains("a")).isTrue();
        assertThat(list.size()).isEqualTo(size);
        assertThat(text.isBlank()).isTrue();
        assertThat(list.contains("a")).isEqualTo(true);
    }
}
"#;
    assert_eq!(lines_of("T049", source), [10, 11, 12, 13, 14, 15, 16, 18]);
    assert_eq!(messages("T049", source)[0], "assertThat() of the result of contains() instead of contains()");
    let result = fixed("T049", source);
    for expected in [
        "assertThat(list).contains(\"a\");",
        "assertThat(list).isNotEmpty();",
        "assertThat(text).as(\"prefix\").startsWith(\"x\");",
        "assertThat(map).doesNotContainKey(\"k\");",
        "assertThat(optional).isPresent();",
        "assertThat(list).hasSize(3);",
        "assertThat(text).isEqualTo(\"x\");",
        "assertThat(wildcard.contains(\"a\")).isTrue();",
        "assertThat(wildcard).isEmpty();",
    ] {
        assert!(result.contains(expected), "{expected} in {result}");
    }
    let custom = r#"import static org.assertj.core.api.Assertions.assertThat;

class FooTest {
    void test(List<String> list) {
        assertThat(list.contains("a")).isTrue();
    }
}
"#;
    assert!(lines_of("T049", custom).is_empty());
    let hamcrest = r#"import static org.hamcrest.MatcherAssert.assertThat;

class FooTest {
    void test(String text) {
        assertThat(text.isEmpty()).isTrue();
    }
}
"#;
    assert!(lines_of("T049", hamcrest).is_empty());
}

#[test]
fn static_mock() {
    let source = r#"import org.mockito.Mock;

class FooTest {
    @Mock
    static Service service;

    @Mock
    static Repository repository;

    @Mock
    Other other;

    @org.mockito.Mock
    private static Client client;

    static void setUp() {
        repository.reset();
    }
}
"#;
    assert_eq!(lines_of("T060", source), [4, 7, 13]);
    assert_eq!(messages("T060", source)[0], "@Mock field is static");
    let result = fixed("T060", source);
    assert!(result.contains("@Mock\n    Service service;"), "{result}");
    assert!(result.contains("@Mock\n    static Repository repository;"), "{result}");
    assert!(result.contains("@org.mockito.Mock\n    private Client client;"), "{result}");
    let other = r#"import com.example.Mock;

class FooTest {
    @Mock
    static Service service;
}
"#;
    assert!(lines_of("T060", other).is_empty());
}

#[test]
fn incomplete_mockito_call() {
    let source = r#"import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import org.mockito.BDDMockito;
import org.mockito.Mockito;

class FooTest {
    void test() {
        verify(service);
        verify(service.call());
        when(service.call());
        Mockito.verify(service, Mockito.times(2));
        BDDMockito.given(service.call());
        verify(service).call();
        when(service.call()).thenReturn(1);
        var stubbing = when(service.call());
    }
}
"#;
    assert_eq!(lines_of("T062", source), [9, 10, 11, 12, 13]);
    assert_eq!(
        messages("T062", source)[..3],
        [
            "verify() without the verified call",
            "verify() without the verified call",
            "Stubbing without a return value or answer"
        ]
    );
    let project = r#"class FooTest {
    void test() {
        verify(service);
    }

    void verify(Object o) {}
}
"#;
    assert!(lines_of("T062", project).is_empty());
}
