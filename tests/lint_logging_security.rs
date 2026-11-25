//! Tests of the logging and security rules and the other rules of the same batch (M043, A044, A059, N015).

mod common;

use common::{fixed, messages, unsafe_fixed, violations};

const SLF4J: &str = "import org.slf4j.Logger;\nimport org.slf4j.LoggerFactory;\n";

fn slf4j(body: &str) -> String {
    format!("{SLF4J}class A {{\n  private static final Logger log = LoggerFactory.getLogger(A.class);\n{body}\n}}\n")
}

// L002

#[test]
fn foreign_logger_class_is_reported_and_fixed() {
    let source = format!("{SLF4J}class A {{\n  static final Logger log = LoggerFactory.getLogger(B.class);\n}}\n");
    assert_eq!(violations("L002", &source), ["4:53 L002"]);
    assert_eq!(messages("L002", &source), ["logger created for B instead of A"]);
    assert!(fixed("L002", &source).contains("LoggerFactory.getLogger(A.class)"));
}

#[test]
fn foreign_logger_class_covers_log4j_and_jul() {
    let source = "import org.apache.logging.log4j.LogManager;\nimport java.util.logging.Logger;\nclass A {\n  \
                  static final org.apache.logging.log4j.Logger a = LogManager.getLogger(B.class);\n  \
                  static final Logger b = Logger.getLogger(B.class.getName());\n}\n";
    assert_eq!(violations("L002", source).len(), 2);
    let fixed = fixed("L002", source);
    assert!(fixed.contains("LogManager.getLogger(A.class)"));
    assert!(fixed.contains("Logger.getLogger(A.class.getName())"));
}

#[test]
fn foreign_logger_class_near_misses() {
    // The own class, a nested class's own logger, a logger in a method, an unknown factory.
    let source = format!(
        "{SLF4J}class A {{\n  static final Logger log = LoggerFactory.getLogger(A.class);\n  \
         static class B {{ static final Logger log = LoggerFactory.getLogger(B.class); }}\n  \
         Logger other() {{ return LoggerFactory.getLogger(B.class); }}\n  \
         static final Object o = Other.getLogger(B.class);\n}}\n"
    );
    assert!(violations("L002", &source).is_empty());
    // Without the import, LoggerFactory may be another class.
    let source = "class A { static final Logger log = LoggerFactory.getLogger(B.class); }";
    assert!(violations("L002", source).is_empty());
}

// L004

#[test]
fn placeholder_count_mismatch_is_reported() {
    let source = slf4j(
        "  void f(String a, int b) {\n    log.info(\"a={} b={}\", a);\n    log.info(\"a={}\", a, b);\n    \
         log.info(\"a={}\");\n    log.info(\"x\" + \"{} {}\", 1, \"s\", b);\n  }",
    );
    assert_eq!(violations("L004", &source), ["6:5 L004", "7:5 L004", "8:5 L004", "9:5 L004"]);
    assert_eq!(
        messages("L004", &source),
        [
            "2 placeholders but 1 argument",
            "1 placeholder but 2 arguments",
            "1 placeholder but no arguments",
            "2 placeholders but 3 arguments"
        ]
    );
}

#[test]
fn placeholder_count_lets_a_throwable_go_last() {
    let source = slf4j(
        "  void f(String a, Object o) {\n    try {} catch (Exception e) {\n      log.warn(\"a={}\", a, e);\n      \
         log.warn(\"a={} e={}\", a, e);\n      log.warn(\"failed\", e);\n      log.warn(\"a={}\", e);\n      \
         log.warn(\"a={}\", a, new IllegalStateException());\n      log.warn(\"x\", a, e);\n    }\n  }",
    );
    // Only the last call has one placeholder too few even if `e` is the exception.
    assert_eq!(violations("L004", &source), ["12:7 L004"]);
}

#[test]
fn placeholder_count_near_misses() {
    let source = slf4j(
        "  void f(String a, Object o, Object[] array, String format) {\n    \
         log.info(\"a={} b={}\", array);\n    log.info(\"a={} b={}\", o);\n    log.info(\"a={} b={}\", call());\n    \
         log.info(\"a={}\", a, o);\n    log.info(\"a=\\\\{} b={}\", a);\n    log.info(format, a, a);\n    \
         log.info(\"\\u007B} {}\", a);\n    log.info(\"a={}\", a);\n  }",
    );
    assert!(violations("L004", &source).is_empty());
}

