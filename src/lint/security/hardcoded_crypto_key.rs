//! V008: a cryptographic key written in the source, such as `new SecretKeySpec("secret".getBytes(), "AES")` (PMD
//! HardCodedCryptoKey): everyone with the code (or the class files) has the key.
//!
//! No fix. Checks the key argument of `javax.crypto.spec.SecretKeySpec` constructors (resolved through the file's
//! imports). Hard-coded keys are the bytes of a string literal (`"...".getBytes(...)`), a string literal decoded
//! (`Base64.getDecoder().decode("...")`, `Hex.decodeHex("...")`), an array of literals (`new byte[] {1, 2}`) or a
//! new array of zeros (`new byte[16]`), directly or through a `final` variable or field initialized with one (except a
//! new array of zeros, which is usually filled before use), or a `final` string variable or field initialized with a
//! literal (`KEY.getBytes()`). Unlike PMD, variables that are not `final` are left out, as they may be assigned
//! another key.

use crate::lint::logging::shared::{Declared, constant_string, declaration, names_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

/// The methods that decode a key from a string.
const DECODERS: &[&str] = &["decode", "decodeBase64", "decodeHex", "parseHexBinary", "parseBase64Binary"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { ident, args, body: None, .. } = &tree.kind else { return };
    let Some(unit) = context.ancestors.first() else { return };
    let Some(key) = args.first() else { return };
    if !names_type(ident, "javax.crypto.spec.SecretKeySpec", unit) {
        return;
    }
    let mut ancestors = context.ancestors.clone();
    ancestors.push(tree);
    let text = context.text;
    let final_init = |name| final_variable_init(name, &ancestors, context);
    let key = strip_parens(key);
    let hardcoded = is_literal_key(key, text, true)
        || final_init(key).is_some_and(|init| is_literal_key(init, text, false))
        || bytes_of(key).and_then(final_init).is_some_and(|init| constant_string(init, text).is_some());
    if hardcoded {
        diagnostics.push(Diagnostic::new(Rule::HardcodedCryptoKey, key.start, "hard-coded cryptographic key"));
    }
}

/// Whether the expression is a key written in the source: the bytes of a string literal, a decoded string literal,
/// an array of literals, or, if `zeros` is set, a new array of zeros.
fn is_literal_key(key: &Tree, text: &str, zeros: bool) -> bool {
    match &strip_parens(key).kind {
        K::MethodInvocation { select, args, .. } => {
            let K::MemberSelect { name, .. } = &select.kind else { return false };
            match args.as_slice() {
                [argument] if DECODERS.contains(&name.as_str()) => constant_string(argument, text).is_some(),
                _ => bytes_of(key).is_some_and(|string| constant_string(string, text).is_some()),
            }
        }
        K::NewArray { dims, init: Some(elements), .. } => {
            dims.is_empty() && !elements.is_empty() && elements.iter().all(is_literal)
        }
        K::NewArray { dims, init: None, .. } => zeros && dims.iter().all(|d| matches!(d.kind, K::Literal(_))),
        _ => false,
    }
}

/// The string `s` of `s.getBytes(...)`.
fn bytes_of(tree: &Tree) -> Option<&Tree> {
    let K::MethodInvocation { select, .. } = &strip_parens(tree).kind else { return None };
    match &select.kind {
        K::MemberSelect { expr, name } if name == "getBytes" => Some(strip_parens(expr)),
        _ => None,
    }
}

/// Whether an array element is a literal, possibly negated or cast (`(byte) 0xff`).
fn is_literal(element: &Tree) -> bool {
    match &strip_parens(element).kind {
        K::Literal(_) => true,
        K::Unary { expr, .. } | K::Cast { expr, .. } => is_literal(expr),
        _ => false,
    }
}

/// The initializer of the `final` variable or field that the name `tree` (below `ancestors`) refers to.
fn final_variable_init<'t>(tree: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> Option<&'t Tree> {
    let K::Ident(name) = &tree.kind else { return None };
    match declaration(tree, ancestors, name, context)? {
        Declared::Var(var) if var.mods.has(Modifier::Final) => var.init.as_deref(),
        _ => None,
    }
}
