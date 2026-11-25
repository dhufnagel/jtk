//! Helpers shared by the rules of the group: recognising the test frameworks from the file's imports and fully
//! qualified names (never from a method's name alone: a project's own `assertEquals` is not JUnit's), and the
//! declared types of local names.

use std::ops::Range;

use crate::lint::Edit;
use crate::lint::tree::{Context, for_each_descendant, strip_parens};
use jtk_parser::ast::{ClassKind, K, LitKind, Modifier, Mods, Tree, VarDecl};

/// The test library a class belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Framework {
    JUnit3,
    JUnit4,
    JUnit5,
    TestNg,
    /// TestNG's `AssertJUnit`: JUnit's argument order in TestNG.
    TestNgJUnit,
    AssertJ,
    Mockito,
}

impl Framework {
    /// The framework of a class (fully qualified) that declares assertions or test annotations.
    pub fn of(class: &str) -> Option<Framework> {
        let package = class.rsplit_once('.').map_or("", |(p, _)| p);
        Some(match class {
            "junit.framework.Assert" | "junit.framework.TestCase" => Framework::JUnit3,
            "org.testng.AssertJUnit" => Framework::TestNgJUnit,
            "org.mockito.Mockito" | "org.mockito.BDDMockito" | "org.mockito.ArgumentMatchers" => Framework::Mockito,
            _ if in_package(package, "org.junit.jupiter") => Framework::JUnit5,
            _ if in_package(package, "org.junit") => Framework::JUnit4,
            _ if in_package(package, "org.testng") => Framework::TestNg,
            _ if package == "org.assertj.core.api" => Framework::AssertJ,
            _ if package == "org.mockito" => Framework::Mockito,
            _ => return None,
        })
    }

    /// Whether the expected value comes before the actual one in `assertEquals` (all but TestNG's `Assert`).
    pub fn expected_first(self) -> bool {
        self != Framework::TestNg
    }

    /// Whether an assertion's message is its first argument (JUnit 3 and 4), not its last.
    pub fn message_first(self) -> bool {
        matches!(self, Framework::JUnit3 | Framework::JUnit4 | Framework::TestNgJUnit)
    }

    /// The class with the `assert...` methods of the framework.
    pub fn assertions_class(self) -> Option<&'static str> {
        Some(match self {
            Framework::JUnit3 => "junit.framework.Assert",
            Framework::JUnit4 => "org.junit.Assert",
            Framework::JUnit5 => "org.junit.jupiter.api.Assertions",
            Framework::TestNg => "org.testng.Assert",
            Framework::TestNgJUnit => "org.testng.AssertJUnit",
            Framework::AssertJ | Framework::Mockito => return None,
        })
    }
}

