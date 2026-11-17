//! I001: an on-demand import such as `import java.util.*;` or `import static java.lang.Math.*;` (Checkstyle
//! AvoidStarImport with its default properties): it hides where names come from, and a new class in the imported
//! package can break the code.

use tree_sitter::Node;

use super::{imported_name, is_on_demand, last_dot};
use crate::lint::{Diagnostic, Rule};

pub fn check(import: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if is_on_demand(import) {
        let message = format!("Using the '.*' form of import should be avoided - {}", imported_name(import, text));
        diagnostics.push(Diagnostic::new(Rule::StarImport, last_dot(import), message));
    }
}
