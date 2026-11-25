//! Rules of group V: security.

mod bidi_character;
mod hardcoded_crypto_key;

use super::Rule;

use jtk_parser::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::HardcodedCryptoKey, &[Tag::NewClass], hardcoded_crypto_key::check),
    (Rule::BidiCharacter, &[Tag::CompilationUnit], bidi_character::check),
];
