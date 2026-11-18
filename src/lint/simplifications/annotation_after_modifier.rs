//! S009: annotations after modifier keywords (the annotation part of Checkstyle ModifierOrder). The order of the
//! keywords themselves is left to the formatter.
//!
//! Like Checkstyle, the check stops at the first keyword out of the JLS order, and allows annotations after the
//! keywords where they may annotate the type: on fields, variables, parameters, constructors and non-void methods.
//! The fix moves the annotations before the first keyword (annotations in the modifier list annotate the same thing
//! wherever they are).

use std::ops::Range;

use crate::ast::{ClassKind, K, Mods, Tree};
use crate::lint::tree::{Context, modifier_keywords};
use crate::lint::{Diagnostic, Edit, Rule};

use super::removal_range;

const JLS_ORDER: &[&str] = &[
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &tree.kind {
        K::Class(class) => check_modifiers(&class.mods, context, diagnostics),
        K::Method(method) => {
            // Annotations after the keywords may annotate the type of a non-void method or of an annotation's
            // element, or a constructor's.
            let in_annotation =
                context.parent().is_some_and(|p| matches!(&p.kind, K::Class(c) if c.kind == ClassKind::Annotation));
            let may_annotate_type = match &method.ret {
                Some(ret) => !in_annotation && !matches!(&ret.kind, K::Primitive(p) if p == "void"),
                None => !method.compact_ctor,
            };
            if !may_annotate_type {
                check_modifiers(&method.mods, context, diagnostics);
            }
        }
        K::Try { resources, .. } => {
            for resource in resources {
                if let K::Var(var) = &resource.kind {
                    check_modifiers(&var.mods, context, diagnostics);
                }
            }
        }
        _ => {}
    }
}

/// A modifier: an annotation, or a keyword with its rank in the JLS order (`None` if it has none).
enum Modifier<'t> {
    Annotation(&'t Tree),
    Keyword(Range<usize>, Option<usize>),
}

impl Modifier<'_> {
    fn start(&self) -> usize {
        match self {
            Modifier::Annotation(a) => a.start,
            Modifier::Keyword(range, _) => range.start,
        }
    }
}

fn check_modifiers(mods: &Mods, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if mods.annotations.is_empty() {
        return;
    }
    let keywords = modifier_keywords(mods, context.text, context.tokens());
    let mut items: Vec<Modifier> = mods.annotations.iter().map(Modifier::Annotation).collect();
    items.extend(keywords.into_iter().map(|(k, r)| Modifier::Keyword(r, JLS_ORDER.iter().position(|o| *o == k))));
    items.sort_by_key(Modifier::start);
    let mut first_keyword = None;
    let mut last_rank = 0;
    for (index, item) in items.iter().enumerate() {
        match item {
            Modifier::Annotation(annotation) => {
                if let Some(first_keyword) = first_keyword {
                    report(&items, first_keyword, annotation, context, diagnostics);
                    return;
                }
            }
            Modifier::Keyword(_, Some(rank)) => {
                // Annotations after misordered keywords are left until the formatter has sorted the keywords.
                if *rank < last_rank {
                    return;
                }
                last_rank = *rank;
                first_keyword.get_or_insert(index);
            }
            Modifier::Keyword(_, None) => {}
        }
    }
}

fn report(
    items: &[Modifier],
    first_keyword: usize,
    annotation: &Tree,
    context: &Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let K::Annotation { ty, .. } = &annotation.kind else { return };
    let message = format!("'@{}' annotation should precede the modifier keywords", context.source(ty));
    let mut diagnostic = Diagnostic::new(Rule::AnnotationAfterModifier, annotation.start, message);
    let end = match &items[items.len() - 1] {
        Modifier::Annotation(a) => a.end,
        Modifier::Keyword(range, _) => range.end,
    };
    if !context.tokens().has_comment(items[0].start()..end) {
        let misplaced: Vec<&Tree> = items[first_keyword..]
            .iter()
            .filter_map(|m| match m {
                Modifier::Annotation(a) => Some(*a),
                Modifier::Keyword(..) => None,
            })
            .collect();
        let moved: String = misplaced.iter().map(|a| format!("{} ", context.source(a))).collect();
        let insertion = items[first_keyword].start();
        let mut edits = vec![Edit::new(insertion..insertion, moved)];
        edits.extend(misplaced.iter().map(|a| Edit::new(removal_range(a.start..a.end, context.text), "")));
        diagnostic = diagnostic.with_fix(edits);
    }
    diagnostics.push(diagnostic);
}
