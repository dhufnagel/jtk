//! K027: `synchronized` on an instance of a boxed primitive or another value-based class: such instances are cached
//! and shared (`Integer.valueOf`, autoboxing), so unrelated code may lock the same object, and they may lose their
//! identity in future Java versions (Error Prone LockOnBoxedPrimitive, IntelliJ SynchronizeOnValueBasedClass, javac
//! `synchronization` lint).
//!
//! No fix. Only locks whose type is known locally are checked: a variable declared in the file, a cast or `new X()`.
//! The classes are the boxed primitives and the value-based classes of `java.util` (`Optional*`) and `java.time`.

use super::shared::has_type;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const VALUE_BASED: &[&str] = &[
    "java.lang.Boolean",
    "java.lang.Byte",
    "java.lang.Character",
    "java.lang.Short",
    "java.lang.Integer",
    "java.lang.Long",
    "java.lang.Float",
    "java.lang.Double",
    "java.lang.ProcessHandle",
    "java.lang.Runtime.Version",
    "java.util.Optional",
    "java.util.OptionalInt",
    "java.util.OptionalLong",
    "java.util.OptionalDouble",
    "java.time.Duration",
    "java.time.Instant",
    "java.time.LocalDate",
    "java.time.LocalDateTime",
    "java.time.LocalTime",
    "java.time.MonthDay",
    "java.time.OffsetDateTime",
    "java.time.OffsetTime",
    "java.time.Period",
    "java.time.Year",
    "java.time.YearMonth",
    "java.time.ZonedDateTime",
    "java.time.ZoneId",
    "java.time.ZoneOffset",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Synchronized { lock, .. } = &tree.kind else { return };
    if has_type(lock, &context.ancestors, VALUE_BASED, context) {
        let message = "Synchronization on an instance of a value-based class";
        diagnostics.push(Diagnostic::new(Rule::SynchronizeOnValueBased, lock.start, message));
    }
}
