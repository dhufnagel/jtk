//! T060: a static field annotated with Mockito's `@Mock` (errorprone StaticMockMember): the mock is shared by all
//! tests of the class (and its state with it), while Mockito's extension or runner creates it anew for each test.
//!
//! Fix: remove `static`. It is offered only where it keeps the code compiling: the field is not `final` and no static
//! member of the class (a static method, initializer, field or nested type) mentions its name. `@Mock` is recognised
//! by the imports or the fully qualified name.

use super::shared::{Names, is_static_member, keyword, keyword_removal, mentions};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return };
    let Some(first) = vars.first() else { return };
    let Some(range) = keyword(&first.mods, Modifier::Static) else { return };
    let Some(names) = Names::new(context) else { return };
    if !names.has_annotation(&first.mods, &["org.mockito.Mock"]) {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::StaticMock, tree.start, "@Mock field is static");
    let used_statically = class
        .members
        .iter()
        .filter(|m| is_static_member(m) && !std::ptr::eq(*m, tree))
        .any(|m| vars.iter().any(|v| mentions(m, &v.name)));
    diagnostics.push(if first.mods.has(Modifier::Final) || used_statically {
        diagnostic
    } else {
        diagnostic.with_fix(vec![keyword_removal(range, context.text)])
    });
}