/// Whether the package is `root` or one of its subpackages.
fn in_package(package: &str, root: &str) -> bool {
    package.strip_prefix(root).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// The classes with JUnit-style assertions (`assertEquals`, `assertTrue`, `fail`).
pub(super) const ASSERTION_CLASSES: &[&str] = &[
    "junit.framework.Assert",
    "junit.framework.TestCase",
    "org.junit.Assert",
    "org.junit.jupiter.api.Assertions",
    "org.testng.Assert",
    "org.testng.AssertJUnit",
];

/// The AssertJ classes with `assertThat`.
const ASSERTJ_CLASSES: &[&str] = &[
    "org.assertj.core.api.Assertions",
    "org.assertj.core.api.BDDAssertions",
    "org.assertj.core.api.AssertionsForClassTypes",
    "org.assertj.core.api.AssertionsForInterfaceTypes",
    "org.assertj.core.api.Fail",
];

/// The types an on-demand import (`import org.junit.*;`) is known to bring in: the names of other types cannot be
/// resolved through such an import.
const KNOWN_TYPES: &[&str] = &[
    "junit.framework.Assert",
    "junit.framework.TestCase",
    "org.junit.After",
    "org.junit.AfterClass",
    "org.junit.Assert",
    "org.junit.Before",
    "org.junit.BeforeClass",
    "org.junit.ClassRule",
    "org.junit.Ignore",
    "org.junit.Rule",
    "org.junit.Test",
    "org.junit.runner.RunWith",
    "org.junit.jupiter.api.AfterAll",
    "org.junit.jupiter.api.AfterEach",
    "org.junit.jupiter.api.Assertions",
    "org.junit.jupiter.api.BeforeAll",
    "org.junit.jupiter.api.BeforeEach",
    "org.junit.jupiter.api.Disabled",
    "org.junit.jupiter.api.Nested",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
    "org.junit.jupiter.api.extension.ExtendWith",
    "org.junit.jupiter.api.extension.RegisterExtension",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.migrationsupport.rules.EnableRuleMigrationSupport",
    "org.testng.Assert",
    "org.testng.AssertJUnit",
    "org.testng.annotations.Test",
    "org.assertj.core.api.Assertions",
    "org.assertj.core.api.BDDAssertions",
    "org.assertj.core.api.AssertionsForClassTypes",
    "org.assertj.core.api.AssertionsForInterfaceTypes",
    "org.assertj.core.api.Fail",
    "org.mockito.ArgumentMatchers",
    "org.mockito.BDDMockito",
    "org.mockito.Mock",
    "org.mockito.Mockito",
    "java.util.ArrayDeque",
    "java.util.ArrayList",
    "java.util.Collection",
    "java.util.Deque",
    "java.util.HashMap",
    "java.util.HashSet",
    "java.util.LinkedHashMap",
    "java.util.LinkedHashSet",
    "java.util.LinkedList",
    "java.util.List",
    "java.util.Map",
    "java.util.NavigableMap",
    "java.util.NavigableSet",
    "java.util.Optional",
    "java.util.Queue",
    "java.util.Set",
    "java.util.SortedMap",
    "java.util.SortedSet",
    "java.util.TreeMap",
    "java.util.TreeSet",
];

/// The `java.lang` types the rules look for.
const JAVA_LANG: &[&str] = &["String", "CharSequence", "Object", "Boolean", "Throwable", "Error", "AssertionError"];

/// The names a file brings into scope: its imports and the types and methods it declares.
pub(super) struct Names<'t> {
    unit: &'t Tree,
    text: &'t str,
    /// Single-type imports, fully qualified.
    types: Vec<String>,
    /// The packages (or types) of type-import-on-demand declarations.
    packages: Vec<String>,
    /// Single-static imports: the class and the member.
    members: Vec<(String, String)>,
    /// The classes of static-import-on-demand declarations.
    classes: Vec<String>,
}

impl<'t> Names<'t> {
    pub fn new(context: &Context<'t>) -> Option<Names<'t>> {
        let unit = *context.ancestors.first()?;
        Some(Names::of(unit, context.text))
    }

    pub fn of(unit: &'t Tree, text: &'t str) -> Names<'t> {
        let mut names =
            Names { unit, text, types: Vec::new(), packages: Vec::new(), members: Vec::new(), classes: Vec::new() };
        let K::CompilationUnit { imports, .. } = &unit.kind else { return names };
        for import in imports {
            let K::Import { is_static, name } = &import.kind else { continue };
            let name = compact(&text[name.start..name.end]);
            let Some((qualifier, last)) = name.rsplit_once('.') else { continue };
            let (qualifier, last) = (qualifier.to_string(), last.to_string());
            match (*is_static, last == "*") {
                (false, false) => names.types.push(name),
                (false, true) => names.packages.push(qualifier),
                (true, false) => names.members.push((qualifier, last)),
                (true, true) => names.classes.push(qualifier),
            }
        }
        names
    }

