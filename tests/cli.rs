//! Command line behaviour. Expected outputs were produced with the original palantir-java-format 2.103.0
//! (`--palantir`).

use std::path::PathBuf;

use jtk::cli::run;

const UNFORMATTED: &str = "class A {\n  int x=1;\n    int y=2;\n}\n";
const FORMATTED: &str = "class A {\n    int x = 1;\n    int y = 2;\n}\n";
const UNICODE: &str = "class U {\n  // äöü \n  String s=\"ä€𝄞\";   int  z=1;\n}\n";
const IMPORTS: &str =
    "import java.util.List;\nimport java.util.Map;\nimport b.Z;\nimport a.Y;\nclass C { Map m; Z z;    Y y; }\n";

/// A temporary directory with the given files, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str, files: &[(&str, &str)]) -> Dir {
        let dir = std::env::temp_dir().join(format!("pjf-cli-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (file, content) in files {
            std::fs::write(dir.join(file), content).unwrap();
        }
        Dir(dir)
    }

    fn path(&self, file: &str) -> String {
        self.0.join(file).to_string_lossy().into_owned()
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.0.join(file)).unwrap()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the tool, returning (exit code, stdout, stderr).
fn pjf(args: &[&str], stdin: &str) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut stdin.as_bytes(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn formats_files_to_stdout_in_order() {
    let d = Dir::new("order", &[("a.java", UNFORMATTED), ("b.java", "class B {}\n")]);
    let (code, out, _) = pjf(&[&d.path("a.java"), &d.path("b.java")], "");
    assert_eq!((code, out), (0, format!("{FORMATTED}class B {{}}\n")));
}

#[test]
fn stdin() {
    assert_eq!(pjf(&["-"], UNFORMATTED), (0, FORMATTED.into(), String::new()));
    assert_eq!(pjf(&["--palantir", "-", "--assume-filename=foo.java", "-n"], UNFORMATTED).1, "foo.java\n");
}

#[test]
fn replace_in_place() {
    let d = Dir::new("replace", &[("a.java", UNFORMATTED)]);
    assert_eq!(pjf(&["-i", &d.path("a.java")], ""), (0, String::new(), String::new()));
    assert_eq!(d.read("a.java"), FORMATTED);
}

#[test]
fn dry_run_and_exit_if_changed() {
    let d = Dir::new("dry", &[("a.java", UNFORMATTED), ("b.java", FORMATTED)]);
    let (a, b) = (d.path("a.java"), d.path("b.java"));
    assert_eq!(pjf(&["--dry-run", &a, &b], ""), (0, format!("{a}\n"), String::new()));
    assert_eq!(pjf(&["-n", "--set-exit-if-changed", &a], "").0, 1);
    assert_eq!(pjf(&["--set-exit-if-changed", &a], ""), (1, FORMATTED.into(), String::new()));
    assert_eq!(pjf(&["--set-exit-if-changed", &b], "").0, 0);
    assert_eq!(d.read("a.java"), UNFORMATTED);
}

#[test]
fn partial_formatting() {
    let d = Dir::new("partial", &[("a.java", UNFORMATTED), ("u.java", UNICODE)]);
    let a = d.path("a.java");
    let only_line_3 = "class A {\n  int x=1;\n    int y = 2;\n}\n";
    assert_eq!(pjf(&["--lines", "3", &a], "").1, only_line_3);
    assert_eq!(pjf(&["--offset", "25", "--length", "0", &a], "").1, only_line_3);
    // Offset 20 is the line break after line 2: no token there, nothing to format.
    assert_eq!(pjf(&["--offset", "20", "--length", "0", &a], "").1, UNFORMATTED);
    assert_eq!(pjf(&["--character-ranges", "15:20", &a], "").1, "class A {\n    int x = 1;\n    int y=2;\n}\n");
    assert_eq!(
        pjf(&["--lines", "3", &d.path("u.java")], "").1,
        "class U {\n    // äöü\n    String s = \"ä€𝄞\";\n    int z = 1;\n}\n"
    );
}

#[test]
fn output_replacements() {
    let d = Dir::new("replacements", &[("a.java", UNFORMATTED), ("u.java", UNICODE)]);
    assert_eq!(
        pjf(&["--output-replacements", "--lines", "3", &d.path("a.java")], "").1,
        r#"[{"replaceRange":{"lowerEndpoint":20,"lowerBoundType":"CLOSED","upperEndpoint":33,"upperBoundType":"OPEN"},"replacementString":"\n    int y = 2;"}]"#
    );
    // Offsets count UTF-16 code units, like Java.
    assert_eq!(
        pjf(&["--output-replacements", &d.path("u.java")], "").1,
        r#"[{"replaceRange":{"lowerEndpoint":0,"lowerBoundType":"CLOSED","upperEndpoint":53,"upperBoundType":"OPEN"},"replacementString":"class U {\n    // äöü\n    String s = \"ä€𝄞\";\n    int z = 1;\n}\n"}]"#
    );
}

#[test]
fn import_options() {
    let d = Dir::new("imports", &[("i.java", IMPORTS)]);
    let i = d.path("i.java");
    assert_eq!(
        pjf(&["--fix-imports-only", &i], "").1,
        "import a.Y;\nimport b.Z;\nimport java.util.Map;\n\nclass C { Map m; Z z;    Y y; }\n"
    );
    let unsorted_kept = pjf(&["--fix-imports-only", "--skip-sorting-imports", &i], "").1;
    assert!(unsorted_kept.starts_with("import java.util.Map;\nimport b.Z;\nimport a.Y;\n"), "{unsorted_kept}");
    assert!(pjf(&["--skip-removing-unused-imports", &i], "").1.contains("import java.util.List;"));
}

#[test]
fn params_file() {
    let d = Dir::new("params", &[("a.java", UNFORMATTED)]);
    std::fs::write(d.0.join("args.txt"), format!("--palantir\n{}\n", d.path("a.java"))).unwrap();
    assert_eq!(pjf(&[&format!("@{}", d.path("args.txt"))], "").1, FORMATTED);
}

#[test]
fn file_errors() {
    let d = Dir::new("errors", &[("bad.java", "class {\n"), ("ok.java", "class B {}\n")]);
    let (code, out, err) = pjf(&[&d.path("bad.java"), &d.path("ok.java")], "");
    assert_eq!((code, out.as_str()), (1, "class B {}\n"));
    assert!(err.starts_with(&format!("{}:1:", d.path("bad.java"))), "{err}");
    assert_eq!(pjf(&["x.txt"], ""), (0, String::new(), "Skipping non-Java file: x.txt\n".into()));
    assert_eq!(pjf(&["missing.java"], "").2, "missing.java: could not read file: missing.java\n");
}

#[test]
fn usage_errors_print_usage_and_exit_zero() {
    for (args, message) in [
        (&["--bogus", "a.java"][..], "unexpected flag: --bogus"),
        (&[][..], "no files were provided"),
        (&["-i"][..], "in-place formatting was requested but no files were provided"),
        (&["--lines", "2", "a.java", "b.java"][..], "partial formatting is only support for a single file"),
        (&["--offset", "5", "a.java"][..], "-offsets and -lengths flags must be provided in matching pairs"),
        (&["-i", "-n", "a.java"][..], "cannot use --dry-run and --in-place at the same time"),
        (&["--length=x", "a.java"][..], "invalid integer value for --length: x"),
        (&["--lines", "x", "a.java"][..], "For input string: \"x\""),
        (&["--aosp", "a.java"][..], "the AOSP style is not supported by this port, only the Palantir style"),
    ] {
        let (code, out, err) = pjf(args, "");
        assert_eq!((code, out.as_str()), (0, ""), "{args:?}");
        assert!(err.starts_with(&format!("{message}\n")), "{args:?}: {err}");
        assert!(err.contains("Usage: jtk [options] file(s)"));
    }
}

#[test]
fn files_after_the_first_file_are_all_files() {
    let d = Dir::new("trailing", &[("a.java", UNFORMATTED)]);
    let (code, out, err) = pjf(&[&d.path("a.java"), "-i"], "");
    assert_eq!((code, out.as_str(), err.as_str()), (0, FORMATTED, "Skipping non-Java file: -i\n"));
}
