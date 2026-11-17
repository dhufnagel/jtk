//! I005: a fully qualified type such as `java.util.List` where the simple name would do (Checkstyle
//! UnnecessaryFullyQualifiedType, PMD UnnecessaryFullyQualifiedName).
//!
//! As in Checkstyle, a qualified type in a declaration, cast, `instanceof`, pattern, `new`, `extends`, `implements`,
//! `throws` or type argument is reported when no other type with the same simple name is used in the file: none
//! imported, declared, referenced by its simple name, or with another qualified name. A name counts as qualified by a
//! package if the identifier before the simple name starts with a lowercase letter, so `java.util.Map.Entry` is not
//! reported. Bounds of type parameters and wildcards, class literals, method references, annotations and `permits`
//! clauses are not checked, and their type names do not count as uses either.
//!
//! Checkstyle looks up the simple name in the packages imported on demand (`import java.util.*;`) on its own class
//! path, which only knows the JDK. jtk knows the types of the most common JDK packages (`jdk_types.txt`, from JDK 25);
//! deviating from Checkstyle, it assumes that other JDK packages (`java.*`, `javax.*`) may contain a type of that
//! name and then does not report it. Like Checkstyle, it assumes that other packages do not. Checkstyle also reports
//! the member type of a generic type with a lowercase name (`outer<T>.Inner`, as `outer.TYPE_ARGUMENTS`), which jtk
//! does not.
//!
//! The fix replaces the qualified name by the simple name if the file imports exactly that type or the type is in the
//! file's package, and the file declares no type parameter of that name. It is not safe if a supertype declares a
//! member type of that name. Types of `java.lang` are not fixed: a type of the same name in the file's package would
//! hide them.

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::{imported_name, is_on_demand, is_static};
use crate::lint::syntax::{for_each_descendant, is_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let file = FileTypes::collect(program, text);
    for reference in &file.qualified {
        if file.has_other_type(reference) {
            continue;
        }
        let message = format!("Unnecessary fully qualified type - {}", reference.name);
        let diagnostic = Diagnostic::new(Rule::FullyQualifiedType, reference.last_dot, message);
        diagnostics.push(match file.fix(reference) {
            Some(edit) => diagnostic.with_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// A type name qualified by a package, like `java.util.List`.
struct QualifiedReference<'t> {
    node: Node<'t>,
    /// The name without whitespace, type arguments and annotations.
    name: String,
    package: String,
    simple_name: String,
    last_dot: usize,
}

/// The type names a file declares, imports and uses.
#[derive(Default)]
struct FileTypes<'t> {
    package: String,
    /// The types imported by single-type imports, by simple name.
    imported: HashMap<String, HashSet<String>>,
    on_demand_packages: Vec<String>,
    declared: HashSet<String>,
    type_parameters: HashSet<String>,
    /// The types used by their simple names, in the places where qualified types are reported.
    simple: HashSet<String>,
    /// The types used qualified by a package, in the same places.
    qualified: Vec<QualifiedReference<'t>>,
    /// The qualified names of `qualified`, by simple name.
    qualified_names: HashMap<String, HashSet<String>>,
}

/// Where type names are neither reported nor count as uses of a simple name.
const UNCHECKED: &[&str] =
    &["type_bound", "wildcard", "class_literal", "method_reference", "annotation", "marker_annotation", "permits"];

impl<'t> FileTypes<'t> {
    fn collect(program: &Node<'t>, text: &str) -> FileTypes<'t> {
        let mut file = FileTypes::default();
        // Depth-first in document order, with whether the node is in an unchecked place and whether it is part of a
        // longer type name.
        let mut stack = vec![(*program, false, false)];
        while let Some((node, unchecked, in_name)) = stack.pop() {
            let kind = node.kind();
            match kind {
                "package_declaration" => {
                    file.package = named_children(&node)
                        .iter()
                        .find(|c| matches!(c.kind(), "identifier" | "scoped_identifier"))
                        .map_or(String::new(), |name| without_whitespace(source(name, text)));
                    continue;
                }
                "import_declaration" => {
                    file.add_import(&node, text);
                    continue;
                }
                "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration" => {
                    if let Some(name) = node.child_by_field_name("name") {
                        file.declared.insert(source(&name, text).to_string());
                    }
                }
                "type_parameter" => {
                    if let Some(name) = named_children(&node).into_iter().find(|c| c.kind() == "type_identifier") {
                        file.type_parameters.insert(source(&name, text).to_string());
                    }
                    // The name is a declaration, and bounds are unchecked.
                    continue;
                }
                "type_identifier" | "scoped_type_identifier" if !in_name && !unchecked => {
                    file.add_reference(node, text)
                }
                _ => {}
            }
            let unchecked = unchecked || UNCHECKED.contains(&kind);
            let children = named_children(&node);
            for child in children.into_iter().rev() {
                let child_in_name = match kind {
                    "scoped_type_identifier" => true,
                    "generic_type" => in_name && child.kind() != "type_arguments",
                    _ => false,
                };
                stack.push((child, unchecked, child_in_name));
            }
        }
        file
    }

    fn add_import(&mut self, import: &Node, text: &str) {
        if is_static(import) {
            return;
        }
        let name = imported_name(import, text);
        if is_on_demand(import) {
            self.on_demand_packages.push(name.trim_end_matches(".*").to_string());
        } else if let Some((_, simple_name)) = name.rsplit_once('.') {
            self.imported.entry(simple_name.to_string()).or_default().insert(name);
        }
    }

    /// Adds a whole type name, which is not part of a longer one.
    fn add_reference(&mut self, node: Node<'t>, text: &str) {
        let segments = segment_names(&node, text);
        let Some((simple_name, qualifier)) = segments.split_last() else { return };
        let Some(last_qualifier) = qualifier.last() else {
            self.simple.insert(simple_name.to_string());
            return;
        };
        // `Map.Entry` names a member type, as do `outer<T>.Inner` and, by convention, other names whose qualifier
        // does not start with a lowercase letter.
        let qualifier_node = named_children(&node).into_iter().find(|c| !c.kind().ends_with("annotation"));
        if !last_qualifier.starts_with(|c: char| c.is_lowercase())
            || qualifier_node.is_none_or(|q| q.kind() == "generic_type")
        {
            return;
        }
        let package = qualifier.join(".");
        let name = format!("{package}.{simple_name}");
        let last_dot = node.children(&mut node.walk()).filter(|c| c.kind() == ".").last();
        self.qualified_names.entry(simple_name.to_string()).or_default().insert(name.clone());
        self.qualified.push(QualifiedReference {
            node,
            name,
            package,
            simple_name: simple_name.to_string(),
            last_dot: last_dot.map_or(node.start_byte(), |d| d.start_byte()),
        });
    }

    /// Whether the file may use another type with the simple name of the reference, so that it has to stay
    /// qualified.
    fn has_other_type(&self, reference: &QualifiedReference) -> bool {
        let simple_name = reference.simple_name.as_str();
        let imported = self.imported.get(simple_name);
        let imports_reference = imported.is_some_and(|names| names.contains(&reference.name));
        // An imported type hides the types of the same name in packages imported on demand.
        let on_demand_other = imported.is_none()
            && self.on_demand_packages.iter().any(|p| *p != reference.package && on_demand_type(p, simple_name));
        // The simple name refers to the reference's type only if that is imported or implicitly visible.
        let simple_other = self.simple.contains(simple_name)
            && !(imports_reference || reference.package == self.package || reference.package == "java.lang");
        self.declared.contains(simple_name)
            || imported.is_some_and(|names| names.iter().any(|n| *n != reference.name))
            || on_demand_other
            || simple_other
            || self.qualified_names[simple_name].len() > 1
    }

    /// The edit that removes the qualifier, if the simple name refers to the same type.
    fn fix(&self, reference: &QualifiedReference) -> Option<Edit> {
        let visible = self.imported.get(&reference.simple_name).is_some_and(|names| names.contains(&reference.name))
            || reference.package == self.package;
        let simple_name = named_children(&reference.node).into_iter().rfind(|c| c.kind() == "type_identifier")?;
        let mut only_names = true;
        for_each_descendant(&reference.node, &[], |n| {
            only_names &= !is_comment(n) && !n.kind().ends_with("annotation");
        });
        let safe = visible
            && only_names
            && reference.package != "java.lang"
            && !self.type_parameters.contains(&reference.simple_name);
        safe.then(|| Edit::new(reference.node.start_byte()..simple_name.start_byte(), ""))
    }
}

/// Whether the package may contain a top-level type of that name: for the most common JDK packages, whether it
/// does in JDK 25; for other JDK packages, conservatively yes; for other packages no, as in Checkstyle.
fn on_demand_type(package: &str, simple_name: &str) -> bool {
    const JDK_TYPES: &str = include_str!("jdk_types.txt");
    let listed = JDK_TYPES.lines().filter(|l| !l.starts_with('#')).find_map(|line| {
        let (listed_package, types) = line.split_once(' ')?;
        (listed_package == package).then_some(types)
    });
    match listed {
        Some(types) => types.split(' ').any(|t| t == simple_name),
        None => package.starts_with("java.") || package.starts_with("javax."),
    }
}

/// The identifiers of a part of a qualified type name.
fn segment_names<'a>(node: &Node, text: &'a str) -> Vec<&'a str> {
    match node.kind() {
        "type_identifier" => vec![source(node, text)],
        "scoped_type_identifier" | "generic_type" => {
            named_children(node).iter().flat_map(|c| segment_names(c, text)).collect()
        }
        _ => Vec::new(),
    }
}

fn without_whitespace(name: &str) -> String {
    name.split_whitespace().collect()
}