    /// The fully qualified name of a type written as `name` (simple or qualified), if it is imported, known to come
    /// from an on-demand import or `java.lang`, or written fully qualified. `None` for types declared in the file.
    pub fn resolve_type(&self, name: &str) -> Option<String> {
        let name = compact(name);
        let (first, rest) = match name.split_once('.') {
            Some((first, rest)) => (first, Some(rest)),
            None => (name.as_str(), None),
        };
        if first.starts_with(|c: char| c.is_ascii_lowercase()) {
            // A fully qualified name.
            return rest.map(|_| name.clone());
        }
        if self.declares_type(first) {
            return None;
        }
        let resolved = if let Some(import) = self.types.iter().find(|t| t.rsplit('.').next() == Some(first)) {
            import.clone()
        } else {
            let candidates: Vec<String> = self
                .packages
                .iter()
                .map(|p| format!("{p}.{first}"))
                .filter(|t| KNOWN_TYPES.contains(&t.as_str()))
                .collect();
            match candidates.as_slice() {
                [only] => only.clone(),
                [] if JAVA_LANG.contains(&first) => format!("java.lang.{first}"),
                _ => return None,
            }
        };
        Some(match rest {
            Some(rest) => format!("{resolved}.{rest}"),
            None => resolved,
        })
    }

    /// The fully qualified name of an annotation's type.
    pub fn annotation(&self, annotation: &Tree) -> Option<String> {
        let K::Annotation { ty, .. } = &annotation.kind else { return None };
        self.resolve_type(&self.text[ty.start..ty.end])
    }

    /// The fully qualified names of the annotations among the modifiers.
    pub fn annotations(&self, mods: &Mods) -> Vec<String> {
        mods.annotations.iter().filter_map(|a| self.annotation(a)).collect()
    }

    /// Whether one of the modifiers' annotations has one of the fully qualified names.
    pub fn has_annotation(&self, mods: &Mods, names: &[&str]) -> bool {
        self.annotations(mods).iter().any(|a| names.contains(&a.as_str()))
    }

    /// The class (fully qualified) declaring the static method that `call` invokes: from its qualifier
    /// (`Assertions.assertEquals`), or from the static imports for an unqualified call, or `junit.framework.TestCase`
    /// for an assertion called in a subclass of it. `None` if it is unknown or may be a method of the file.
    pub fn static_method_class(&self, call: &Tree, enclosing: &[&Tree]) -> Option<String> {
        let K::MethodInvocation { select, .. } = &call.kind else { return None };
        match &select.kind {
            K::Ident(method) => self.imported_method_class(method, enclosing),
            K::MemberSelect { expr, .. } if is_qualified_name(expr) => {
                self.resolve_type(&self.text[expr.start..expr.end])
            }
            _ => None,
        }
    }

    /// The class of an unqualified static method call.
    fn imported_method_class(&self, method: &str, enclosing: &[&Tree]) -> Option<String> {
        if self.declares_method(method) {
            return None;
        }
        let single: Vec<&String> = self.members.iter().filter(|(_, m)| m == method).map(|(c, _)| c).collect();
        if let Some(first) = single.first() {
            return single.iter().all(|c| c == first).then(|| first.to_string());
        }
        let mut found = None;
        for class in &self.classes {
            match declares_static_method(class, method) {
                None => return None,
                Some(true) if found.is_some() => return None,
                Some(true) => found = Some(class.clone()),
                Some(false) => {}
            }
        }
        if found.is_some() {
            return found;
        }
        let in_test_case = enclosing.iter().any(|tree| match &tree.kind {
            K::Class(class) => class.extends.as_deref().is_some_and(|e| {
                self.resolve_type(&self.text[e.start..e.end]).as_deref() == Some("junit.framework.TestCase")
            }),
            _ => false,
        });
        (in_test_case && (method.starts_with("assert") || method == "fail"))
            .then(|| "junit.framework.TestCase".to_string())
    }

