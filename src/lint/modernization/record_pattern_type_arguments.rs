//! M005: type arguments of a record pattern (Checkstyle UnnecessaryTypeArgumentsWithRecordPattern), Java 21:
//! `case Box<String>(var s)` can be `case Box(var s)`, the compiler infers them. As in Checkstyle, only wildcards
//! (`Box<?>`) are accepted.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(pattern: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(arguments) = named_children(pattern)
        .into_iter()
        .find(|c| c.kind() == "generic_type")
        .and_then(|t| named_children(&t).into_iter().find(|c| c.kind() == "type_arguments"))
    else {
        return;
    };
    if named_children(&arguments).iter().any(|a| !is_comment(a) && a.kind() != "wildcard") {
        let message = "Unnecessary type arguments with record pattern";
        let fix = Edit::new(arguments.byte_range(), "");
        diagnostics.push(
            Diagnostic::new(Rule::RecordPatternTypeArguments, arguments.start_byte(), message).with_fix(vec![fix]),
        );
    }
}
