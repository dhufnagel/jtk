//! Upstream import tests (`GoogleImportStyleTest`, `RemoveUnusedImportsTest`, `RemoveUnusedImportsCaseLabelsTest`),
//! extracted into `tests/imports/*.json` as `[[input lines], [output lines]]` pairs.

use palantir_java_format::imports::{remove_unused_imports, reorder_imports};

fn cases(file: &str) -> Vec<(String, String)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/imports").join(file);
    let raw: Vec<[Vec<String>; 2]> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    raw.into_iter()
        .map(|[input, output]| {
            let output = if output.is_empty() { input.clone() } else { output };
            // As in ImportOrdererUtils: a line ending in `\` is joined with the next one.
            let input = (input.join("\n") + "\n").replace("\\\n", "");
            (input, output.join("\n") + "\n")
        })
        .collect()
}

fn check(file: &str, f: impl Fn(&str) -> Result<String, String>) {
    let mut failures = Vec::new();
    let all = cases(file);
    for (i, (input, expected)) in all.iter().enumerate() {
        let actual = match f(input) {
            Ok(out) => out,
            Err(e) => format!("!!{e}\n"),
        };
        if &actual != expected {
            failures.push(format!("case {i}:\n--- input\n{input}--- expected\n{expected}--- actual\n{actual}"));
        }
    }
    for f in &failures {
        println!("{f}");
    }
    println!("{file}: {} of {} passed", all.len() - failures.len(), all.len());
    assert!(failures.is_empty());
}

#[test]
fn google_import_style() {
    check("google_style.json", reorder_imports);
}

#[test]
fn remove_unused() {
    check("remove_unused.json", |s| Ok(remove_unused_imports(s)));
}
