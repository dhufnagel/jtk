// Derived from palantir-java-format's Main.java, CommandLineOptionsParser.java and FormatFileCallable.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! The command line interface, compatible with the original's `Main`, `CommandLineOptionsParser` and
//! `FormatFileCallable`.
//!
//! Differences: only the Palantir style is supported (`--palantir` is accepted and implied, `--aosp` is rejected),
//! and diagnostics come from this port's parser rather than javac.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::formatter::{FormatError, FormatOptions, fix_imports, format_file, format_replacements_with_reflow};
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

/// Formats one input (port of `FormatFileCallable.call`).
fn format_input(o: &Options, input: &str) -> Result<String, FormatError> {
    let options = o.format_options();
    if o.fix_imports_only {
        return fix_imports(input, &options);
    }
    let ranges = character_ranges(o, input);
    if o.output_replacements {
        let replacements = format_replacements_with_reflow(input, &ranges, &options)?;
        return Ok(replacements_json(input, &replacements));
    }
    format_file(input, &ranges, &options)
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
    match format_input(o, &input) {
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
    let mut all_ok = true;
    let mut inputs: Vec<(&str, String)> = Vec::new();
    for file in &o.files {
        if !file.ends_with(".java") {
            let _ = writeln!(err, "Skipping non-Java file: {file}");
            continue;
        }
        match std::fs::read_to_string(file) {
            Ok(text) => inputs.push((file, text)),
            Err(e) => {
                let _ = writeln!(err, "{file}: could not read file: {}", read_error_message(file, &e));
                all_ok = false;
            }
        }
    }
    let results = format_parallel(o, &inputs);
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

/// Formats the inputs on all available cores, returning the results in order.
fn format_parallel(o: &Options, inputs: &[(&str, String)]) -> Vec<Result<String, FormatError>> {
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(inputs.len());
    if workers <= 1 {
        return inputs.iter().map(|(_, text)| format_input(o, text)).collect();
    }
    let next = AtomicUsize::new(0);
    let mut results: Vec<(usize, Result<String, FormatError>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some((_, text)) = inputs.get(i) else { break };
                        done.push((i, format_input(o, text)));
                    }
                    done
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("formatter worker panicked")).collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}
