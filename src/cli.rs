// Derived from palantir-java-format's Main.java, CommandLineOptionsParser.java and FormatFileCallable.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! The command line interface, compatible with the original's `Main`, `CommandLineOptionsParser` and
//! `FormatFileCallable`.
//!
//! Differences: only the Palantir style is supported (`--palantir` is accepted and implied, `--aosp` is rejected),
//! and diagnostics come from this port's parser rather than javac.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::config::{Configs, LintArguments, java_files};
use crate::formatter::{
    FormatError, FormatOptions, fix_imports, format_and_lint, format_file, format_replacements_with_reflow,
};
use crate::lint::{self, Diagnostic, Selection, Settings};
use crate::newlines;
use crate::output::Replacement;

const STDIN_FILENAME: &str = "<stdin>";

pub fn version_string() -> String {
    format!("jtk: Version {} (compatible with palantir-java-format 2.103.0)", env!("CARGO_PKG_VERSION"))
}

const USAGE: &str = "
Usage: jtk [options] file(s)

Options:
  -i, -r, -replace, --replace
    Send formatted output back to files, not stdout.
  -
    Format stdin -> stdout
  --assume-filename, -assume-filename
    File name to use for diagnostics when formatting standard input (default is <stdin>).
  --palantir, -palantir
    Use Palantir style. Accepted for compatibility: this port always uses the Palantir style.
  --fix-imports-only
    Fix import order and remove any unused imports, but do no other formatting.
  --skip-sorting-imports
    Do not fix the import order. Unused imports will still be removed.
  --skip-removing-unused-imports
    Do not remove unused imports. Imports will still be sorted.
  --skip-reflowing-long-strings
    Do not reflow string literals that exceed the column limit.
  --format-javadoc
    Reflow javadoc comments (not done by default, as in the original's Palantir style).
  --config
    The jtk.toml whose [format] settings apply, instead of the nearest one in each file's directory or above.
  --dry-run, -n
    Prints the paths of the files whose contents would change if the formatter were run normally.
  --set-exit-if-changed
    Return exit code 1 if there are any formatting changes.
  --lines, -lines, --line, -line
    Line range(s) to format, like 5:10 (1-based; default is all).
  --character-ranges, -character-ranges, --character-range, -character-range
    character range(s) to format, like 5:10 (0-based; default is all).
  --offset, -offset
    Character offset to format (0-based; default is all).
  --length, -length
    Character length to format.
  --help, -help, -h
    Print this usage statement.
  --version, -version, -v
    Print the version.
  --output-replacements, -output-replacements
    Outputs formatting replacements instead of the full formatted file.
  @<filename>
    Read options and filenames from file.

If -i is given with -, the result is sent to stdout.
The --lines, --offset, and --length flags may be given more than once.
The --offset and --length flags must be given an equal number of times.
If --lines, --offset, or --length are given, only one file (or -) may be given.
";

/// Parsed command line options (port of `CommandLineOptions`).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Options {
    pub files: Vec<String>,
    pub in_place: bool,
    /// 0-based, closed-open line ranges.
    pub lines: Vec<(i64, i64)>,
    /// 0-based, closed-open ranges of UTF-16 code units.
    pub character_ranges: Vec<(i64, i64)>,
    pub offsets: Vec<i64>,
    pub lengths: Vec<i64>,
    pub aosp: bool,
    pub version: bool,
    pub help: bool,
    pub stdin: bool,
    pub fix_imports_only: bool,
    pub skip_sorting_imports: bool,
    pub skip_removing_unused_imports: bool,
    pub skip_reflowing_long_strings: bool,
    pub format_javadoc: bool,
    pub dry_run: bool,
    pub set_exit_if_changed: bool,
    pub assume_filename: Option<String>,
    pub output_replacements: bool,
    pub config: Option<String>,
}

impl Options {
    fn is_selection(&self) -> bool {
        !self.lines.is_empty() || !self.offsets.is_empty() || !self.character_ranges.is_empty()
    }

