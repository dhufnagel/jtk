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

use crate::ast::{ClassKind, K, Tree};
use crate::lint::tree::{Context, has_modifier};
use crate::lint::{Diagnostic, Edit, Rule};

use super::shared_a::assigned_name;

/// What the walk collects.
#[derive(Default)]
struct Collected<'t> {
    /// Where each name is assigned, with the start of the innermost class body around the assignment (0 if none).
    assignments: HashMap<&'t str, Vec<(usize, usize)>>,
    /// The local variable declarations, with the tree they are visible in.
    declarations: Vec<(&'t Tree, &'t Tree)>,
}

pub fn check<'t>(unit: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let text = context.text;
    let mut collected = Collected::default();
    collect(unit, 0, &mut collected);
    for (declaration, scope) in collected.declarations {
        let K::Vars(vars) = &declaration.kind else { continue };
        if has_modifier(&vars[0].mods, text, context.tokens(), "final") {
            continue;
        }
        for var in vars {
            if var.init.is_none() || var.name == "_" {
                continue;
            }
            let in_scope = var.end..scope.end;
            // Assignments in a class body that starts in the scope are in a nested class.
            let reassigned = collected.assignments.get(var.name.as_str()).is_some_and(|at| {
                at.iter().any(|(p, class_start)| in_scope.contains(p) && !in_scope.contains(class_start))
            });
            if reassigned {
                continue;
            }
            let message = format!("Variable '{}' should be declared final", var.name);
            let diagnostic = Diagnostic::new(Rule::FinalLocalVariable, var.name_pos, message);
            // `var` is the token before the name.
            let type_start = match var.ty.as_deref() {
                Some(ty) => Some(ty.start),
                None => context.tokens().previous(var.name_pos).map(|t| t.start),
            };
            diagnostics.push(match type_start {
                Some(start) if vars.len() == 1 => diagnostic.with_fix(vec![Edit::new(start..start, "final ")]),
                _ => diagnostic,
            });
        }
    }
}

/// Collects the assignments and local variable declarations below `tree`, which is in the class body starting at
/// `class_start`.
fn collect<'t>(tree: &'t Tree, class_start: usize, collected: &mut Collected<'t>) {
    // Class bodies as tree-sitter's `class_body`: of classes, records, anonymous classes and enum constants, each
    // represented by a position in it.
    let constant_bodies: Vec<(usize, &[Tree])> = match &tree.kind {
        K::Class(class) => class.enum_constants.iter().filter_map(|c| Some((c.name_pos, c.body.as_deref()?))).collect(),
        _ => Vec::new(),
    };
    let body_start = |child: &Tree| match &tree.kind {
        K::Class(class) if matches!(class.kind, ClassKind::Class | ClassKind::Record) => tree.start,
        K::Class(_) => constant_bodies
            .iter()
            .find(|(_, body)| body.iter().any(|m| std::ptr::eq(m, child)))
            .map_or(class_start, |(start, _)| *start),
        K::NewClass { body: Some(body), .. } if body.first().is_some_and(|m| m.start <= child.start) => tree.start,
        _ => class_start,
    };
    tree.for_each_child(&mut |child| {
        let class_start = body_start(child);
        if let Some((name, target)) = assigned_name(child, tree) {
            collected.assignments.entry(name).or_default().push((target.start, class_start));
        }
        // A variable declared in a `case` group is in scope up to the end of the switch.
        if matches!(child.kind, K::Vars(_))
            && matches!(tree.kind, K::Block { .. } | K::Switch { .. } | K::SwitchExpr { .. })
        {
            collected.declarations.push((child, tree));
        }
        collect(child, class_start, collected);
    });
}