    /// The framework of a static method call, with the method's name.
    pub fn framework_call<'a>(&self, call: &'a Tree, enclosing: &[&Tree]) -> Option<(Framework, &'a str)> {
        let name = method_name(call)?;
        let class = self.static_method_class(call, enclosing)?;
        Some((Framework::of(&class)?, name))
    }

    /// The framework of a JUnit-style assertion (`assertEquals`, `fail`) that `call` invokes.
    pub fn assertion<'a>(&self, call: &'a Tree, enclosing: &[&Tree]) -> Option<(Framework, &'a str)> {
        let name = method_name(call)?;
        if !name.starts_with("assert") && name != "fail" {
            return None;
        }
        let class = self.static_method_class(call, enclosing)?;
        ASSERTION_CLASSES.contains(&class.as_str()).then_some(())?;
        Some((Framework::of(&class)?, name))
    }

    /// Whether `call` is AssertJ's `assertThat(...)` (or `then(...)` of `BDDAssertions`).
    pub fn is_assertj_assert_that(&self, call: &Tree, enclosing: &[&Tree]) -> bool {
        let Some(name) = method_name(call) else { return false };
        (name == "assertThat" || name == "then")
            && self.static_method_class(call, enclosing).is_some_and(|c| ASSERTJ_CLASSES.contains(&c.as_str()))
    }

    /// The edits that make `call` (of a static method of `class`) call `new_name` of the same class instead: its name
    /// replaced, and a static import added if the call is unqualified and the new name is not imported yet. `None`
    /// if the new name cannot be imported (the file declares or imports a method of that name from elsewhere).
    pub fn rename_static_call(&self, call: &Tree, class: &str, new_name: &str) -> Option<Vec<Edit>> {
        let K::MethodInvocation { select, .. } = &call.kind else { return None };
        let name_start = select.end - method_name(call)?.len();
        let mut edits = vec![Edit::new(name_start..select.end, new_name)];
        if matches!(select.kind, K::Ident(_)) {
            edits.extend(self.static_import(class, new_name)?);
        }
        Some(edits)
    }

    /// The edits that replace `call` (of a static method of `class`) with a call of `new_name` of the same class with
    /// the arguments `args` (sources), keeping its qualifier; see [`Names::rename_static_call`].
    pub fn rewrite_static_call(&self, call: &Tree, class: &str, new_name: &str, args: &[&str]) -> Option<Vec<Edit>> {
        let K::MethodInvocation { select, type_args, .. } = &call.kind else { return None };
        if !type_args.is_empty() {
            return None;
        }
        let name_start = select.end - method_name(call)?.len();
        let mut edits = vec![Edit::new(name_start..call.end, format!("{new_name}({})", args.join(", ")))];
        if matches!(select.kind, K::Ident(_)) {
            edits.extend(self.static_import(class, new_name)?);
        }
        Some(edits)
    }

    /// The edit that imports the static method `method` of `class` if it is not imported yet; `None` if it cannot be
    /// (the file declares a method of that name, or imports one from another class).
    pub fn static_import(&self, class: &str, method: &str) -> Option<Option<Edit>> {
        if self.declares_method(method) {
            return None;
        }
        if self.members.iter().any(|(c, m)| m == method && c != class) {
            return None;
        }
        let imported =
            self.members.iter().any(|(c, m)| m == method && c == class) || self.classes.iter().any(|c| c == class);
        // Unqualified assertions of `TestCase` are inherited, unless imported.
        let inherited = class == "junit.framework.TestCase" && !self.members.iter().any(|(c, _)| c == class);
        if imported || inherited {
            return Some(None);
        }
        if self.classes.iter().any(|c| declares_static_method(c, method) != Some(false)) {
            return None;
        }
        let K::CompilationUnit { imports, package, .. } = &self.unit.kind else { return None };
        let position = match (imports.last(), package) {
            (Some(last), _) => last.end,
            (None, Some((_, name))) => self.text[name.end..].find(';').map(|i| name.end + i + 1)?,
            (None, None) => return None,
        };
        Some(Some(Edit::new(position..position, format!("\nimport static {class}.{method};"))))
    }

    /// Whether the file declares a type of that name.
    fn declares_type(&self, name: &str) -> bool {
        let mut found = false;
        for_each_descendant(self.unit, &mut |tree, _| {
            if let K::Class(class) = &tree.kind {
                found |= class.name == name || class.type_params.iter().any(|p| is_type_parameter(p, name));
            }
            if let K::Method(method) = &tree.kind {
                found |= method.type_params.iter().any(|p| is_type_parameter(p, name));
            }
            !found
        });
        found
    }

    /// Whether the file declares a method of that name.
    pub fn declares_method(&self, name: &str) -> bool {
        let mut found = false;
        for_each_descendant(self.unit, &mut |tree, _| {
            found |= matches!(&tree.kind, K::Method(method) if method.name == name);
            !found
        });
        found
    }
}

