mod common;

use common::{fixed, lines_of, messages, unsafe_fixed};

const SLF4J: &str = "import org.slf4j.Logger;\nimport org.slf4j.LoggerFactory;\n";

fn logging(body: &str) -> String {
    format!(
        "{SLF4J}class A {{\n  private static final Logger log = LoggerFactory.getLogger(A.class);\n  static final \
         String NAME = \"n\";\n  void m(String x, int n, Object[] arr) {{\n{body}\n  }}\n}}\n"
    )
}

#[test]
fn concatenated_log_message_reports_concatenation_and_format() {
    let source = logging(
        "    log.info(\"x=\" + x);\n    log.error(\"failed \" + x, new RuntimeException());\n    \
         log.debug(String.format(\"x=%s\", x));\n    log.debug(\"x=%s\".formatted(x));\n    log.warn(x + \"!\");",
    );
    assert_eq!(lines_of("L008", &source), [7, 8, 9, 10, 11]);
    assert_eq!(messages("L008", &source)[0], "log message built by concatenation instead of with placeholders");
}

#[test]
fn concatenated_log_message_ignores_constants_and_other_loggers() {
    let source = logging(
        "    log.info(\"a\" + \"b\");\n    log.info(\"a \" + NAME + 1);\n    log.info(\"x={}\", x);\n    \
         log.info(x);\n    log.info(String.format(\"fixed\"));\n    String s = \"a\" + x;\n    log.info(s);",
    );
    assert!(lines_of("L008", &source).is_empty());
    let other = "class A {\n  Logger log;\n  void m(String x) { log.info(\"x=\" + x); }\n}\n";
    assert!(lines_of("L008", other).is_empty());
    let jul = "import java.util.logging.Logger;\nclass A {\n  Logger log;\n  void m(String x) { log.info(\"x=\" + \
               x); }\n}\n";
    assert!(lines_of("L008", jul).is_empty());
    let own_string =
        logging("    log.info(String.format(\"x=%s\", x));").replace("class A {", "class A {\n  class String {}");
    assert!(lines_of("L008", &own_string).is_empty());
}

#[test]
fn concatenated_log_message_fixes_unsafely() {
    let source = logging("    log.error(\"DELETE \" + x + \"[\" + n + \"]\");");
    assert_eq!(fixed("L008", &source), source);
    assert!(unsafe_fixed("L008", &source).contains("log.error(\"DELETE {}[{}]\", x, n);"));
    let throwable = logging("    log.error(\"failed \" + x + \" \" + x.length(), new IllegalStateException());");
    assert!(
        unsafe_fixed("L008", &throwable)
            .contains("log.error(\"failed {} {}\", x, x.length(), new IllegalStateException());")
    );
    let leading = logging("    log.warn(x + \"!\" + (n + 1));");
    assert!(unsafe_fixed("L008", &leading).contains("log.warn(\"{}!{}\", x, (n + 1));"));
    let format = logging("    log.debug(String.format(\"a=%s b=%s\", x, n));\n    log.debug(\"x=%s\".formatted(x));");
    let result = unsafe_fixed("L008", &format);
    assert!(result.contains("log.debug(\"a={} b={}\", x, n);"));
    assert!(result.contains("log.debug(\"x={}\", x);"));
}

#[test]
fn concatenated_log_message_fixes_after_marker() {
    let source = format!(
        "import org.slf4j.Marker;\n{}",
        logging("    Marker marker = null;\n    log.info(marker, \"x=\" + x);")
    );
    assert_eq!(lines_of("L008", &source), [9]);
    assert!(unsafe_fixed("L008", &source).contains("log.info(marker, \"x={}\", x);"));
}

#[test]
fn concatenated_log_message_does_not_fix_risky_cases() {
    for body in [
        // The literal has a placeholder or a backslash already.
        "    log.info(\"x={} \" + x, n);",
        "    log.info(\"path\\\\\" + x);",
        // Numeric addition before the first string.
        "    log.info(n + n + \"!\");",
        // A throwable among the values.
        "    try { } catch (RuntimeException e) { log.info(\"failed: \" + e); }",
        // A comment in the message.
        "    log.info(\"x=\" /* the x */ + x);",
        // Format specifiers other than %s, or a count that does not match.
        "    log.info(String.format(\"n=%d\", n));",
        "    log.info(String.format(\"%s %s\", x));",
        // A single array argument of String.format is the varargs array.
        "    log.info(String.format(\"%s\", arr));",
    ] {
        let source = logging(body);
        assert_eq!(lines_of("L008", &source).len(), 1, "{body}");
        assert_eq!(unsafe_fixed("L008", &source), source, "{body}");
    }
}

#[test]
fn commented_out_code_reports_code() {
    let source = "class A {\n  // public OrtTyp placeType;\n\n  void m() {\n    // billingService.createLink(user);\n    \
                  run();\n    // if (x) {\n    //   y = 1;\n    // }\n    /* int z = compute(); */\n  }\n}\n\
                  //import java.util.List;\n";
    assert_eq!(lines_of("A101", source), [2, 5, 7, 10, 13]);
    assert_eq!(messages("A101", source)[0], "commented-out code");
}

#[test]
fn commented_out_code_ignores_prose() {
    let comments = [
        "/** Calls {@code run();}. */",
        "/// run();",
        "// see above;",
        "// Example: run();",
        "// TODO: run();",
        "// http://example.com/a;",
        "// NOSONAR run();",
        "// We call this;",
        "// a + b;",
        "// run()",
        "// @formatter:off",
        "// For example:\n    /* x = 1; */",
        "/*\n     * For example:\n     */\n    // x = 1;",
        "/*\n     * Copyright 2024 Example; all rights reserved.\n     */",
    ];
    for comment in comments {
        let source = format!("class A {{\n  void m() {{\n    {comment}\n    run(); // run();\n  }}\n}}\n");
        assert!(lines_of("A101", &source).is_empty(), "{comment}");
    }
}

#[test]
fn commented_out_code_groups_consecutive_line_comments() {
    let source = "class A {\n  void m() {\n    // run();\n    // and then we stop\n    // stop();\n\n    // a();\n    \
                  // b();\n  }\n}\n";
    // The first block mixes prose and code and is no Java; the second is one block.
    assert_eq!(lines_of("A101", source), [7]);
}

#[test]
fn commented_out_code_fix_removes_lines_unsafely() {
    let source = "class A {\n  String a;\n\n  // public OrtTyp placeType;\n\n}\n";
    assert_eq!(fixed("A101", source), source);
    assert_eq!(unsafe_fixed("A101", source), "class A {\n  String a;\n\n}\n");
    let block = "class A {\n  void m() {\n    run();\n    // a();\n    // b();\n    run();\n  }\n}\n";
    assert_eq!(unsafe_fixed("A101", block), "class A {\n  void m() {\n    run();\n    run();\n  }\n}\n");
}
