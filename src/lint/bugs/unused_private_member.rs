//! B027: private field or method that is never used (Checkstyle UnusedPrivateField, PMD UnusedPrivateField,
//! UnusedPrivateMethod, Error Prone UnusedVariable, UnusedMethod).
//!
//! Not reported, as they are used without being named in the code: serialization members, annotated members
//! (frameworks access them by reflection), fields in files that use Lombok, and methods named in a string (JUnit's
//! `@MethodSource("name")`).
//!
//! Unsafe fix (Eclipse save action "Remove unused private members"): removes the member with its Javadoc and the
//! comments directly above it, and a blank line that would be left doubled; from a field declaration of several
//! variables only the declarator. Fields whose value may have side effects (other than creating an object or array
//! from side-effect-free values) are kept. Unsafe because uses are found by name only, so a use through reflection
//! or a member of the same name elsewhere is not told apart.

use std::cell::OnceCell;
use std::collections::HashSet;

use super::shared_b::{is_name_reference_in, is_side_effect_free, is_simple_literal};
use crate::lint::tree::{Context, for_each_descendant, is_type_child};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Modifier, Mods, Tree, VarDecl};

const SERIALIZATION_FIELDS: &[&str] = &["serialVersionUID", "serialPersistentFields"];
const SERIALIZATION_METHODS: &[&str] =
    &["writeObject", "readObject", "readObjectNoData", "writeReplace", "readResolve"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::CompilationUnit { imports, .. } = &tree.kind else { return };
    let text = context.text;
    // Only needed for files with private members.
    let names = OnceCell::new();
    let names = || names.get_or_init(|| UsedNames::find(tree, text));
    let uses_lombok = imports.iter().any(|i| context.source(i).contains("lombok."));
    let is_candidate = |mods: &Mods| mods.annotations.is_empty() && mods.has(Modifier::Private);
    for_each_descendant(tree, &mut |member, parent| {
        match &member.kind {
            K::Vars(vars) if !uses_lombok && is_field_declaration(parent) && is_candidate(&vars[0].mods) => {
                for (index, var) in vars.iter().enumerate() {
                    let field = var.name.as_str();
                    if !names().variables.contains(field) && !SERIALIZATION_FIELDS.contains(&field) {
                        let message = format!("Unused private field '{field}'");
                        let diagnostic = Diagnostic::new(Rule::UnusedPrivateMember, var.name_pos, message);
                        diagnostics.push(match field_removal(member, vars, index, context) {
                            Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
                            None => diagnostic,
                        });
                    }
                }
            }
            K::Method(method) if is_method_declaration(member, parent) && is_candidate(&method.mods) => {
                let name = method.name.as_str();
                if !names().methods.contains(name) && !SERIALIZATION_METHODS.contains(&name) {
                    let message = format!("Unused private method '{name}'");
                    let diagnostic = Diagnostic::new(Rule::UnusedPrivateMember, method.name_pos, message);
                    // An overload may be the method a test framework finds by name (JUnit's `@MethodSource`).
                    diagnostics.push(if names().overloaded.contains(name) {
                        diagnostic
                    } else {
                        diagnostic.with_unsafe_fix(vec![member_removal(member, context)])
                    });
                }
            }
            _ => {}
        }
        true
    });
}

/// Whether variables declared in `parent` are fields of a class body (not constants of an interface or annotation
/// type, nor local variables).
fn is_field_declaration(parent: &Tree) -> bool {
    match &parent.kind {
        K::Class(class) => !matches!(class.kind, ClassKind::Interface | ClassKind::Annotation),
        K::NewClass { .. } => true,
        _ => false,
    }
}

/// Whether the tree declares a method (not a constructor nor an element of an annotation type).
fn is_method_declaration(tree: &Tree, parent: &Tree) -> bool {
    let K::Method(method) = &tree.kind else { return false };
    method.ret.is_some() && !matches!(&parent.kind, K::Class(c) if c.kind == ClassKind::Annotation)
}

/// The edit that removes the declarator `index` of a field declaration, or the whole declaration if it is the only
/// one; `None` if its value may have side effects.
fn field_removal(field: &Tree, vars: &[VarDecl], index: usize, context: &Context) -> Option<Edit> {
    let var = &vars[index];
    if var.init.as_deref().is_some_and(|v| !is_removable_value(v, context.text)) {
        return None;
    }
    match (index.checked_sub(1).map(|i| &vars[i]), vars.get(index + 1)) {
        _ if context.tokens().has_comment(field.start..field.end) => None,
        (_, Some(next)) => Some(Edit::new(var.name_pos..next.name_pos, "")),
        (Some(previous), None) => Some(Edit::new(previous.end..var.end, "")),
        (None, None) => Some(member_removal(field, context)),
    }
}

