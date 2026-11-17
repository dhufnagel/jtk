//! I003: a static import such as `import static java.lang.Math.max;` (Checkstyle AvoidStaticImport with its default
//! properties): the member reads as if it were declared in the class.

use tree_sitter::Node;

use super::{imported_name, is_static, last_dot};
use crate::lint::{Diagnostic, Rule};

pub fn check(import: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if is_static(import) {
        let message = format!("Using a static member import should be avoided - {}", imported_name(import, text));
        diagnostics.push(Diagnostic::new(Rule::StaticImport, last_dot(import), message));
    }
}
