//! Upstream javadoc formatting tests (`JavadocFormattingTest`), extracted into `tests/javadoc/cases.json`, plus its
//! file-based cases. Like upstream, they format in the Google style (100 columns, 2-space indent) with javadoc
//! formatting enabled.

use std::path::Path;

use jtk::formatter::{FormatOptions, Style, format_ranges_with};

fn format(text: &str) -> String {
    let options = FormatOptions { format_javadoc: true, style: Style::GOOGLE, ..FormatOptions::default() };
    format_ranges_with(text, &[(0, text.len())], &options).unwrap_or_else(|e| format!("ERROR: {e}"))
}

fn dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/javadoc"))
}

#[test]
fn upstream_cases() {
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        input: Vec<String>,
        expected: Vec<String>,
    }
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(dir().join("cases.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    for case in &cases {
        let actual = format(&case.input.join("\n"));
        let expected = case.expected.join("\n") + "\n";
        if actual != expected {
            failures.push(format!("--- {}\n--- expected\n{expected}--- actual\n{actual}", case.name));
        }
    }
    for f in &failures {
        println!("{f}");
    }
    println!("{} of {} javadoc cases passed", cases.len() - failures.len(), cases.len());
    assert!(failures.is_empty());
}

#[test]
fn file_cases() {
    for name in ["B28750242", "B31404367"] {
        let input = std::fs::read_to_string(dir().join(format!("{name}.input"))).unwrap();
        let expected = std::fs::read_to_string(dir().join(format!("{name}.output"))).unwrap();
        assert_eq!(format(&input), expected, "{name}");
    }
}

#[test]
fn windows_line_separator() {
    let input = ["/**", " * hello", " *", " * <p>world", " */", "class Test {}"];
    for separator in ["\r", "\r\n"] {
        let text = input.join(separator);
        assert_eq!(format(&text), text.clone() + separator, "{separator:?}");
    }
}
