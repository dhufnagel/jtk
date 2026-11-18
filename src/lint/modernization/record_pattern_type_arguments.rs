//! M005: type arguments of a record pattern (Checkstyle UnnecessaryTypeArgumentsWithRecordPattern), Java 21:
//! `case Box<String>(var s)` can be `case Box(var s)`, the compiler infers them. As in Checkstyle, only wildcards
//! (`Box<?>`) are accepted.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(pattern: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::RecordPattern { ty, .. } = &pattern.kind else { return };
    let K::Parameterized { base, args } = &ty.kind else { return };
    let is_wildcard = |a: &Tree| match &a.kind {
        K::Annotated { base, .. } => matches!(base.kind, K::Wildcard { .. }),
        kind => matches!(kind, K::Wildcard { .. }),
    };
    let Some(open) = context.tokens().next(base.end) else { return };
    if args.iter().any(|a| !is_wildcard(a)) {
        let message = "Unnecessary type arguments with record pattern";
        let fix = Edit::new(open.start..ty.end, "");
        diagnostics.push(Diagnostic::new(Rule::RecordPatternTypeArguments, open.start, message).with_fix(vec![fix]));
    }
}