    fn format_options(&self) -> FormatOptions {
        FormatOptions {
            remove_unused_imports: !self.skip_removing_unused_imports,
            sort_imports: !self.skip_sorting_imports,
            reflow_long_strings: !self.skip_reflowing_long_strings,
            format_javadoc: self.format_javadoc,
            ..FormatOptions::default()
        }
    }
}

/// A usage error: printed together with the usage text.
#[derive(Debug, PartialEq)]
pub struct UsageError(pub String);

/// Expands `@file` arguments into the whitespace-separated arguments in the file; `@@x` stands for a literal `@x`.
fn expand_params_files(args: &[String], out: &mut Vec<String>) -> Result<(), UsageError> {
    for arg in args {
        if arg.is_empty() {
            continue;
        }
        if let Some(rest) = arg.strip_prefix("@@") {
            out.push(format!("@{rest}"));
        } else if let Some(path) = arg.strip_prefix('@') {
            let content = std::fs::read_to_string(path)
                .map_err(|e| UsageError(format!("{path}: could not read file: {}", read_error_message(path, &e))))?;
            let args: Vec<String> = content.split_whitespace().map(str::to_string).collect();
            expand_params_files(&args, out)?;
        } else {
            out.push(arg.clone());
        }
    }
    Ok(())
}

fn parse_int(s: &str) -> Result<i64, UsageError> {
    s.parse::<i32>().map(i64::from).map_err(|_| UsageError(format!("For input string: \"{s}\"")))
}

/// Parses `5:10` or `5` into a closed range.
fn parse_closed_range(s: &str) -> Result<(i64, i64), UsageError> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        [a] => {
            let a = parse_int(a)?;
            Ok((a, a))
        }
        [a, b] => Ok((parse_int(a)?, parse_int(b)?)),
        _ => Err(UsageError(s.to_string())),
    }
}

/// Parses the command line (port of `CommandLineOptionsParser.parse`).
pub fn parse(args: &[String]) -> Result<Options, UsageError> {
    let mut expanded = Vec::new();
    expand_params_files(args, &mut expanded)?;
    let mut options = Options::default();
    let mut it = expanded.into_iter();
    while let Some(option) = it.next() {
        if !option.starts_with('-') {
            // The first non-flag argument starts the list of files.
            options.files.push(option);
            options.files.extend(it.by_ref());
            break;
        }
        let (flag, inline_value) = match option.find('=') {
            Some(idx) => (option[..idx].to_string(), Some(option[idx + 1..].to_string())),
            None => (option.clone(), None),
        };
        let value = |it: &mut std::vec::IntoIter<String>| -> Result<String, UsageError> {
            match inline_value.clone() {
                Some(v) => Ok(v),
                None => it.next().ok_or_else(|| UsageError(format!("required value was not provided for: {flag}"))),
            }
        };
        match flag.as_str() {
            "-i" | "-r" | "-replace" | "--replace" => options.in_place = true,
            "--lines" | "-lines" | "--line" | "-line" => {
                for range in value(&mut it)?.split(',') {
                    let (a, b) = parse_closed_range(range)?;
                    options.lines.push((a - 1, b));
                }
            }
            "--character-ranges" | "-character-ranges" | "--character-range" | "-character-range" => {
                for range in value(&mut it)?.split(',') {
                    let (a, b) = parse_closed_range(range)?;
                    options.character_ranges.push((a, b + 1));
                }
            }
            "--offset" | "-offset" | "--length" | "-length" => {
                let v = value(&mut it)?;
                // The original reports the inline value, which is `null` for `--length 5`-style arguments.
                let shown = inline_value.clone().unwrap_or_else(|| "null".into());
                let n =
                    v.parse::<i32>().map_err(|_| UsageError(format!("invalid integer value for {flag}: {shown}")))?;
                if flag.ends_with("offset") {
                    options.offsets.push(n.into());
                } else {
                    options.lengths.push(n.into());
                }
            }
            "--aosp" | "-aosp" | "-a" => options.aosp = true,
            "--palantir" | "-palantir" => {}
            "--version" | "-version" | "-v" => options.version = true,
            "--help" | "-help" | "-h" => options.help = true,
            "--fix-imports-only" => options.fix_imports_only = true,
            "--skip-sorting-imports" => options.skip_sorting_imports = true,
            "--skip-removing-unused-imports" => options.skip_removing_unused_imports = true,
            "--skip-reflowing-long-strings" => options.skip_reflowing_long_strings = true,
            "--format-javadoc" => options.format_javadoc = true,
            "-" => options.stdin = true,
            "-n" | "--dry-run" => options.dry_run = true,
            "--set-exit-if-changed" => options.set_exit_if_changed = true,
            "-assume-filename" | "--assume-filename" => options.assume_filename = Some(value(&mut it)?),
            "-output-replacements" | "--output-replacements" => options.output_replacements = true,
            "--config" => options.config = Some(value(&mut it)?),
            _ => return Err(UsageError(format!("unexpected flag: {flag}"))),
        }
    }
    Ok(options)
}