fn is_type_parameter(tree: &Tree, name: &str) -> bool {
    matches!(&tree.kind, K::TypeParameter { name: n, .. } if n == name)
}

/// Whether a known class (fully qualified) has a static method of that name; `None` for unknown classes.
fn declares_static_method(class: &str, method: &str) -> Option<bool> {
    let assertion = method.starts_with("assert") || method == "fail";
    let mockito = matches!(method, "when" | "verify" | "given" | "then" | "mock" | "spy");
    Some(match class {
        _ if ASSERTION_CLASSES.contains(&class) => assertion,
        _ if ASSERTJ_CLASSES.contains(&class) => {
            !(method.starts_with("assert") && !method.starts_with("assertThat")) && !mockito || method == "then"
        }
        "org.mockito.Mockito" => !assertion && method != "then" && method != "given",
        "org.mockito.BDDMockito" => !assertion,
        "org.mockito.ArgumentMatchers" => !assertion && !mockito && !method.starts_with("do"),
        "org.hamcrest.Matchers" | "org.hamcrest.CoreMatchers" => !assertion && !mockito,
        "org.hamcrest.MatcherAssert" => method == "assertThat",
        "org.junit.Assume" | "org.junit.jupiter.api.Assumptions" | "org.assertj.core.api.Assumptions" => {
            method.starts_with("assume")
        }
        _ => return None,
    })
}

/// The name of the invoked method.
pub(super) fn method_name(call: &Tree) -> Option<&str> {
    let K::MethodInvocation { select, .. } = &call.kind else { return None };
    match &select.kind {
        K::Ident(name) | K::MemberSelect { name, .. } => Some(name),
        _ => None,
    }
}

/// The arguments of a method invocation.
pub(super) fn arguments(call: &Tree) -> &[Tree] {
    match &call.kind {
        K::MethodInvocation { args, .. } => args,
        _ => &[],
    }
}

/// The receiver of a method invocation `receiver.name(...)`.
pub(super) fn receiver(call: &Tree) -> Option<&Tree> {
    let K::MethodInvocation { select, .. } = &call.kind else { return None };
    match &select.kind {
        K::MemberSelect { expr, .. } => Some(expr),
        _ => None,
    }
}

/// Whether the tree is a (possibly qualified) name: `a`, `a.b.C`.
fn is_qualified_name(tree: &Tree) -> bool {
    match &tree.kind {
        K::Ident(_) => true,
        K::MemberSelect { expr, .. } => is_qualified_name(expr),
        _ => false,
    }
}

