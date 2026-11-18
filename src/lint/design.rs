//! Rules of group A: design and structure of types and files.

mod constant_interface;
mod declaration_usage_distance;
mod final_class;
mod magic_number;
mod mutable_exception;
mod one_top_level_class;
mod outer_type_filename;
mod overload_order;
mod package_directory;
mod utility_class_constructor;
mod visibility_modifier;

mod abstract_without_abstract;
mod throws_exception;

use crate::ast::{ClassDecl, K, Mods, Tag, Tree};

use super::Rule;
use super::tree::{Context, has_modifier};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::UtilityClassConstructor, &[Tag::Class], utility_class_constructor::check),
    (Rule::FinalClass, &[Tag::Class], final_class::check),
    (Rule::VisibilityModifier, &[Tag::Vars], visibility_modifier::check),
    (Rule::MutableException, &[Tag::Class], mutable_exception::check),
    (Rule::ConstantInterface, &[Tag::Class], constant_interface::check),
    (Rule::OneTopLevelClass, &[Tag::CompilationUnit], one_top_level_class::check),
    (Rule::PackageDirectory, &[Tag::CompilationUnit], package_directory::check),
    (Rule::OverloadOrder, &[Tag::Class, Tag::NewClass], overload_order::check),
    (Rule::ThrowsException, &[Tag::Method], throws_exception::check),
    (Rule::AbstractWithoutAbstract, &[Tag::Class], abstract_without_abstract::check),
    (Rule::DeclarationUsageDistance, &[Tag::Block, Tag::Switch, Tag::SwitchExpr], declaration_usage_distance::check),
    (Rule::MagicNumber, &[Tag::Literal], magic_number::check),
    (Rule::OuterTypeFilename, &[Tag::CompilationUnit], outer_type_filename::check),
];

/// Whether a declaration whose modifiers end before `end` (its name) has the modifier keyword. Lexes only if the
/// keyword is in the text.
fn has_keyword(mods: &Mods, end: usize, keyword: &str, context: &Context) -> bool {
    context.text[mods.start..end].contains(keyword) && has_modifier(mods, context.text, context.tokens(), keyword)
}

/// The qualified name of a type or package (or annotation) without type arguments and annotations: `a.b.C<D>` is
/// `a.b.C`.
fn qualified_name(tree: &Tree) -> String {
    let mut name = String::new();
    append_name(tree, &mut name);
    name
}

fn append_name(tree: &Tree, name: &mut String) {
    let segment = match &tree.kind {
        K::Ident(segment) | K::Primitive(segment) => segment,
        K::MemberSelect { expr, name: segment } => {
            append_name(expr, name);
            segment
        }
        K::Parameterized { base, .. } | K::Annotated { base, .. } => return append_name(base, name),
        K::ArrayType { elem, .. } => return append_name(elem, name),
        _ => return,
    };
    if !name.is_empty() {
        name.push('.');
    }
    name.push_str(segment);
}

/// The last segment of a qualified name.
fn simple_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The package a compilation unit declares, or `""`.
fn package_name(unit: &Tree) -> String {
    match &unit.kind {
        K::CompilationUnit { package: Some((_, name)), .. } => qualified_name(name),
        _ => String::new(),
    }
}

/// The imports of the compilation unit.
fn imports(unit: &Tree) -> &[Tree] {
    match &unit.kind {
        K::CompilationUnit { imports, .. } => imports,
        _ => &[],
    }
}

/// The qualified names of the single-type imports (not `a.*`), static ones included.
fn single_type_imports(unit: &Tree) -> impl Iterator<Item = String> + '_ {
    imports(unit).iter().filter_map(|import| match &import.kind {
        K::Import { name, .. } if !matches!(&name.kind, K::MemberSelect { name, .. } if name == "*") => {
            Some(qualified_name(name))
        }
        _ => None,
    })
}

/// The top-level type declarations of a compilation unit.
fn top_level_types(unit: &Tree) -> impl Iterator<Item = (&Tree, &ClassDecl)> {
    let types = match &unit.kind {
        K::CompilationUnit { types, .. } => types.as_slice(),
        _ => &[],
    };
    types.iter().filter_map(|t| match &t.kind {
        K::Class(class) => Some((t, &**class)),
        _ => None,
    })
}
