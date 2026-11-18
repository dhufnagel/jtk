//! Runs the upstream formatter integration test corpus (`tests/testdata/*.input` / `*.output`).
//!
//! Prints a summary and fails if any test fails. Set `PJF_TEST=<name>` to run a single case and print a diff.

use std::fs;
use std::path::Path;

use jtk::{format_source, format_without_reflow};

fn first_diff(expected: &str, actual: &str) -> String {
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}:\n  expected: {e:?}\n  actual:   {a:?}", i + 1);
        }
    }
    format!("line count differs: expected {}, actual {}", expected.lines().count(), actual.lines().count())
}

/// Inputs that javac's parser accepts but jtk's parser (like the tree-sitter grammar it follows) rejects. They are not valid Java: `this(...)` outside
/// a constructor, and annotations mixing positional and named arguments.
const KNOWN_FAILURES: &[&str] = &["B26952926", "B38352414", "palantir-expression-lambda-2"];

#[test]
fn testdata() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata");
    let only = std::env::var("PJF_TEST").ok();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension()? == "input").then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut idempotency = Vec::new();
    let mut known = Vec::new();
    for name in &names {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        if KNOWN_FAILURES.contains(&name.as_str()) {
            let input = fs::read_to_string(dir.join(format!("{name}.input"))).unwrap();
            match format_source(&input) {
                Err(_) => known.push(name.clone()),
                Ok(_) => {
                    failed.push(format!("{name}: listed in KNOWN_FAILURES but formats now; remove it from the list"))
                }
            }
            continue;
        }
        let input = fs::read_to_string(dir.join(format!("{name}.input"))).unwrap();
        let expected = fs::read_to_string(dir.join(format!("{name}.output"))).unwrap();
        let result = std::panic::catch_unwind(|| format_source(&input));
        match result {
            Ok(Ok(actual)) if actual == expected => passed.push(name.clone()),
            Ok(Ok(actual)) => {
                if only.is_some() {
                    println!("{actual}");
                }
                failed.push(format!("{name}: {}", first_diff(&expected, &actual)))
            }
            Ok(Err(e)) => failed.push(format!("{name}: {e}")),
            Err(_) => failed.push(format!("{name}: panic")),
        }
        // Idempotency (like FormatterIntegrationTest.idempotent{LF,CR,CRLF}): formatting the expected output with
        // any line separator must not change it.
        let separator = if expected.ends_with("\r\n") {
            "\r\n"
        } else if expected.ends_with('\r') {
            "\r"
        } else {
            "\n"
        };
        for sep in ["\n", "\r", "\r\n"] {
            let mangled = expected.replace(separator, sep);
            match std::panic::catch_unwind(|| format_without_reflow(&mangled)) {
                Ok(Ok(out)) if out == mangled => {}
                Ok(Ok(out)) => idempotency.push(format!("{name} ({sep:?}): {}", first_diff(&mangled, &out))),
                Ok(Err(e)) => idempotency.push(format!("{name} ({sep:?}): {e}")),
                Err(_) => idempotency.push(format!("{name} ({sep:?}): panic")),
            }
        }
    }
    for f in &failed {
        println!("FAIL {f}");
    }
    for f in &idempotency {
        println!("NOT IDEMPOTENT {f}");
    }
    println!(
        "\n{} passed, {} failed, {} idempotency failures, {} known failures ({})",
        passed.len(),
        failed.len(),
        idempotency.len(),
        known.len(),
        known.join(", ")
    );
    fs::write(dir.join("../passing.txt"), passed.join("\n") + "\n").ok();
    assert!(failed.is_empty() && idempotency.is_empty());
}