/// The text without whitespace.
pub(super) fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The indices of the two compared values among the arguments of a JUnit-style `assertEquals` (or `assertSame`,
/// `assertNotEquals`, ...), in the order of the arguments. With a message first (JUnit 3 and 4), three arguments are
/// the message and the values if the first is a string literal, or the values and a delta if the last is a number
/// literal; other calls of three arguments are ambiguous.
pub(super) fn compared_values(framework: Framework, args: &[Tree]) -> Option<(usize, usize)> {
    let is_string = |t: &Tree| matches!(strip_parens(t).kind, K::Literal(LitKind::String));
    let is_number = |t: &Tree| {
        matches!(strip_parens(t).kind, K::Literal(LitKind::Int | LitKind::Long | LitKind::Float | LitKind::Double))
    };
    match args.len() {
        2 => Some((0, 1)),
        _ if !framework.message_first() => (args.len() <= 4).then_some((0, 1)),
        3 if is_string(&args[0]) => Some((1, 2)),
        3 if is_number(&args[2]) => Some((0, 1)),
        4 => Some((1, 2)),
        _ => None,
    }
}

/// Whether the expression is a constant: a literal (possibly negated), a class literal or a name in upper case
/// (`MAX_SIZE`, `Color.RED`).
pub(super) fn is_constant(tree: &Tree) -> bool {
    let tree = strip_parens(tree);
    let upper = |name: &str| {
        name.len() > 1
            && name.starts_with(|c: char| c.is_ascii_uppercase())
            && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    };
    match &tree.kind {
        K::Literal(_) => true,
        K::Unary { op, expr, .. } if op == "-" || op == "+" => matches!(strip_parens(expr).kind, K::Literal(_)),
        K::Ident(name) => upper(name),
        K::MemberSelect { name, expr } => name == "class" || upper(name) && is_qualified_name(expr),
        _ => false,
    }
}

/// Whether the method's body, or one of its parameters' types, mentions the name (as an identifier or member).
pub(super) fn mentions(tree: &Tree, name: &str) -> bool {
    let mut found = false;
    for_each_descendant(tree, &mut |child, _| {
        found |= match &child.kind {
            K::Ident(n) | K::MemberSelect { name: n, .. } => n == name,
            K::MemberRef { name: Some(n), .. } => n == name,
            _ => false,
        };
        !found
    });
    found
}

/// The declared type (its source) of the local variable, parameter or field `name` visible at the end of `path` (the
/// trees from the compilation unit down). `None` if no declaration is found in the enclosing code, if the name is
/// declared with extra array dimensions (`int a[]`), as a lambda parameter without a type or with `var`, and if it
/// may be a field inherited by an enclosing class (which has a supertype and does not declare the name itself).
pub(super) fn declared_type<'t>(path: &[&'t Tree], name: &str, text: &'t str) -> Option<&'t str> {
    for pair in path.windows(2).rev() {
        let (parent, child) = (pair[0], pair[1]);
        if let Some(found) = declaration_in(parent, child, name, text) {
            return found.filter(|t| *t != "var");
        }
        let may_inherit = match &parent.kind {
            K::Class(class) => class.extends.is_some() || !class.implements.is_empty(),
            K::NewClass { body: Some(_), .. } => true,
            _ => false,
        };
        if may_inherit {
            return None;
        }
    }
    None
}

/// The declaration of `name` in `parent` that is visible in its child `child`: `Some` with the type if found, or with
/// `None` if the search stops there without a type.
fn declaration_in<'t>(parent: &'t Tree, child: &Tree, name: &str, text: &'t str) -> Option<Option<&'t str>> {
    let found = |ty: Option<&'t str>| ty.map(Some);
    match &parent.kind {
        K::Block { stmts, .. } => {
            found(stmts.iter().filter(|s| s.end <= child.start).filter_map(|s| vars_type(s, name, text)).next_back())
        }
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => found(
            cases
                .iter()
                .flat_map(|c| &c.stmts)
                .filter(|s| s.end <= child.start)
                .filter_map(|s| vars_type(s, name, text))
                .next_back(),
        ),
        K::Class(class) => {
            let field = if class.kind != ClassKind::Annotation {
                class.members.iter().find_map(|m| vars_type(m, name, text))
            } else {
                None
            };
            let component = || class.record_components.iter().find_map(|c| parameter_type(c, name, text));
            let constant = class.enum_constants.iter().any(|c| c.name == name);
            if constant {
                return Some(None);
            }
            found(field.or_else(component))
        }
        K::For { init, .. } => found(init.first().and_then(|i| vars_type(i, name, text))),
        K::ForEach { var, .. } if var.name == name => found(type_without_dimensions(var, text)),
        K::Try { resources, catches, .. } => {
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if let Some(catch) = catch.filter(|c| c.param.name == name) {
                return Some(catch.param.ty.as_deref().map(|t| &text[t.start..t.end]));
            }
            found(resources.iter().find_map(|r| match &r.kind {
                K::Var(var) if var.name == name => type_without_dimensions(var, text),
                _ => None,
            }))
        }
        K::Method(method) => found(method.params.iter().find_map(|p| parameter_type(p, name, text))),
        K::Lambda { params, .. } => match params.iter().find(|p| p.name == name) {
            Some(VarDecl { ty: None, .. }) => Some(None),
            Some(param) => found(parameter_type(param, name, text)),
            None => None,
        },
        _ => None,
    }
}

