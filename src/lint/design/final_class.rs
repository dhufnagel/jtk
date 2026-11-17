//! A002: a class that cannot be extended, as it only has private constructors, but is not declared `final`
//! (Checkstyle FinalClass, PMD ClassWithOnlyPrivateConstructorsShouldBeFinal).
//!
//! As in Checkstyle, a class without constructors counts if it is private, and a class is not reported if a class or
//! anonymous class in the same file extends it. Like Checkstyle, the extended class is found by name: among the
//! classes whose qualified name ends with it, the one sharing the longest prefix with the type where the name
//! appears (the class's enclosing type for `extends`).

use tree_sitter::Node;

use super::{TYPE_DECLARATIONS, has_keyword, members, package_name, qualified_name, root, walk};
use crate::lint::syntax::{named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(class: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if has_keyword(class, text, "final") || has_keyword(class, text, "abstract") {
        return;
    }
    let Some(body) = class.child_by_field_name("body") else { return };
    let constructors: Vec<Node> =
        members(&body).into_iter().filter(|m| m.kind() == "constructor_declaration").collect();
    let only_private = if constructors.is_empty() {
        has_keyword(class, text, "private")
    } else {
        constructors.iter().all(|c| has_keyword(c, text, "private"))
    };
    if !only_private || is_extended_in_file(class, text) {
        return;
    }
    let name = class.child_by_field_name("name").map_or("", |n| source(&n, text));
    let message = format!("Class {name} should be declared as final");
    diagnostics.push(Diagnostic::new(Rule::FinalClass, class.start_byte(), message));
}

/// A name of a class that another class extends, and the qualified name of the type the name appears in.
struct Extension {
    name: Vec<String>,
    scope: Vec<String>,
}

/// Whether a class or anonymous class in the file extends the class. Without types, the extended class is found by
/// its name: the class whose qualified name ends with it and that is the closest relative of the type where the
/// name appears (the longest common prefix of qualified names), or the least nested and then the first one among equally close ones.
fn is_extended_in_file(class: &Node, text: &str) -> bool {
    let program = root(class);
    let package = package_name(&program, text);
    // The qualified name of the type being walked, as segments.
    let mut scope: Vec<String> = package.split('.').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let mut declarations: Vec<(Vec<String>, usize)> = Vec::new();
    let mut extensions: Vec<Extension> = Vec::new();
    walk(&program, |node, entering| {
        let is_type = TYPE_DECLARATIONS.contains(&node.kind());
        if !entering {
            if is_type {
                scope.pop();
            }
            return;
        }
        if node.kind() == "object_creation_expression" && named_children(node).iter().any(|c| c.kind() == "class_body")
        {
            extensions.push(Extension { name: created_type(node, text), scope: scope.clone() });
        }
        if !is_type {
            return;
        }
        if let Some(superclass) = node.child_by_field_name("superclass").and_then(|s| s.named_child(0)) {
            extensions
                .push(Extension { name: segments(&qualified_name_of_type(&superclass, text)), scope: scope.clone() });
        }
        let name = node.child_by_field_name("name").map_or(String::new(), |n| source(&n, text).to_string());
        scope.push(name);
        if node.kind() == "class_declaration" {
            declarations.push((scope.clone(), node.id()));
        }
    });
    extensions.iter().any(|extension| resolve(extension, &declarations) == Some(class.id()))
}

/// The class an extended name refers to, by the id of its declaration.
fn resolve(extension: &Extension, declarations: &[(Vec<String>, usize)]) -> Option<usize> {
    let common_prefix = |name: &[String]| name.iter().zip(&extension.scope).take_while(|(a, b)| a == b).count();
    declarations
        .iter()
        .filter(|(name, _)| name.ends_with(&extension.name))
        .min_by_key(|(name, _)| (std::cmp::Reverse(common_prefix(name)), name.len()))
        .map(|(_, id)| *id)
}

/// The name of the class an anonymous class extends: for `outer.new Inner() {}`, the class name of `outer` if it is
/// created too, followed by `Inner`.
fn created_type(creation: &Node, text: &str) -> Vec<String> {
    let mut name = match creation.named_child(0).filter(|c| c.kind() == "object_creation_expression") {
        Some(outer) => created_type(&outer, text),
        None => Vec::new(),
    };
    if let Some(ty) = creation.child_by_field_name("type") {
        name.extend(segments(&qualified_name_of_type(&ty, text)));
    }
    name
}

fn segments(name: &str) -> Vec<String> {
    name.split('.').map(str::to_string).collect()
}

/// The qualified name of a type without its type arguments: `a.b.C<D>` is `a.b.C`.
fn qualified_name_of_type(ty: &Node, text: &str) -> String {
    match ty.kind() {
        "generic_type" => named_children(ty).first().map_or(String::new(), |t| qualified_name(t, text)),
        _ => qualified_name(ty, text),
    }
}
