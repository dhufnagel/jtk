//! Linter rules of group B (likely bugs) on comparisons and arithmetic with constant or suspicious results.

mod common;
use common::{fixed, messages, unsafe_fixed, violations};

/// A class with the statements in a method.
fn in_method(statements: &str) -> String {
    format!("class A {{\n    void f() {{\n{statements}    }}\n}}\n")
}

#[test]
fn compare_to_constant() {
    let source = in_method(
        "        b = a.compareTo(b) == 1;
        b = a.compareTo(b) == -1;
        b = -1 != a.compareTo(b);
        b = (a.compareTo(b)) >= 1;
        b = Integer.compare(x, y) < 1;
        b = java.lang.Long.compare(x, y) > -1;
        b = Objects.compare(a, b, c) <= -1;
        b = a.compareTo(b) == 2;
        b = comparator.compare(a, b) == 1;
        b = a.compareTo(b) > 0;
        b = a.compareTo(b, c) == 1;
        b = differ.compare(a, b) == 1;
        b = a.compareTo(b) == ONE;
        b = a.compareTo(b) == /* one */ 1;
",
    );
    assert_eq!(
        violations("B054", &source),
        [
            "3:13 B054",
            "4:13 B054",
            "5:13 B054",
            "6:13 B054",
            "7:13 B054",
            "8:13 B054",
            "9:13 B054",
            "10:13 B054",
            "16:13 B054"
        ]
    );
    assert_eq!(
        messages("B054", &in_method("b = a.compareTo(b) == 1;\n")),
        ["compareTo result compared with a constant other than 0"]
    );
    assert_eq!(
        fixed("B054", &source),
        in_method(
            "        b = a.compareTo(b) > 0;
        b = a.compareTo(b) < 0;
        b = a.compareTo(b) >= 0;
        b = (a.compareTo(b)) > 0;
        b = Integer.compare(x, y) <= 0;
        b = java.lang.Long.compare(x, y) >= 0;
        b = Objects.compare(a, b, c) < 0;
        b = a.compareTo(b) == 2;
        b = comparator.compare(a, b) == 1;
        b = a.compareTo(b) > 0;
        b = a.compareTo(b, c) == 1;
        b = differ.compare(a, b) == 1;
        b = a.compareTo(b) == ONE;
        b = a.compareTo(b) == /* one */ 1;
"
        )
    );
    // A comparator declared in the file.
    let source = "class A {
    java.util.Comparator<String> comparator;
    boolean f(String a, String b) { return comparator.compare(a, b) == 1; }
}
";
    assert_eq!(violations("B054", source), ["3:44 B054"]);
}

#[test]
fn nan_comparison() {
    let source = in_method(
        "        b = x == Double.NaN;
        b = Float.NaN != x + 1;
        b = x < java.lang.Double.NaN;
        b = x.equals(Double.NaN);
        b = Double.isNaN(x);
        b = x == Other.NaN;
        b = x == /* nan */ Double.NaN;
",
    );
    assert_eq!(violations("B078", &source), ["3:13 B078", "4:13 B078", "5:13 B078", "9:13 B078"]);
    assert_eq!(
        messages("B078", &source)[..3],
        [
            "Comparison with NaN is always false",
            "Comparison with NaN is always true",
            "Comparison with NaN is always false"
        ]
    );
    assert_eq!(
        fixed("B078", &source),
        in_method(
            "        b = Double.isNaN(x);
        b = !Float.isNaN(x + 1);
        b = x < java.lang.Double.NaN;
        b = x.equals(Double.NaN);
        b = Double.isNaN(x);
        b = x == Other.NaN;
        b = x == /* nan */ Double.NaN;
"
        )
    );
}