#[test]
fn placeholder_count_needs_a_known_logger() {
    // An unknown logger type, java.util.logging and a Log4j 2 formatter logger.
    let source = "import org.apache.logging.log4j.LogManager;\nimport org.apache.logging.log4j.Logger;\nclass A {\n  \
                  Logger f = LogManager.getFormatterLogger(A.class);\n  Logger g = LogManager.getLogger(A.class);\n  \
                  void f(Foo log, java.util.logging.Logger jul) {\n    log.info(\"{}\");\n    f.info(\"{}\");\n    \
                  g.info(\"{}\");\n  }\n}\n";
    assert_eq!(violations("L004", source), ["9:5 L004"]);
}

#[test]
fn placeholder_count_with_lombok() {
    let source = "import lombok.extern.slf4j.Slf4j;\n@Slf4j\nclass A {\n  void f() { log.info(\"{} {}\", 1); }\n}\n";
    assert_eq!(violations("L004", source), ["4:14 L004"]);
    let source = "@Slf4j\nclass A {\n  void f() { log.info(\"{} {}\", 1); }\n}\n";
    assert!(violations("L004", source).is_empty());
}

// L006

#[test]
fn throwable_not_last_is_reported_and_fixed() {
    let source =
        slf4j("  void f(long id) {\n    try {} catch (Exception e) {\n      log.warn(\"x {}\", e, id);\n    }\n  }");
    assert_eq!(violations("L006", &source), ["7:24 L006"]);
    assert!(fixed("L006", &source).contains("log.warn(\"x {}\", id, e);"));
}

#[test]
fn throwable_not_last_without_fix() {
    // Placeholders for all arguments, an unknown later argument, a call that would be reordered.
    let source = slf4j(
        "  void f(long id, Object o) {\n    try {} catch (Exception e) {\n      log.warn(\"x {} {}\", e, id);\n      \
         log.warn(\"x {}\", e, o);\n      log.warn(\"x {}\", e, call());\n    }\n  }",
    );
    assert_eq!(violations("L006", &source).len(), 3);
    assert_eq!(fixed("L006", &source), source);
}

#[test]
fn throwable_not_last_near_misses() {
    let source = slf4j(
        "  void f(long id, Object o) {\n    try {} catch (Exception e) {\n      log.warn(\"x {}\", id, e);\n      \
         log.warn(\"x {}\", o, id);\n      log.warn(\"x\", e);\n    }\n  }",
    );
    assert!(violations("L006", &source).is_empty());
}

// L009

#[test]
fn log_guard_level_is_reported_with_unsafe_fix() {
    let source = slf4j("  void f() {\n    if (log.isDebugEnabled()) {\n      log.info(\"x\");\n    }\n  }");
    assert_eq!(violations("L009", &source), ["6:5 L009"]);
    assert_eq!(messages("L009", &source), ["guard checks level debug for info calls"]);
    assert_eq!(fixed("L009", &source), source);
    assert!(unsafe_fixed("L009", &source).contains("if (log.isInfoEnabled())"));
}

#[test]
fn log_guard_level_with_condition_and_jul() {
    let source = "import java.util.logging.Level;\nimport java.util.logging.Logger;\nclass A {\n  \
                  static final Logger log = Logger.getLogger(A.class.getName());\n  void f(boolean b) {\n    \
                  if (b && log.isLoggable(Level.FINE)) log.info(\"x\");\n    \
                  if (log.isLoggable(Level.FINE)) log.log(Level.FINER, \"x\");\n  }\n}\n";
    assert_eq!(violations("L009", source), ["6:5 L009", "7:5 L009"]);
    let fixed = unsafe_fixed("L009", source);
    assert!(fixed.contains("log.isLoggable(Level.INFO)) log.info"));
    assert!(fixed.contains("log.isLoggable(Level.FINER)) log.log"));
}

