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

use super::{import_name, imported_name};
use crate::ast::{K, Tree};
use crate::lint::tree::{Context, is_type_child};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let mut file = FileTypes::default();
    file.collect(unit, context.text, false);
    for reference in &file.qualified {
        if file.has_other_type(reference) {
            continue;
        }
        let K::MemberSelect { expr, .. } = &reference.tree.kind else { continue };
        let last_dot = context.tokens().next(expr.end).map_or(reference.tree.start, |dot| dot.start);
        let message = format!("Unnecessary fully qualified type - {}", reference.name);
        let diagnostic = Diagnostic::new(Rule::FullyQualifiedType, last_dot, message);
        diagnostics.push(match file.fix(reference, context) {
            Some(edit) => diagnostic.with_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// A type name qualified by a package, like `java.util.List`.
struct QualifiedReference<'t> {
    /// The `MemberSelect` selecting the simple name, without type arguments and leading annotations.
    tree: &'t Tree,
    /// The name without whitespace, type arguments and annotations.
    name: String,
    package: String,
    simple_name: &'t str,
}

/// The type names a file declares, imports and uses.
#[derive(Default)]
struct FileTypes<'t> {
    package: String,
    /// The types imported by single-type imports, by simple name.
    imported: HashMap<String, HashSet<String>>,
    on_demand_packages: Vec<String>,
    declared: HashSet<&'t str>,
    type_parameters: HashSet<&'t str>,
    /// The types used by their simple names, in the places where qualified types are reported.
    simple: HashSet<&'t str>,
    /// The types used qualified by a package, in the same places.
    qualified: Vec<QualifiedReference<'t>>,
    /// The qualified names of `qualified`, by simple name.
    qualified_names: HashMap<&'t str, HashSet<String>>,
}

impl<'t> FileTypes<'t> {
    /// Collects the names in `tree`, which is not a type; `unchecked` if it is in a place where type names are
    /// neither reported nor count as uses.
    fn collect(&mut self, tree: &'t Tree, text: &str, unchecked: bool) {
        let mut unchecked = unchecked;
        let mut permits: &[Tree] = &[];
        match &tree.kind {
            K::CompilationUnit { package, imports, types, .. } => {
                if let Some((_, name)) = package {
                    self.package = without_whitespace(&text[name.start..name.end]);
                }
                imports.iter().for_each(|import| self.add_import(import, text));
                types.iter().for_each(|t| self.collect(t, text, unchecked));
                return;
            }
            K::Class(class) => {
                self.declared.insert(&class.name);
                permits = &class.permits;
            }
            // The name is a declaration, and bounds are unchecked.
            K::TypeParameter { name, .. } => {
                self.type_parameters.insert(name);
                return;
            }
            K::Annotation { .. } | K::MemberRef { .. } => unchecked = true,
            K::MemberSelect { name, .. } if name == "class" => unchecked = true,
            _ => {}
        }
        // The variables of `String a, b[];` each have a copy of the type, which is written once.
        let mut previous_type_start = None;
        tree.for_each_child(&mut |child| {
            // Type parameters and patterns are declarations, which contain types.
            let is_declaration = matches!(
                child.kind,
                K::TypeParameter { .. } | K::BindingPattern { .. } | K::RecordPattern { .. } | K::AnyPattern
            );
            if is_type_child(tree, child) && !is_declaration {
                let is_copy = previous_type_start.replace(child.start) == Some(child.start);
                if !unchecked && !is_copy && !permits.iter().any(|p| std::ptr::eq(p, child)) {
                    self.add_type(child);
                }
            } else {
                self.collect(child, text, unchecked);
            }
        });
    }

    /// Adds the type names of a type.
    fn add_type(&mut self, ty: &'t Tree) {
        match &ty.kind {
            K::Ident(_) | K::MemberSelect { .. } | K::Parameterized { .. } => self.add_reference(ty),
            K::Annotated { base, .. } => self.add_type(base),
            K::ArrayType { elem, .. } => self.add_type(elem),
            K::Union(types) | K::Intersection(types) => types.iter().for_each(|t| self.add_type(t)),
            _ => {}
        }
    }

    fn add_import(&mut self, import: &Tree, text: &str) {
        let Some((_, on_demand)) = import_name(import) else { return };
        if matches!(import.kind, K::Import { is_static: true, .. }) {
            return;
        }
        let name = imported_name(import, text);
        if on_demand {
            self.on_demand_packages.push(name.trim_end_matches(".*").to_string());
        } else if let Some((_, simple_name)) = name.rsplit_once('.') {
            self.imported.entry(simple_name.to_string()).or_default().insert(name);
        }
    }

    /// Adds a whole class type name with its type arguments, which is not part of a longer one.
    fn add_reference(&mut self, ty: &'t Tree) {
        self.add_type_arguments(ty);
        let name = without_annotations_and_arguments(ty);
        let (simple_name, qualifier) = match &name.kind {
            K::Ident(simple_name) => {
                self.simple.insert(simple_name);
                return;
            }
            K::MemberSelect { expr, name } => (name.as_str(), expr),
            _ => return,
        };
        let mut segments = Vec::new();
        segment_names(qualifier, &mut segments);
        // `Map.Entry` names a member type, as do `outer<T>.Inner` and, by convention, other names whose qualifier
        // does not start with a lowercase letter.
        let is_package = segments.last().is_some_and(|last| last.starts_with(|c: char| c.is_lowercase()));
        if !is_package || matches!(without_annotations(qualifier).kind, K::Parameterized { .. }) {
            return;
        }
        let package = segments.join(".");
        let qualified_name = format!("{package}.{simple_name}");
        self.qualified_names.entry(simple_name).or_default().insert(qualified_name.clone());
        self.qualified.push(QualifiedReference { tree: name, name: qualified_name, package, simple_name });
    }

    /// Adds the type arguments anywhere in a class type name, such as the `T` of `Outer<T>.Inner`.
    fn add_type_arguments(&mut self, ty: &'t Tree) {
        match &ty.kind {
            K::Parameterized { base, args } => {
                self.add_type_arguments(base);
                args.iter().for_each(|arg| self.add_type(arg));
            }
            K::MemberSelect { expr: inner, .. } | K::Annotated { base: inner, .. } => self.add_type_arguments(inner),
            _ => {}
        }
    }

    /// Whether the file may use another type with the simple name of the reference, so that it has to stay
    /// qualified.
    fn has_other_type(&self, reference: &QualifiedReference) -> bool {
        let simple_name = reference.simple_name;
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
    fn fix(&self, reference: &QualifiedReference, context: &Context) -> Option<Edit> {
        let visible = self.imported.get(reference.simple_name).is_some_and(|names| names.contains(&reference.name))
            || reference.package == self.package;
        let range = reference.tree.start..reference.tree.end;
        let tokens = context.tokens();
        let simple_name = tokens.previous(range.end)?;
        let only_names =
            !tokens.has_comment(range.clone()) && tokens.in_range(range.clone()).all(|t| &context.text[t] != "@");
        let safe = visible
            && only_names
            && reference.package != "java.lang"
            && !self.type_parameters.contains(reference.simple_name);
        safe.then(|| Edit::new(range.start..simple_name.start, ""))
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

/// The type name without the type arguments of its last part and the annotations around it.
fn without_annotations_and_arguments(mut ty: &Tree) -> &Tree {
    loop {
        match &ty.kind {
            K::Parameterized { base, .. } | K::Annotated { base, .. } => ty = base,
            _ => return ty,
        }
    }
}

fn without_annotations(mut ty: &Tree) -> &Tree {
    while let K::Annotated { base, .. } = &ty.kind {
        ty = base;
    }
    ty
}

/// Appends the identifiers of a part of a class type name.
fn segment_names<'t>(ty: &'t Tree, segments: &mut Vec<&'t str>) {
    match &ty.kind {
        K::Ident(name) => segments.push(name),
        K::MemberSelect { expr, name } => {
            segment_names(expr, segments);
            segments.push(name);
        }
        K::Parameterized { base, .. } | K::Annotated { base, .. } => segment_names(base, segments),
        _ => {}
    }
}

fn without_whitespace(name: &str) -> String {
    name.split_whitespace().collect()
}