/// Validates the options (port of `Main.processArgs`).
fn validate(o: &Options) -> Result<(), UsageError> {
    let err = |m: &str| Err(UsageError(m.to_string()));
    if o.aosp {
        return err("the AOSP style is not supported by this port, only the Palantir style");
    }
    let files_to_format = o.files.len() + usize::from(o.stdin);
    if o.in_place && o.files.is_empty() {
        return err("in-place formatting was requested but no files were provided");
    }
    if o.is_selection() && files_to_format != 1 {
        return err("partial formatting is only support for a single file");
    }
    if o.offsets.len() != o.lengths.len() {
        return err("-offsets and -lengths flags must be provided in matching pairs");
    }
    if files_to_format == 0 && !o.version && !o.help {
        return err("no files were provided");
    }
    if o.stdin && !o.files.is_empty() {
        return err("cannot format from standard input and files simultaneously");
    }
    if o.assume_filename.is_some() && !o.stdin {
        return err("--assume-filename is only supported when formatting standard input");
    }
    if o.dry_run && o.in_place {
        return err("cannot use --dry-run and --in-place at the same time");
    }
    Ok(())
}

/// Converts an offset in UTF-16 code units (as used on the command line and in replacements) to a byte offset.
fn utf16_to_byte(text: &str, offset: i64) -> usize {
    if offset <= 0 {
        return 0;
    }
    let mut units = 0i64;
    for (i, c) in text.char_indices() {
        if units >= offset {
            return i;
        }
        units += c.len_utf16() as i64;
    }
    text.len() + (offset - units).max(0) as usize
}

fn byte_to_utf16(text: &str, offset: usize) -> usize {
    newlines::jlen(&text[..offset.min(text.len())])
}

/// The byte ranges to format (port of `FormatFileCallable.characterRanges`).
fn character_ranges(o: &Options, input: &str) -> Vec<(usize, usize)> {
    if o.character_ranges.is_empty() && o.lines.is_empty() && o.offsets.is_empty() {
        return vec![(0, input.len())];
    }
    let mut ranges = Vec::new();
    for &(start, end) in &o.character_ranges {
        ranges.push((utf16_to_byte(input, start), utf16_to_byte(input, end)));
    }
    if !o.lines.is_empty() {
        // `Utils.lineRangesToCharRanges`: line starts, plus a sentinel one past the end.
        let mut starts = newlines::line_offsets(input);
        starts.push(input.len() + 1);
        let line_count = (starts.len() - 1) as i64;
        for &(lo, hi) in &o.lines {
            let (lo, hi) = (lo.max(0), hi.min(line_count));
            if lo >= hi {
                continue;
            }
            ranges.push((starts[lo as usize], starts[hi as usize] - 1));
        }
    }
    for (&offset, &length) in o.offsets.iter().zip(&o.lengths) {
        // 0 stands for "format the line under the cursor".
        let length = if length == 0 { 1 } else { length };
        ranges.push((utf16_to_byte(input, offset), utf16_to_byte(input, offset + length)));
    }
    ranges
}

