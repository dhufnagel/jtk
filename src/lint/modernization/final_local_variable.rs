//! M015: local variable declared with a value, never assigned again, but not `final` (Checkstyle FinalLocalVariable,
//! PMD LocalVariableCouldBeFinal, in part). Variables declared without a value, `for` loop variables and unnamed
//! variables (`_`) are not checked.
//!
//! An assignment of the name later in the enclosing block counts, also in lambdas, but not in nested classes: code
//! there cannot assign a local variable of the enclosing code (it can only capture effectively final ones), so the
//! name means a field or a variable of its own, perhaps an inherited field.
//!
//! The fix adds `final` to variables alone in their declaration.
use std::collections::HashMap;

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, has_modifier, named_children, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Where each name is assigned, with the start of the innermost class body around the assignment.
    let mut assignments: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    let mut declarations = Vec::new();
    for_each_descendant(program, &[], |node| {
        let target = match node.kind() {
            "assignment_expression" => node.child_by_field_name("left"),
            "update_expression" => named_children(node).into_iter().next(),
            "local_variable_declaration" => {
                declarations.push(*node);
                None
            }
            _ => None,
        };
        if let Some(target) = target.map(|t| strip_parentheses(&t))
            && target.kind() == "identifier"
        {
            let class_body = std::iter::successors(target.parent(), |n| n.parent()).find(|n| n.kind() == "class_body");
            let class_start = class_body.map_or(0, |b| b.start_byte());
            assignments.entry(source(&target, text)).or_default().push((target.start_byte(), class_start));
        }
    });
    for declaration in declarations {
        let Some(parent) = declaration.parent() else { continue };
        if parent.kind() == "for_statement" || has_modifier(&declaration, text, "final") {
            continue;
        }
        // A variable declared in a `case` group is in scope up to the end of the switch.
        let scope = match parent.kind() {
            "switch_block_statement_group" => parent.parent().unwrap_or(parent),
            _ => parent,
        };
        let declarators: Vec<Node> =
            named_children(&declaration).into_iter().filter(|c| c.kind() == "variable_declarator").collect();
        for declarator in &declarators {
            let Some(name) = declarator.child_by_field_name("name") else { continue };
            let name_text = source(&name, text);
            if declarator.child_by_field_name("value").is_none() || name_text == "_" {
                continue;
            }
            let in_scope = declarator.end_byte()..scope.end_byte();
            // Assignments in a class body that starts in the scope are in a nested class.
            let reassigned = assignments.get(name_text).is_some_and(|at| {
                at.iter().any(|(p, class_start)| in_scope.contains(p) && !in_scope.contains(class_start))
            });
            if reassigned {
                continue;
            }
            let message = format!("Variable '{name_text}' should be declared final");
            let diagnostic = Diagnostic::new(Rule::FinalLocalVariable, name.start_byte(), message);
            let declared_type = declaration.child_by_field_name("type");
            diagnostics.push(match declared_type {
                Some(t) if declarators.len() == 1 => {
                    diagnostic.with_fix(vec![Edit::new(t.start_byte()..t.start_byte(), "final ")])
                }
                _ => diagnostic,
            });
        }
    }
}
