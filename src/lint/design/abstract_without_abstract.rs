//! A011: an abstract class without abstract methods, which neither extends a class nor implements an interface (PMD
//! AbstractClassWithoutAbstractMethod). A class that has supertypes may leave their abstract methods unimplemented.

use super::has_keyword;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class
        || class.extends.is_some()
        || !class.implements.is_empty()
        || !has_keyword(&class.mods, class.name_pos, "abstract", context)
    {
        return;
    }
    let has_abstract_method = class.members.iter().any(|m| {
        matches!(&m.kind, K::Method(method)
            if method.ret.is_some() && has_keyword(&method.mods, method.name_pos, "abstract", context))
    });
    if !has_abstract_method {
        let message = "This abstract class does not have any abstract methods";
        diagnostics.push(Diagnostic::new(Rule::AbstractWithoutAbstract, class.name_pos, message));
    }
}