/// Formats one input (port of `FormatFileCallable.call`) with the formatting steps of its configuration.
fn format_input(o: &Options, input: &str, options: &FormatOptions) -> Result<String, FormatError> {
    if o.fix_imports_only {
        return fix_imports(input, options);
    }
    let ranges = character_ranges(o, input);
    if o.output_replacements {
        let replacements = format_replacements_with_reflow(input, &ranges, options)?;
        return Ok(replacements_json(input, &replacements));
    }
    format_file(input, &ranges, options)
}

/// The formatting steps for each file, from the command line and the files' configurations.
fn format_options(o: &Options, files: &[&str]) -> Result<Vec<FormatOptions>, String> {
    let mut configs = Configs::new(o.config.as_deref().map(Path::new))?;
    files.iter().map(|file| Ok(configs.for_file(Path::new(file))?.format_options(o.format_options()))).collect()
}

/// Serializes replacements like the original (Jackson with Guava's `Range` serializer), with UTF-16 offsets.
fn replacements_json(input: &str, replacements: &[Replacement]) -> String {
    let mut sorted: Vec<&Replacement> = replacements.iter().collect();
    sorted.sort_by_key(|r| r.start);
    let items: Vec<String> = sorted
        .iter()
        .map(|r| {
            format!(
                "{{\"replaceRange\":{{\"lowerEndpoint\":{},\"lowerBoundType\":\"CLOSED\",\"upperEndpoint\":{},\
                 \"upperBoundType\":\"OPEN\"}},\"replacementString\":{}}}",
                byte_to_utf16(input, r.start),
                byte_to_utf16(input, r.end),
                json_string(&r.text)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Runs the command line tool; returns the exit code.
pub fn run(args: &[String], stdin: &mut dyn Read, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match args.split_first().map(|(command, rest)| (command.as_str(), rest)) {
        Some(("lint", files)) => return lint_files(files, out, err),
        Some(("check", args)) => return check_files(args, out, err),
        _ => {}
    }
    let options = match parse(args).and_then(|o| validate(&o).map(|()| o)) {
        Ok(o) => o,
        Err(UsageError(message)) => {
            // Like the original, usage errors print the usage and exit with 0.
            let _ = write!(err, "{message}\n{USAGE}\n{}\n", version_string());
            return 0;
        }
    };
    if options.version {
        let _ = writeln!(err, "{}", version_string());
        return 0;
    }
    if options.help {
        let _ = write!(err, "{USAGE}\n{}\n", version_string());
        return 0;
    }
    if options.stdin { format_stdin(&options, stdin, out, err) } else { format_files(&options, out, err) }
}

fn format_stdin(o: &Options, stdin: &mut dyn Read, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut input = String::new();
    if let Err(e) = stdin.read_to_string(&mut input) {
        let _ = writeln!(err, "{STDIN_FILENAME}: could not read standard input: {e}");
        return 1;
    }
    let name = o.assume_filename.as_deref().unwrap_or(STDIN_FILENAME);
    let options = match format_options(o, &[name]) {
        Ok(options) => options[0],
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    match format_input(o, &input, &options) {
        Ok(output) => {
            let changed = output != input;
            if o.dry_run {
                if changed {
                    let _ = writeln!(out, "{name}");
                }
            } else {
                let _ = out.write_all(output.as_bytes());
            }
            if changed && o.set_exit_if_changed { 1 } else { 0 }
        }
        Err(e) => {
            let _ = writeln!(err, "{name}:{e}");
            1
        }
    }
}

fn read_error_message(file: &str, e: &std::io::Error) -> String {
    match e.kind() {
        // Java's NoSuchFileException and AccessDeniedException messages are just the path.
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => file.to_string(),
        _ => e.to_string(),
    }
}

fn format_files(o: &Options, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (inputs, mut all_ok) = read_inputs(&o.files, err);
    let files: Vec<&str> = inputs.iter().map(|(file, _)| *file).collect();
    let options = match format_options(o, &files) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    let jobs: Vec<_> = inputs.iter().zip(&options).collect();
    let results = map_parallel(&jobs, |((_, text), options)| format_input(o, text, options));
    for ((file, input), result) in inputs.iter().zip(results) {
        let formatted = match result {
            Ok(formatted) => formatted,
            Err(e) => {
                let _ = writeln!(err, "{file}:{e}");
                all_ok = false;
                continue;
            }
        };
        let changed = &formatted != input;
        if changed && o.set_exit_if_changed {
            all_ok = false;
        }
        if o.in_place {
            if changed && let Err(e) = std::fs::write(file, &formatted) {
                let _ = writeln!(err, "{file}: could not write file: {e}");
                all_ok = false;
            }
        } else if o.dry_run {
            if changed {
                let _ = writeln!(out, "{file}");
            }
        } else {
            let _ = out.write_all(formatted.as_bytes());
        }
    }
    if all_ok { 0 } else { 1 }
}

/// Reads the Java files, reporting unreadable and skipped files. Returns the inputs and whether all could be read.
fn read_inputs<'a>(files: &'a [String], err: &mut dyn Write) -> (Vec<(&'a str, String)>, bool) {
    let mut all_ok = true;
    let mut inputs = Vec::new();
    for file in files {
        if !file.ends_with(".java") {
            let _ = writeln!(err, "Skipping non-Java file: {file}");
            continue;
        }
        match std::fs::read_to_string(file) {
            Ok(text) => inputs.push((file.as_str(), text)),
            Err(e) => {
                let _ = writeln!(err, "{file}: could not read file: {}", read_error_message(file, &e));
                all_ok = false;
            }
        }
    }
    (inputs, all_ok)
}

/// Applies `f` to each input on all cores; returns the results in input order.
fn map_parallel<I: Sync, T: Send>(inputs: &[I], f: impl Fn(&I) -> T + Sync) -> Vec<T> {
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(inputs.len());
    if workers <= 1 {
        return inputs.iter().map(&f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut results: Vec<(usize, T)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(input) = inputs.get(i) else { break };
                        done.push((i, f(input)));
                    }
                    done
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("worker panicked")).collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}

const LINT_USAGE: &str = "Usage: jtk lint [options] path(s)

Prints one line per rule violation: file:line:column: CODE message. Directories are searched for .java files.
  --select CODES  Only check rules whose code starts with one of the comma-separated prefixes (e.g. B,S001),
                  ALL for all rules. Without it, the configured or default rules are checked.
  --extend-select CODES  Also check the rules matching the prefixes.
  --ignore CODES  Do not check rules whose code starts with one of the prefixes.
  --java VERSION  The project's Java version: skip rules that suggest newer language features.
  --fix           Apply automatic fixes in place, and report the remaining violations.
  --unsafe-fixes  With --fix, also apply the fixes that may change the behaviour in edge cases.
  --config FILE   The jtk.toml to use instead of the nearest one in each file's directory or above.
Exits with 1 if there are violations or errors, and with 2 on usage or configuration errors.";

const CHECK_USAGE: &str = "Usage: jtk check [options] path(s)

Formats and lints the files. Prints the files that are not formatted and one line per rule violation:
file:line:column: CODE message. Directories are searched for .java files.
  -i              Format the files in place and lint the formatted text.
  --select CODES  Only check rules whose code starts with one of the comma-separated prefixes (e.g. B,S001),
                  ALL for all rules. Without it, the configured or default rules are checked.
  --extend-select CODES  Also check the rules matching the prefixes.
  --ignore CODES  Do not check rules whose code starts with one of the prefixes.
  --java VERSION  The project's Java version: skip rules that suggest newer language features.
  --fix           Apply automatic fixes before formatting (implies -i).
  --unsafe-fixes  With --fix, also apply the fixes that may change the behaviour in edge cases.
  --config FILE   The jtk.toml to use instead of the nearest one in each file's directory or above.
Exits with 1 if a file is not formatted (without -i), has violations or errors, and with 2 on usage or
configuration errors.";

/// Options of `jtk lint` and `jtk check`.
struct LintOptions {
    paths: Vec<String>,
    in_place: bool,
    fix: bool,
    unsafe_fixes: bool,
    arguments: LintArguments,
    config: Option<String>,
}

fn parse_lint_options(args: &[String], allow_in_place: bool) -> Result<LintOptions, String> {
    let mut paths = Vec::new();
    let mut in_place = false;
    let mut fix = false;
    let mut unsafe_fixes = false;
    let mut arguments = LintArguments::default();
    let mut config = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-i" if allow_in_place => in_place = true,
            "--fix" => fix = true,
            "--unsafe-fixes" => unsafe_fixes = true,
            "--select" => arguments.select.extend(rule_codes(it.next())?),
            "--extend-select" => arguments.extend_select.extend(rule_codes(it.next())?),
            "--ignore" => arguments.ignore.extend(rule_codes(it.next())?),
            "--java" => arguments.java = Some(parse_java_version(it.next())?),
            "--config" => config = Some(it.next().ok_or("missing configuration file")?.clone()),
            flag if flag.starts_with('-') => return Err(format!("unknown option: {flag}")),
            path => paths.push(path.to_string()),
        }
    }
    if paths.is_empty() {
        return Err("no files given".to_string());
    }
    Selection::new(arguments.select.clone(), arguments.ignore.clone())?.extended(arguments.extend_select.clone())?;
    let in_place = in_place || (fix && allow_in_place);
    Ok(LintOptions { paths, in_place, fix, unsafe_fixes, arguments, config })
}

/// `17`, or `1.8` for 8.
fn parse_java_version(arg: Option<&String>) -> Result<u32, String> {
    let arg = arg.ok_or("missing Java version")?;
    let version = arg.strip_prefix("1.").unwrap_or(arg);
    version.parse().map_err(|_| format!("invalid Java version: {arg}"))
}

fn rule_codes(arg: Option<&String>) -> Result<Vec<String>, String> {
    let arg = arg.ok_or("missing rule codes")?;
    Ok(arg.split(',').map(str::trim).filter(|c| !c.is_empty()).map(str::to_string).collect())
}

/// A file to lint, with its text and the settings of its configuration.
struct LintJob {
    file: String,
    text: String,
    settings: Settings,
    format: FormatOptions,
}

/// The files to lint: reads the files given and found in the directories given, and finds their configurations.
/// Returns them and whether all files could be read, or a configuration error.
fn lint_jobs(o: &LintOptions, err: &mut dyn Write) -> Result<(Vec<LintJob>, bool), String> {
    let mut configs = Configs::new(o.config.as_deref().map(Path::new))?;
    let files = java_files(&o.paths, &mut configs)?;
    let (inputs, all_ok) = read_inputs(&files, err);
    let mut jobs = Vec::with_capacity(inputs.len());
    for (file, text) in inputs {
        let config = configs.for_file(Path::new(file))?;
        let settings = config.lint_settings(Path::new(file), &o.arguments)?;
        let format = config.format_options(FormatOptions::default());
        jobs.push(LintJob { file: file.to_string(), text, settings, format });
    }
    Ok((jobs, all_ok))
}

/// Parses the options and prepares the files of `jtk lint` or `jtk check`, or prints the usage or configuration
/// error and returns the exit code.
fn prepare(args: &[String], check: bool, err: &mut dyn Write) -> Result<(LintOptions, Vec<LintJob>, bool), i32> {
    let o = parse_lint_options(args, check).map_err(|message| {
        let usage = if check { CHECK_USAGE } else { LINT_USAGE };
        let _ = writeln!(err, "{message}\n\n{usage}");
        2
    })?;
    let (jobs, all_ok) = lint_jobs(&o, err).map_err(|message| {
        let _ = writeln!(err, "{message}");
        2
    })?;
    Ok((o, jobs, all_ok))
}

/// `jtk lint`: reports the rule violations in the given files.
fn lint_files(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (o, jobs, mut all_ok) = match prepare(args, false, err) {
        Ok(prepared) => prepared,
        Err(code) => return code,
    };
    let results = map_parallel(&jobs, |job| lint_input(job, &o));
    for (job, result) in jobs.iter().zip(results) {
        let file = &job.file;
        match result {
            Ok((text, diagnostics)) => {
                if text != job.text
                    && let Err(e) = std::fs::write(file, &text)
                {
                    let _ = writeln!(err, "{file}: could not write file: {e}");
                    all_ok = false;
                }
                all_ok &= report_violations(file, &text, &diagnostics, out);
            }
            Err(e) => {
                let _ = writeln!(err, "{file}:{e}");
                all_ok = false;
            }
        }
    }
    if all_ok { 0 } else { 1 }
}

/// Returns the (with `--fix`, fixed) text and its violations.
fn lint_input(job: &LintJob, o: &LintOptions) -> Result<(String, Vec<Diagnostic>), FormatError> {
    let input = &job.text;
    let text = if o.fix { lint::fix_with(input, &job.settings, o.unsafe_fixes)? } else { input.clone() };
    let diagnostics = lint::lint_file(&text, Some(Path::new(&job.file)), &job.settings)?;
    Ok((text, diagnostics))
}

/// `jtk check`: formats and lints the given files in one pass.
fn check_files(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (o, jobs, mut all_ok) = match prepare(args, true, err) {
        Ok(prepared) => prepared,
        Err(code) => return code,
    };
    let results = map_parallel(&jobs, |job| check_input(job, &o));
    for (job, result) in jobs.iter().zip(results) {
        let (file, input) = (&job.file, &job.text);
        let (formatted, diagnostics) = match result {
            Ok(checked) => checked,
            Err(e) => {
                let _ = writeln!(err, "{file}:{e}");
                all_ok = false;
                continue;
            }
        };
        let changed = formatted != *input;
        if changed && o.in_place {
            if let Err(e) = std::fs::write(file, &formatted) {
                let _ = writeln!(err, "{file}: could not write file: {e}");
                all_ok = false;
            }
        } else if changed {
            let _ = writeln!(out, "{file}: not formatted");
            all_ok = false;
        }
        let on_disk = if o.in_place { &formatted } else { input };
        all_ok &= report_violations(file, on_disk, &diagnostics, out);
    }
    if all_ok { 0 } else { 1 }
}

/// Formats the input and lints the text that is on disk afterwards, so that positions match the file. Linting the
/// formatted text reuses the formatter's syntax tree; an unformatted input left unchanged needs its own parse.
fn check_input(job: &LintJob, o: &LintOptions) -> Result<(String, Vec<Diagnostic>), FormatError> {
    let input = &job.text;
    let fixed = if o.fix { lint::fix_with(input, &job.settings, o.unsafe_fixes)? } else { input.clone() };
    let path = Some(Path::new(&job.file));
    let (formatted, diagnostics) = format_and_lint(&fixed, path, &job.format, &job.settings)?;
    if o.in_place || formatted == *input {
        return Ok((formatted, diagnostics));
    }
    Ok((formatted, lint::lint_file(input, path, &job.settings)?))
}

/// Prints the violations as `file:line:column: CODE message`; returns whether there were none.
/// The diagnostics are ordered by position, so lines are counted from the previous one rather than from the start.
fn report_violations(file: &str, text: &str, diagnostics: &[Diagnostic], out: &mut dyn Write) -> bool {
    let (mut counted_to, mut lines_before) = (0, 0);
    for d in diagnostics {
        let (line_in_rest, column) = newlines::line_column(&text[counted_to..], d.offset - counted_to);
        let line = lines_before + line_in_rest;
        let line_start = text[..d.offset].rfind(['\n', '\r']).map_or(0, |i| i + 1).max(counted_to);
        lines_before = line - 1;
        counted_to = line_start;
        let _ = writeln!(out, "{file}:{line}:{column}: {} {}", d.rule.code(), d.message);
    }
    diagnostics.is_empty()
}
