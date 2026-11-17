//! B024: a `clone()` or `finalize()` that does not call `super.clone()` or `super.finalize()` (Checkstyle SuperClone
//! and SuperFinalize, PMD ProperCloneImplementation and FinalizeDoesNotCallSuperFinalize).
//!
//! As in Checkstyle: only the overloads without parameters count, not native ones nor those of interfaces; an
//! abstract method is reported too (it has no call); the call must be unqualified (`super.clone()` without
//! arguments, or `super::clone`) and belong to the method itself, which a lambda does but a constructor, initializer
//! or other method (also of a nested class) does not.

use tree_sitter::Node;

use super::avoided_methods::is_parameterless;
use crate::lint::syntax::{has_modifier, is_comment, source};
use crate::lint::{Diagnostic, Rule};

/// Declarations whose code does not belong to the enclosing method. Instance initializers are `block`s in a class
/// body, handled separately.
const OTHER_CODE: &[&str] = &["method_declaration", "constructor_declaration"];

pub fn check(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = ["clone", "finalize"].into_iter().find(|n| is_parameterless(method, text, n)) else { return };
    if has_modifier(method, text, "native")
        || method.parent().is_some_and(|p| matches!(p.kind(), "interface_body" | "annotation_type_body"))
        || method.child_by_field_name("body").is_some_and(|body| calls_super(&body, text, name))
    {
        return;
    }
    let name_node = method.child_by_field_name("name").unwrap_or(*method);
    diagnostics.push(Diagnostic::new(
        Rule::MissingSuperCall,
        name_node.start_byte(),
        format!("Method '{name}' should call 'super.{name}'"),
    ));
}

/// Whether the method body calls the super method of the name, outside nested methods, constructors and
/// initializers.
fn calls_super(body: &Node, text: &str, name: &str) -> bool {
    let mut stack = vec![*body];
    while let Some(node) = stack.pop() {
        for child in node.named_children(&mut node.walk()) {
            let initializer = child.kind() == "block" && node.kind() == "class_body";
            if OTHER_CODE.contains(&child.kind()) || initializer {
                continue;
            }
            if is_super_call(&child, text, name) {
                return true;
            }
            stack.push(child);
        }
    }
    false
}

/// `super.name()` without arguments or `super::name`.
fn is_super_call(node: &Node, text: &str, name: &str) -> bool {
    match node.kind() {
        "method_invocation" => {
            node.child_by_field_name("object").is_some_and(|o| o.kind() == "super")
                && node.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
                && node
                    .child_by_field_name("arguments")
                    .is_some_and(|a| !a.named_children(&mut a.walk()).any(|c| !is_comment(&c)))
        }
        "method_reference" => {
            let parts: Vec<Node> = node.named_children(&mut node.walk()).filter(|c| !is_comment(c)).collect();
            matches!(parts.as_slice(), [receiver, .., method] if receiver.kind() == "super" && source(method, text) == name)
        }
        _ => false,
    }
}