#[test]
fn log_guard_level_near_misses() {
    let source = slf4j(
        "  void f(boolean b) {\n    if (log.isDebugEnabled()) log.debug(\"x\");\n    \
         if (log.isDebugEnabled()) { log.debug(\"x\"); log.info(\"y\"); }\n    \
         if (log.isDebugEnabled()) { log.debug(\"x\"); } else { log.info(\"y\"); }\n    \
         if (log.isDebugEnabled()) { try { run(); } catch (Exception e) { log.error(\"y\", e); } }\n    \
         if (log.isDebugEnabled()) { run(() -> log.info(\"y\")); }\n    \
         if (log.isDebugEnabled() || b) log.info(\"x\");\n    if (log.isDebugEnabled()) other.info(\"x\");\n  }",
    );
    assert!(violations("L009", &source).is_empty());
}

// V008

#[test]
fn hardcoded_crypto_key_is_reported() {
    let source = "import javax.crypto.spec.SecretKeySpec;\nclass A {\n  static final String KEY = \"k\";\n  \
                  static final byte[] BYTES = {1, 2};\n  void f() {\n    \
                  new SecretKeySpec(\"secret\".getBytes(), \"AES\");\n    \
                  new SecretKeySpec(new byte[] {1, 2}, \"AES\");\n    \
                  new SecretKeySpec(new byte[16], \"AES\");\n    new SecretKeySpec(KEY.getBytes(UTF_8), \"AES\");\n    \
                  new SecretKeySpec(BYTES, \"AES\");\n    \
                  new SecretKeySpec(Base64.getDecoder().decode(\"c2VjcmV0\"), \"AES\");\n  }\n}\n";
    assert_eq!(
        violations("V008", source),
        ["6:23 V008", "7:23 V008", "8:23 V008", "9:23 V008", "10:23 V008", "11:23 V008"]
    );
}

#[test]
fn hardcoded_crypto_key_near_misses() {
    let source = "import javax.crypto.spec.SecretKeySpec;\nclass A {\n  static String key = \"k\";\n  \
                  void f(byte[] k) {\n    \
                  byte[] buffer = new byte[16];\n    random.nextBytes(buffer);\n    \
                  new SecretKeySpec(buffer, \"AES\");\n    new SecretKeySpec(k, \"AES\");\n    \
                  new SecretKeySpec(key.getBytes(), \"AES\");\n    \
                  new SecretKeySpec(load().getBytes(), \"AES\");\n  }\n}\n";
    assert!(violations("V008", source).is_empty());
    // Another SecretKeySpec.
    assert!(violations("V008", "class A { Object o = new SecretKeySpec(\"s\".getBytes(), \"AES\"); }").is_empty());
}

// V014

#[test]
fn bidi_characters_are_reported_anywhere() {
    let source = "class A {\n  // a \u{202E} b\n  String s = \"\u{2067}\";\n  String t = \"\u{200F}\";\n}\n";
    assert_eq!(violations("V014", source), ["2:8 V014", "3:15 V014", "4:15 V014"]);
    assert_eq!(messages("V014", source)[0], "Unicode directionality character U+202E");
    // An escape is no violation.
    assert!(violations("V014", "class A { String s = \"\\u202E\"; }").is_empty());
}

// M043

#[test]
fn path_of_replaces_the_import() {
    let source = "import java.nio.file.Paths;\nclass A {\n  Object a = Paths.get(\"a\", \"b\");\n  \
                  Object b = Paths.get(\"c\");\n}\n";
    assert_eq!(violations("M043", source), ["3:14 M043", "4:14 M043"]);
    assert_eq!(
        fixed("M043", source),
        "import java.nio.file.Path;\nclass A {\n  Object a = Path.of(\"a\", \"b\");\n  Object b = Path.of(\"c\");\n}\n"
    );
}