#[test]
fn constant_bitmask() {
    let source = in_method(
        "        b = (x & 2) == 1;
        b = 0x1234 != (0xFF00 & x);
        b = (x | 4) == 3;
        i = x & 1 & 2;
        i = 0x01 & 0x02;
        i = 2 & (x & 1);
        b = (x & 3) == 1;
        b = (x & 0xFF00) == 0x1200;
        b = (x | 1) == 3;
        i = x & 1 & 3;
        i = 0x03 & 0x01;
        i = x & 0;
        b = (x & MASK) == 1;
",
    );
    assert_eq!(
        violations("B086", &source),
        ["3:13 B086", "4:13 B086", "5:13 B086", "6:13 B086", "7:13 B086", "8:13 B086"]
    );
    assert_eq!(
        messages("B086", &source)[..2],
        [
            "Incompatible bit mask: the comparison is always false",
            "Incompatible bit mask: the comparison is always true"
        ]
    );
}

#[test]
fn constant_comparison() {
    let source = "import java.util.*;
class A {
    List<String> list;
    int[] array;
    void f(String s, StringBuilder builder, Map<String, Integer> map, java.util.ArrayList<String> other) {
        b = list.size() >= 0;
        b = 0 <= s.length();
        b = s.indexOf(\"a\") >= -1;
        b = -1 <= list.lastIndexOf(x);
        b = map.size() < 0;
        b = array.length >= 0;
        b = builder.indexOf(\"a\", 2) > -2;
        b = \"abc\".length() != -1;
        b = other.size() == -1;
        b = (list.size()) > -1;
        b = list.size() > 0;
        b = s.indexOf(\"a\") >= 0;
        b = list.size() >= 1;
        b = getList().size() >= 0;
        b = custom.size() >= 0;
        b = list.size(1) >= 0;
        b = s.indexOf(\"a\") > -1;
        b = s.indexOf(\"a\") != -1;
        b = array.length > 0;
        b = s.length >= 0;
    }
    void g(Custom list) { b = list.size() >= 0; }
    void h() { var list = new ArrayList<String>(); b = list.size() >= 0; }
}
";
    assert_eq!(
        violations("B087", source),
        [
            "6:13 B087",
            "7:13 B087",
            "8:13 B087",
            "9:13 B087",
            "10:13 B087",
            "11:13 B087",
            "12:13 B087",
            "13:13 B087",
            "14:13 B087",
            "15:13 B087"
        ]
    );
    assert_eq!(
        messages("B087", source)[..3],
        [
            "size() is never less than 0, so the comparison is always true",
            "length() is never less than 0, so the comparison is always true",
            "indexOf() is never less than -1, so the comparison is always true"
        ]
    );
}

#[test]
fn oddness_check() {
    let source = in_method(
        "        b = x % 2 == 1;
        b = 1 != (x + y) % 2;
        b = (x % 2) == 1L;
        b = x % 2 == 0;
        b = x % 2 != 0;
        b = (x & 1) == 1;
        b = x % 3 == 1;
        b = list.size() % 2 == 1;
        b = s.length() % 2 == 1;
        b = array.length % 2 == 1;
        b = Math.abs(x) % 2 == 1;
        b = x % 2 == -1;
        b = x % 2 /* odd */ == 1;
",
    );
    assert_eq!(violations("B090", &source), ["3:13 B090", "4:13 B090", "5:13 B090", "15:13 B090"]);
    assert_eq!(fixed("B090", &source), source);
    assert_eq!(
        unsafe_fixed("B090", &source),
        in_method(
            "        b = x % 2 != 0;
        b = 0 == (x + y) % 2;
        b = (x % 2) != 0;
        b = x % 2 == 0;
        b = x % 2 != 0;
        b = (x & 1) == 1;
        b = x % 3 == 1;
        b = list.size() % 2 == 1;
        b = s.length() % 2 == 1;
        b = array.length % 2 == 1;
        b = Math.abs(x) % 2 == 1;
        b = x % 2 == -1;
        b = x % 2 /* odd */ != 0;
"
        )
    );
}

#[test]
fn constant_overflow() {
    let source = "class A {
    long ms = 365 * 24 * 3600 * 1000;
    int i = 1000000 * 1000000 * 1000;
    long l = 1000000000000L * 1000000000L;
    long s = -(1 << 30) * 4 + x;
    long c = (long) (1000000 * 1000000);
    long t = 24 * (3600 * 1000 * 1000);
    long ok1 = 365L * 24 * 3600 * 1000;
    long ok2 = (long) 365 * 24 * 3600 * 1000;
    int ok3 = 0x7fffffff + 1;
    int ok4 = -2147483648 - 1;
    int ok5 = 1 << 31;
    int ok6 = 0xFFFFFFFF * 1;
    long ok7 = x * 1000000 * 1000000;
    int ok8 = 2147483647 * 1;
    double ok9 = 1e10 * 1e10;
    int ok10 = 1 / 0 * 2;
    long ok11 = -2147483648 * -1L;
}
";
    assert_eq!(
        violations("B093", source),
        ["2:15 B093", "3:13 B093", "4:14 B093", "5:14 B093", "6:14 B093", "7:14 B093"]
    );
    assert_eq!(messages("B093", "class A { int i = 65536 * 65536; }"), ["Constant expression overflows"]);
    assert_eq!(fixed("B093", source), source);
    let fixed = unsafe_fixed("B093", source);
    assert!(fixed.contains("long ms = 365L * 24 * 3600 * 1000;"));
    assert!(fixed.contains("int i = 1000000 * 1000000 * 1000;"));
    assert!(fixed.contains("long t = 24 * (3600 * 1000 * 1000);"));
}

#[test]
fn division_by_zero() {
    let source = in_method(
        "        i = x / 0;
        i = x % 0L;
        x /= 0;
        x %= 0x0;
        i = x / (1 - 1);
        d = 1.0 / 0;
        d = -1.0 / 0;
        d = 0.0 / 0.0;
        d = x / 0.0;
        i = x / 1;
        x += 0;
",
    );
    assert_eq!(violations("B096", &source), ["3:13 B096", "4:13 B096", "5:9 B096", "6:9 B096", "7:13 B096"]);
    assert_eq!(messages("B096", &source)[..2], ["Division by zero", "Remainder by zero"]);
}

#[test]
fn random_cast_to_int() {
    let source = "import java.util.Random;
class A {
    Random random = new Random();
    void f(java.security.SecureRandom secure, Other other) {
        i = (int) Math.random() * 10;
        l = (long) (StrictMath.random());
        i = (int) random.nextDouble() * 6;
        i = (int) secure.nextFloat();
        i = (int) ThreadLocalRandom.current().nextDouble();
        i = (char) new Random().nextDouble();
        i = (int) (Math.random() * 10);
        d = (double) Math.random();
        i = (int) other.nextDouble();
        i = (int) random.nextDouble(10);
        i = (int) random.nextInt();
        i = (int) Other.random();
    }
}
";
    assert_eq!(
        violations("B101", source),
        ["5:13 B101", "6:13 B101", "7:13 B101", "8:13 B101", "9:13 B101", "10:13 B101"]
    );
}

#[test]
fn xor_power() {
    let source = in_method(
        "        i = 2 ^ 8;
        i = 10 ^ 3;
        l = 2L ^ 40;
        i = a * 2 ^ 8;
        i = 2 ^ 32;
        i = 10 ^ 10;
        i = x | 2 ^ 16;
        i = 2 ^ 0x10;
        i = 3 ^ 8;
        i = 2 ^ x;
        i = a ^ 2 ^ 8;
        i = 02 ^ 8;
        i = 2 ^ /* bits */ 8;
",
    );
    assert_eq!(
        violations("B108", &source),
        ["3:13 B108", "4:13 B108", "5:13 B108", "6:13 B108", "7:13 B108", "8:13 B108", "9:17 B108", "15:13 B108"]
    );
    assert_eq!(messages("B108", &in_method("i = 2 ^ 8;\n")), ["^ is exclusive or, not a power: 2 ^ 8 is 10"]);
    assert_eq!(
        fixed("B108", &source),
        in_method(
            "        i = 1 << 8;
        i = 1000;
        l = 1L << 40;
        i = a * (1 << 8);
        i = 2 ^ 32;
        i = 10 ^ 10;
        i = x | 1 << 16;
        i = 2 ^ 0x10;
        i = 3 ^ 8;
        i = 2 ^ x;
        i = a ^ 2 ^ 8;
        i = 02 ^ 8;
        i = 2 ^ /* bits */ 8;
"
        )
    );
}
