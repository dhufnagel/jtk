// Derived from palantir-java-format's ModifierOrderer.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! Port of `ModifierOrderer.java`: fixes sequences of modifiers to be in JLS order.

use crate::input::JavaInput;

/// Modifiers in `javax.lang.model.element.Modifier` declaration order.
const ORDER: &[&str] = &[
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

fn as_modifier(text: &str) -> Option<usize> {
    ORDER.iter().position(|&m| m == text)
}

/// Reorders the modifiers starting within the given closed-open token ranges. Returns `None` if nothing changed.
pub fn reorder_modifiers(input: &JavaInput, token_ranges: &[(i32, i32)]) -> Option<String> {
    let tokens = &input.tokens;
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        i += 1;
        let index = input.tok(token.tok).index;
        if !token_ranges.iter().any(|&(lo, hi)| lo <= index && index < hi) {
            continue;
        }
        let Some(m) = as_modifier(&input.tok(token.tok).text) else {
            continue;
        };
        let mut mods = vec![m];
        let mut modifier_tokens = vec![token];
        let begin = input.tok(token.tok).position;
        let mut end = None;
        while i < tokens.len() {
            let t = &tokens[i];
            i += 1;
            let Some(m) = as_modifier(&input.tok(t.tok).text) else {
                break;
            };
            mods.push(m);
            modifier_tokens.push(t);
            let tok = input.tok(t.tok);
            end = Some(tok.position + tok.text.len());
        }
        if mods.windows(2).any(|w| w[0] > w[1]) {
            mods.sort_unstable();
            let mut replacement = String::new();
            for (j, &m) in mods.iter().enumerate() {
                if j > 0 {
                    for &b in &modifier_tokens[j].before {
                        replacement.push_str(&input.tok(b).text);
                    }
                }
                replacement.push_str(ORDER[m]);
                if j < modifier_tokens.len() - 1 {
                    for &a in &modifier_tokens[j].after {
                        replacement.push_str(&input.tok(a).text);
                    }
                }
            }
            replacements.push((begin, end.unwrap(), replacement));
        }
    }
    if replacements.is_empty() {
        return None;
    }
    let mut text = input.text.clone();
    for (begin, end, replacement) in replacements.into_iter().rev() {
        text.replace_range(begin..end, &replacement);
    }
    Some(text)
}