#[test]
fn path_of_removes_or_keeps_the_paths_import() {
    let source =
        "import java.nio.file.Path;\nimport java.nio.file.Paths;\nclass A {\n  Path a = Paths.get(\"a\");\n}\n";
    assert_eq!(fixed("M043", source), "import java.nio.file.Path;\nclass A {\n  Path a = Path.of(\"a\");\n}\n");
    // A method reference keeps using Paths.
    let source =
        "import java.nio.file.Paths;\nclass A {\n  Object a = Paths.get(\"a\");\n  Object f = Paths::get;\n}\n";
    assert_eq!(
        fixed("M043", source),
        "import java.nio.file.Path;\nimport java.nio.file.Paths;\nclass A {\n  Object a = Path.of(\"a\");\n  \
         Object f = Paths::get;\n}\n"
    );
    // On-demand import.
    let source = "import java.nio.file.*;\nclass A {\n  Object a = Paths.get(\"a\");\n}\n";
    assert_eq!(fixed("M043", source), "import java.nio.file.*;\nclass A {\n  Object a = Path.of(\"a\");\n}\n");
    // A Javadoc link keeps the import.
    let source = "import java.nio.file.Paths;\n/** {@link Paths} */\nclass A {\n  Object a = Paths.get(\"a\");\n}\n";
    assert!(fixed("M043", source).starts_with("import java.nio.file.Path;\nimport java.nio.file.Paths;\n"));
}

#[test]
fn path_of_qualified_and_near_misses() {
    let source = "class A {\n  Object a = java.nio.file.Paths.get(\"a\");\n}\n";
    assert_eq!(fixed("M043", source), "class A {\n  Object a = java.nio.file.Path.of(\"a\");\n}\n");
    // Paths of another package, or not imported.
    assert!(violations("M043", "import a.Paths;\nclass A { Object a = Paths.get(\"a\"); }").is_empty());
    assert!(violations("M043", "class A { Object a = Paths.get(\"a\"); }").is_empty());
    // Another Path in the file: reported without fix.
    let source = "import java.nio.file.Paths;\nimport a.Path;\nclass A { Object a = Paths.get(\"a\"); }";
    assert_eq!(violations("M043", source).len(), 1);
    assert_eq!(fixed("M043", source), source);
}

// A044

#[test]
fn protected_in_final_class_is_reported_and_fixed() {
    let source = "final class A {\n  protected int x;\n  protected A() {}\n  protected void f() {}\n  \
                  protected static void g() {}\n}\n";
    assert_eq!(violations("A044", source), ["2:3 A044", "3:3 A044", "4:3 A044", "5:3 A044"]);
    assert_eq!(fixed("A044", source), "final class A {\n  int x;\n  A() {}\n  void f() {}\n  static void g() {}\n}\n");
}

#[test]
fn protected_in_final_class_near_misses() {
    // Methods that may override: in a subclass, with @Override, Object's clone and finalize.
    let source = "final class A extends B {\n  protected int x;\n  protected void f() {}\n}\n\
                  final class C {\n  @Override protected Object clone() { return this; }\n  \
                  protected void finalize() {}\n}\nclass D {\n  protected int x;\n}\n";
    assert_eq!(violations("A044", source), ["2:3 A044"]);
}

// A059

#[test]
fn mutable_public_array_is_reported() {
    let source = "class A {\n  public static final int[] X = {1};\n  \
                  public static final String[] Y = new String[] {\"a\"};\n  \
                  public static final int[] Z = new int[3];\n}\ninterface I {\n  int[] W = {1};\n}\n";
    assert_eq!(violations("A059", source), ["2:29 A059", "3:32 A059", "4:29 A059", "7:9 A059"]);
}

#[test]
fn mutable_public_array_near_misses() {
    let source = "class A {\n  public static final int[] X = {};\n  public static final int[] Y = new int[0];\n  \
                  private static final int[] Z = {1};\n  public static int[] W = {1};\n  \
                  public static final int[] V = values();\n  @DataPoints public static final int[] D = {1};\n  \
                  public static final int[] S = new int[SIZE];\n}\n";
    assert!(violations("A059", source).is_empty());
}

// N015

#[test]
fn method_named_like_class() {
    let source = "class Foo {\n  Foo() {}\n  void Foo() {}\n  class Bar { int Bar() { return 0; } void foo() {} }\n}\n";
    assert_eq!(violations("N015", source), ["3:8 N015", "4:19 N015"]);
}
