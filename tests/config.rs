//! The project configuration in `jtk.toml` and directories as arguments.

use std::path::PathBuf;

use jtk::cli::run;

/// A temporary directory with the given files, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str, files: &[(&str, &str)]) -> Dir {
        let dir = std::env::temp_dir().join(format!("jtk-config-{}-{name}", std::process::id()));
        for (file, content) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        Dir(dir)
    }

    fn path(&self, file: &str) -> String {
        self.0.join(file).to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the tool, returning (exit code, stdout with the directory's path removed and `/` as separator, stderr).
fn jtk(dir: &Dir, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut "".as_bytes(), &mut out, &mut err);
    let prefix = format!("{}{}", dir.0.display(), std::path::MAIN_SEPARATOR);
    let out = String::from_utf8(out).unwrap().replace(&prefix, "");
    // Windows paths use `\`.
    let out = if cfg!(windows) { out.replace('\\', "/") } else { out };
    (code, out, String::from_utf8(err).unwrap())
}

const LONG_LIST: &str = "class A {\n    void f(int a, int b, int c, int d, int e, int f, int g, int h) {}\n}\n";

#[test]
fn selects_rules_and_sets_their_parameters() {
    let config = "[lint]\nselect = [\"C010\"]\n";
    let d = Dir::new("parameters", &[("jtk.toml", config), ("src/A.java", LONG_LIST)]);
    let (code, out, _) = jtk(&d, &["lint", &d.path("src")]);
    assert_eq!((code, out.as_str()), (1, "src/A.java:2:10: C010 More than 7 parameters (found 8).\n"));

    std::fs::write(d.path("jtk.toml"), format!("{config}[lint.options.C010]\nmax = 8\n")).unwrap();
    assert_eq!(jtk(&d, &["lint", &d.path("src")]), (0, String::new(), String::new()));
    // The command line replaces the selection, but keeps the parameters.
    assert_eq!(jtk(&d, &["lint", "--select", "C01", &d.path("src")]).0, 0);
}

#[test]
fn excludes_files_and_ignores_rules_per_file() {
    let config = "exclude = [\"src/generated\"]\n\n[lint]\nselect = [\"C010\"]\n\n[lint.per-file-ignores]\n\"**/*Test.java\" = [\"C\"]\n";
    let files = [
        ("jtk.toml", config),
        ("src/A.java", LONG_LIST),
        ("src/ATest.java", LONG_LIST),
        ("src/generated/G.java", LONG_LIST),
    ];
    let d = Dir::new("exclude", &files);
    let (_, out, _) = jtk(&d, &["lint", &d.path("")]);
    assert_eq!(out, "src/A.java:2:10: C010 More than 7 parameters (found 8).\n");
    // A file given explicitly is checked even if excluded.
    assert_eq!(jtk(&d, &["lint", &d.path("src/generated/G.java")]).0, 1);
}

#[test]
fn nearest_configuration_applies() {
    let files = [
        ("jtk.toml", "[lint]\nselect = [\"C010\"]\n"),
        ("legacy/jtk.toml", "[lint]\nselect = [\"C010\"]\n\n[lint.options.C010]\nmax = 8\n"),
        ("A.java", LONG_LIST),
        ("legacy/B.java", LONG_LIST),
    ];
    let d = Dir::new("nearest", &files);
    let (_, out, _) = jtk(&d, &["lint", &d.path("")]);
    assert_eq!(out, "A.java:2:10: C010 More than 7 parameters (found 8).\n");
    let (_, out, _) = jtk(&d, &["lint", "--config", &d.path("jtk.toml"), &d.path("")]);
    assert_eq!(out.lines().count(), 2);
}

#[test]
fn java_version_and_format_settings() {
    let source = "import b.B;\nimport a.A;\n\nclass C {\n    A a;\n    B b;\n\n    void f(Object o) {\n        if (o instanceof String) {\n            String s = (String) o;\n            a.use(s);\n        }\n    }\n}\n";
    let config = "java = 11\n\n[format]\nsort-imports = false\n\n[lint]\nselect = [\"M001\"]\n";
    let d = Dir::new("format", &[("jtk.toml", config), ("C.java", source)]);
    assert_eq!(jtk(&d, &["check", &d.path("C.java")]), (0, String::new(), String::new()));
    assert_eq!(jtk(&d, &["lint", "--java", "17", &d.path("C.java")]).0, 1);
    assert_eq!(jtk(&d, &["format", &d.path("C.java")]).1, source);
}

#[test]
fn configuration_errors() {
    let d = Dir::new("errors", &[("jtk.toml", "[lint]\nselect = [\"X1\"]\n"), ("A.java", "class A {}\n")]);
    let (code, _, err) = jtk(&d, &["lint", &d.path("A.java")]);
    assert_eq!((code, err), (2, format!("{}: no rule matches 'X1'\n", d.path("jtk.toml"))));

    std::fs::write(d.path("jtk.toml"), "[lint.options.C010]\nmaximum = 8\n").unwrap();
    let (code, _, err) = jtk(&d, &["lint", &d.path("A.java")]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown field `maximum`, expected `max`"), "{err}");
}

#[test]
fn rule_parameters_have_checkstyle_names() {
    let config = "[lint]\nselect = [\"I002\", \"N013\", \"A015\"]\n\n[lint.options]\nI002 = { illegal-pkgs = [\"com.acme\"] }\nN013 = { allowed-abbreviation-length = 1 }\nA015 = { ignore-numbers = [7] }\n";
    let source = "import com.acme.Internal;\n\nclass A {\n    int parseXMLDoc = 7;\n    int b = 1;\n}\n";
    let d = Dir::new("rule-parameters", &[("jtk.toml", config), ("A.java", source)]);
    let (_, out, _) = jtk(&d, &["lint", &d.path("A.java")]);
    let codes: Vec<&str> = out.lines().map(|l| l.split(' ').nth(1).unwrap()).collect();
    assert_eq!(codes, ["I002", "N013", "A015"]);
}

const NULL_CHECK: &str =
    "class A {\n    boolean f(String x) {\n        if (x.equals(null)) return true;\n        return false;\n    }\n}\n";

#[test]
fn fix_policy_decides_which_fixes_apply() {
    let config = "[lint]\nselect = [\"S001\", \"B009\"]\nunfixable = [\"S001\"]\nextend-safe-fixes = [\"B009\"]\n";
    let d = Dir::new("fix-policy", &[("jtk.toml", config), ("A.java", NULL_CHECK)]);
    let (code, out, _) = jtk(&d, &["lint", "--fix", &d.path("A.java")]);
    assert_eq!((code, out.as_str()), (1, "A.java:3:9: S001 'if' body without braces\n"));
    let fixed = std::fs::read_to_string(d.path("A.java")).unwrap();
    assert!(fixed.contains("if (x == null) return true;"), "{fixed}");
}

#[test]
fn reports_as_json() {
    let source = "class A {\n    boolean f(String x) {\n        if (x.equals(null)) return true;\n            return false;\n    }\n}\n";
    let d = Dir::new("json", &[("A.java", source)]);
    let (code, out, _) = jtk(&d, &["check", "--output-format", "json", &d.path("A.java")]);
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let expected = serde_json::json!({
        "unformatted": ["A.java"],
        "violations": [
            {"file": "A.java", "line": 3, "column": 9, "code": "S001", "message": "'if' body without braces", "fix": "safe"},
            {"file": "A.java", "line": 3, "column": 13, "code": "B009", "message": "equals(null) instead of == null", "fix": "unsafe"},
        ],
    });
    assert_eq!((code, report), (1, expected));
}