/// A value whose evaluation can be dropped: a side-effect-free expression, a lambda or method reference, or the
/// creation of an object or array from such values.
fn is_removable_value(value: &Tree, text: &str) -> bool {
    let all = |trees: &[Tree]| trees.iter().all(|t| is_removable_value(t, text));
    match &value.kind {
        K::Lambda { .. } | K::MemberRef { .. } => true,
        K::MemberSelect { name, .. } if name == "class" => true,
        K::NewClass { args, body: None, .. } => all(args),
        // Like tree-sitter's array creation, whose type must be a simple, generic or integral type.
        K::NewArray { elem, dims, init, .. } => {
            let simple_type = |t: &Tree| match &t.kind {
                K::Ident(_) | K::Parameterized { .. } => true,
                K::Primitive(p) => matches!(p.as_str(), "int" | "long" | "short" | "byte" | "char"),
                _ => false,
            };
            elem.as_deref().is_none_or(simple_type) && all(dims) && init.as_deref().is_none_or(all)
        }
        K::Unary { op, expr, .. } => op != "++" && op != "--" && is_removable_value(expr, text),
        K::Binary { lhs, rhs, .. } => is_removable_value(lhs, text) && is_removable_value(rhs, text),
        // tree-sitter has a unary expression for a negative literal.
        K::Literal(kind) if text[value.start..].starts_with('-') => {
            is_simple_literal(*kind, text[value.start + 1..value.end].trim_start())
        }
        _ => is_side_effect_free(value, text),
    }
}

/// The edit that removes a member declaration with the comments directly above it (its Javadoc), and the blank line
/// after it if there is one before it too, or the blank line before it if it is the last member.
fn member_removal(member: &Tree, context: &Context) -> Edit {
    let text = context.text;
    let tokens = context.tokens();
    let previous_token_end = tokens.previous(member.start).map_or(0, |t| t.end);
    let comments: Vec<_> = tokens.comments_in(previous_token_end..member.start).collect();
    let mut start = member.start;
    for comment in comments.iter().rev() {
        let between = &text[comment.end..start];
        let line_start = text[..comment.start].rfind('\n').map_or(0, |i| i + 1);
        let starts_line = text[line_start..comment.start].trim().is_empty();
        if !starts_line || !between.trim().is_empty() || between.matches('\n').count() > 1 {
            break;
        }
        start = comment.start;
    }
    let end = member.end;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    if !text[line_start..start].trim().is_empty() || !text[end..line_end].trim().is_empty() {
        // Not alone on its lines.
        let trailing = text[end..].len() - text[end..].trim_start_matches([' ', '\t']).len();
        return Edit::new(start..end + trailing, "");
    }
    let blank_line_before = text[..line_start].strip_suffix('\n').is_some_and(|before| {
        let previous_line_start = before.rfind('\n').map_or(0, |i| i + 1);
        before[previous_line_start..].trim().is_empty()
    });
    let mut range = line_start..line_end;
    if blank_line_before {
        let next_line_end = text[line_end..].find('\n').map_or(text.len(), |i| line_end + i + 1);
        let next_line = &text[line_end..next_line_end];
        if next_line.trim().is_empty() && next_line_end > line_end {
            range.end = next_line_end;
        } else if next_line.trim_start().starts_with('}') {
            let before = &text[..line_start - 1];
            range.start = before.rfind('\n').map_or(0, |i| i + 1);
        }
    }
    Edit::new(range, "")
}

/// The names used in the file, by name only: a use of any field or method with the name counts.
struct UsedNames<'a> {
    variables: HashSet<&'a str>,
    methods: HashSet<&'a str>,
    /// The names of methods declared more than once.
    overloaded: HashSet<&'a str>,
}

impl<'a> UsedNames<'a> {
    fn find(unit: &'a Tree, text: &'a str) -> UsedNames<'a> {
        let mut names = UsedNames { variables: HashSet::new(), methods: HashSet::new(), overloaded: HashSet::new() };
        let mut declared = HashSet::new();
        names.visit_children(unit, None, false, text, &mut declared);
        names
    }

    /// Records the names in the children of `parent` (whose parent is `grandparent`), which are in a type if
    /// `in_type`.
    fn visit_children(
        &mut self,
        parent: &'a Tree,
        grandparent: Option<&'a Tree>,
        in_type: bool,
        text: &'a str,
        declared: &mut HashSet<&'a str>,
    ) {
        parent.for_each_child(&mut |tree| {
            let is_select = matches!(&parent.kind, K::MethodInvocation { select, .. } if std::ptr::eq(&**select, tree));
            let in_type = !matches!(tree.kind, K::Annotation { .. }) && (in_type || is_type_child(parent, tree));
            match &tree.kind {
                K::Ident(name) if !in_type && is_select => {
                    self.methods.insert(name);
                }
                K::Ident(name) if !in_type && is_name_reference_in(tree, parent, grandparent) => {
                    self.variables.insert(name);
                }
                K::MemberSelect { name, .. } if !in_type && is_select => {
                    self.methods.insert(name);
                }
                // A field access, whose field tree-sitter takes for a variable name.
                K::MemberSelect { name, .. } if !in_type && name != "class" => {
                    self.variables.insert(name);
                }
                K::MemberRef { name: Some(name), .. } if name != "new" => {
                    self.methods.insert(name);
                }
                K::Method(method) if is_method_declaration(tree, parent) && !declared.insert(&method.name) => {
                    self.overloaded.insert(&method.name);
                }
                K::Literal(LitKind::String) => {
                    self.methods.insert(text[tree.start..tree.end].trim_matches('"'));
                }
                _ => {}
            }
            self.visit_children(tree, Some(parent), in_type, text, declared);
        });
    }
}
