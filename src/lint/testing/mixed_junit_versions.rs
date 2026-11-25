//! T009: annotations of one JUnit version in a test class of another (IntelliJ JUnitMixedFramework, PMD
//! WrongTestAnnotation, baseline JUnit5RuleUsage, errorprone JUnitAmbiguousTestClass): the engine that runs the
//! class ignores them, so a `@Before` in a JUnit 5 class or a `@Rule` in a JUnit 5 class is never applied, and a
//! JUnit 3 `TestCase` subclass runs its `test...` methods but ignores `@Test`.
//!
//! The class's version comes from its test methods: JUnit 4's `@Test`, or JUnit 5's `@Test`,
//! `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory` and `@TestTemplate` (the version with more of them if
//! both occur, JUnit 5 on a tie), or JUnit 3 for a subclass of `junit.framework.TestCase`. Reported are the
//! annotations of its methods and fields that belong to another version, recognised by the imports or the fully
//! qualified name: JUnit 4's `@Test`, `@Before`, `@After`, `@BeforeClass`, `@AfterClass`, `@Ignore`, `@Rule` and
//! `@ClassRule` (not `@RunWith`: the `JUnitPlatform` runner runs JUnit 5 tests), and all of JUnit 5's. `@Rule` and
//! `@ClassRule` are accepted in a JUnit 5 class with `@EnableRuleMigrationSupport`. Classes without test methods
//! (abstract bases) are not checked.
//!
//! No fix: replacing an annotation needs its counterpart's import, and some have none (`@Rule`).

use super::shared::Names;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Mods, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    JUnit3,
    JUnit4,
    JUnit5,
}

impl Version {
    fn name(self) -> &'static str {
        match self {
            Version::JUnit3 => "JUnit 3",
            Version::JUnit4 => "JUnit 4",
            Version::JUnit5 => "JUnit 5",
        }
    }
}

const JUNIT4: &[&str] = &[
    "org.junit.Test",
    "org.junit.Before",
    "org.junit.After",
    "org.junit.BeforeClass",
    "org.junit.AfterClass",
    "org.junit.Ignore",
    "org.junit.Rule",
    "org.junit.ClassRule",
];

const JUNIT5_TESTS: &[&str] = &[
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
    "org.junit.jupiter.params.ParameterizedTest",
];

/// The version of a JUnit annotation (fully qualified).
fn version(annotation: &str) -> Option<Version> {
    if JUNIT4.contains(&annotation) {
        Some(Version::JUnit4)
    } else if annotation.starts_with("org.junit.jupiter.") {
        Some(Version::JUnit5)
    } else {
        None
    }
}

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    let Some(names) = Names::new(context) else { return };
    let member_mods: Vec<&Mods> = class
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            K::Method(method) => Some(&method.mods),
            K::Vars(vars) => vars.first().map(|v| &v.mods),
            _ => None,
        })
        .collect();
    let count = |tests: &[&str]| member_mods.iter().filter(|mods| names.has_annotation(mods, tests)).count();
    let (junit4, junit5) = (count(&["org.junit.Test"]), count(JUNIT5_TESTS));
    let extends_test_case = class
        .extends
        .as_deref()
        .is_some_and(|e| names.resolve_type(context.source(e)).is_some_and(|t| t == "junit.framework.TestCase"));
    let class_version = if extends_test_case {
        Version::JUnit3
    } else if junit4 == 0 && junit5 == 0 {
        return;
    } else if junit4 > junit5 {
        Version::JUnit4
    } else {
        Version::JUnit5
    };
    let migration =
        names.has_annotation(&class.mods, &["org.junit.jupiter.migrationsupport.rules.EnableRuleMigrationSupport"]);
    for annotation in member_mods.iter().flat_map(|mods| &mods.annotations) {
        let Some(name) = names.annotation(annotation) else { continue };
        let Some(version) = version(&name) else { continue };
        let migrated_rule = migration && (name == "org.junit.Rule" || name == "org.junit.ClassRule");
        if version != class_version && !migrated_rule {
            let message = format!("{} annotation in a {} test class", version.name(), class_version.name());
            diagnostics.push(Diagnostic::new(Rule::MixedJunitVersions, annotation.start, message));
        }
    }
}
