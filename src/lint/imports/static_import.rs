//! I003: a static import such as `import static java.lang.Math.max;` (Checkstyle AvoidStaticImport with its default
//! properties): the member reads as if it were declared in the class.

use super::{imported_name, last_dot};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(import: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(import.kind, K::Import { is_static: true, .. }) {
        let name = imported_name(import, context.text);
        let message = format!("Using a static member import should be avoided - {name}");
        diagnostics.push(Diagnostic::new(Rule::StaticImport, last_dot(import, context), message));
    }
}
