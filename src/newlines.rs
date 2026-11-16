//! Platform-independent newline handling (port of `Newlines.java`).

/// Returns the start offsets (in bytes) of all lines in the input. Always starts with 0.
pub fn line_offsets(input: &str) -> Vec<usize> {
    let bytes = input.as_bytes();
    let mut offsets = vec![0];
    let mut idx = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'\r' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] == b'\n' {
                    idx += 1;
                }
                idx += 1;
                offsets.push(idx);
            }
            b'\n' => {
                idx += 1;
                offsets.push(idx);
            }
            _ => idx += 1,
        }
    }
    offsets
}

/// Returns the number of line breaks in the input.
pub fn count(input: &str) -> usize {
    line_offsets(input).len() - 1
}

/// Returns the byte index of the first break in the input, if any.
pub fn first_break(input: &str) -> Option<usize> {
    line_offsets(input).get(1).copied()
}

pub fn is_newline(input: &str) -> bool {
    matches!(input, "\r\n" | "\n" | "\r")
}

/// Returns the length of the newline sequence at the given offset, if any.
pub fn has_newline_at(input: &str, idx: usize) -> Option<usize> {
    let rest = &input.as_bytes()[idx..];
    if rest.starts_with(b"\r\n") {
        Some(2)
    } else if rest.starts_with(b"\n") || rest.starts_with(b"\r") {
        Some(1)
    } else {
        None
    }
}

/// Returns the terminating line break of the input, if it ends in one.
pub fn line_ending(input: &str) -> Option<&'static str> {
    if input.ends_with("\r\n") {
        Some("\r\n")
    } else if input.ends_with('\n') {
        Some("\n")
    } else if input.ends_with('\r') {
        Some("\r")
    } else {
        None
    }
}

/// Returns the first line separator in the text, or `"\n"` if there is none.
pub fn guess_line_separator(text: &str) -> &'static str {
    let bytes = text.as_bytes();
    for (i, &c) in bytes.iter().enumerate() {
        match c {
            b'\r' => {
                return if i + 1 < bytes.len() && bytes[i + 1] == b'\n' { "\r\n" } else { "\r" };
            }
            b'\n' => return "\n",
            _ => {}
        }
    }
    "\n"
}

pub fn contains_breaks(text: &str) -> bool {
    text.contains(['\n', '\r'])
}

/// Returns the lines of the input, each including its trailing line break.
pub fn lines(input: &str) -> Vec<&str> {
    let offsets = line_offsets(input);
    let mut result = Vec::new();
    for (i, &start) in offsets.iter().enumerate() {
        let end = offsets.get(i + 1).copied().unwrap_or(input.len());
        if start < end {
            result.push(&input[start..end]);
        }
    }
    result
}

/// Java's `String.length()`: the number of UTF-16 code units.
pub fn jlen(s: &str) -> usize {
    if s.is_ascii() { s.len() } else { s.encode_utf16().count() }
}