/// The type of a local variable or field declaration that declares `name` without extra array dimensions.
fn vars_type<'t>(declaration: &Tree, name: &str, text: &'t str) -> Option<&'t str> {
    let K::Vars(vars) = &declaration.kind else { return None };
    type_without_dimensions(vars.iter().find(|v| v.name == name)?, text)
}

/// The type of the parameter `name`, if it is not a variable arity parameter.
fn parameter_type<'t>(parameter: &VarDecl, name: &str, text: &'t str) -> Option<&'t str> {
    let is_variable_arity = text[..parameter.name_pos].trim_end().ends_with("...");
    (parameter.name == name && !is_variable_arity).then(|| type_without_dimensions(parameter, text)).flatten()
}

/// The type of the variable unless it has dimensions after its name; `var` if it has no type.
fn type_without_dimensions<'t>(var: &VarDecl, text: &'t str) -> Option<&'t str> {
    match var.ty.as_deref() {
        None => Some("var"),
        Some(ty) if ty.end > var.name_pos => None,
        Some(ty) => Some(&text[ty.start..ty.end]),
    }
}

/// The type of an expression if it is known locally: of a string literal, `new X(...)`, an array creation, or a name
/// declared in the file (see [`declared_type`]). `path` leads from the compilation unit to a tree containing it.
pub(super) fn local_type(expression: &Tree, path: &[&Tree], text: &str) -> Option<String> {
    let expression = strip_parens(expression);
    match &expression.kind {
        K::Literal(LitKind::String) => Some("String".to_string()),
        K::Ident(name) => declared_type(path, name, text).map(compact),
        K::NewClass { ident, body: None, .. } => Some(compact(&text[ident.start..ident.end])),
        K::NewArray { elem: Some(elem), dims, init, .. } => {
            // The brackets after the element type: `new int[] {1}`, `new int[n][]`.
            let end = match init {
                Some(_) => text[elem.end..expression.end].find('{').map_or(expression.end, |i| elem.end + i),
                None => expression.end,
            };
            let after = dims.last().map_or(elem.end, |d| d.end);
            let brackets = dims.len() + text[after..end].matches('[').count();
            Some(format!("{}{}", compact(&text[elem.start..elem.end]), "[]".repeat(brackets)))
        }
        _ => None,
    }
}

/// The edit that removes a modifier keyword with the spaces after it.
pub(super) fn keyword_removal(range: Range<usize>, text: &str) -> Edit {
    let trailing = text[range.end..].len() - text[range.end..].trim_start_matches([' ', '\t']).len();
    Edit::new(range.start..range.end + trailing, "")
}

/// The range of a modifier keyword.
pub(super) fn keyword(mods: &Mods, modifier: Modifier) -> Option<Range<usize>> {
    mods.keywords.iter().find(|(m, _)| *m == modifier).map(|(_, r)| r.clone())
}

/// Whether the member of a class is static: a static method, field, initializer or nested type.
pub(super) fn is_static_member(member: &Tree) -> bool {
    match &member.kind {
        K::Method(method) => method.mods.has(Modifier::Static),
        K::Vars(vars) => vars.first().is_some_and(|v| v.mods.has(Modifier::Static)),
        K::Block { is_static, .. } => *is_static,
        K::Class(class) => class.mods.has(Modifier::Static) || class.kind != ClassKind::Class,
        _ => false,
    }
}

/// The trees from the compilation unit down to `tree`.
pub(super) fn path_with<'t>(context: &Context<'t>, tree: &'t Tree) -> Vec<&'t Tree> {
    let mut path = context.ancestors.clone();
    path.push(tree);
    path
}
