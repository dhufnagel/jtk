//! C022: a method or constructor whose cognitive complexity reaches 15 (PMD CognitiveComplexity with its default
//! `reportLevel`).
//!
//! Follows PMD's computation (`CognitiveComplexityVisitor`), including its quirks:
//! - `if`, loops, `switch`, `catch` and `?:` add one plus the nesting level and nest their parts (an `if` only its
//!   `then` branch, after its condition); `else` and `else if` add one and nest the `else` branch, so the branches of
//!   a long `else if` chain are nested deeper and deeper.
//! - Lambdas and class bodies nest their code without adding to the complexity.
//! - A sequence of `&&` or of `||` adds one; the sequence goes on until the other of the two operators, a `!`, or the
//!   next statement of a block (not the next statement in a `switch` group, nor the next expression).
//! - A labeled `break` or `continue` adds one, and so does a recursive call.
//!
//! PMD resolves the called method to find recursive calls. Here a call is recursive if it has the name and number of
//! arguments of a method that encloses it, and no receiver or one whose type is obviously the method's class (see
//! [`Visit::recursive_callee`]).
//!
//! Not in the shared walk of the Checkstyle metrics: those follow Checkstyle's syntax tree, this rule PMD's.

use std::collections::HashMap;
use std::ptr;

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassDecl, ClassKind, K, LitKind, MethodDecl, Tree, VarDecl};

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    if method.body.is_none() || method.compact_ctor {
        return;
    }
    let Some(parent) = context.parent() else { return };
    let report_level = context.options.cognitive_complexity.report_level;
    // Like PMD, the enclosing methods count for recursive calls.
    let ancestors = &context.ancestors;
    let methods = (1..ancestors.len())
        .filter(|&i| matches!(ancestors[i].kind, K::Method(_)))
        .filter_map(|i| Method::new(ancestors[i], Owner::of(ancestors[i], ancestors[i - 1])))
        .collect();
    let mut visit =
        Visit { context, complexity: 0, nesting: 0, boolean_operator: None, methods, variables: HashMap::new() };
    visit.method(tree, Owner::of(tree, parent));
    let complexity = visit.complexity;
    if complexity >= report_level {
        let kind = if method.ret.is_some() { "method" } else { "constructor" };
        let types: Vec<String> = method.params.iter().map(|p| parameter_type(p, context.text)).collect();
        let message = format!(
            "The {kind} '{}({})' has a cognitive complexity of {complexity}, current threshold is {report_level}",
            method.name,
            types.join(", ")
        );
        diagnostics.push(Diagnostic::new(Rule::CognitiveComplexity, method.name_pos, message));
    }
}

/// The members of a class body, with the class if the body is a class's (not an anonymous class's or an enum
/// constant's).
#[derive(Clone, Copy)]
struct Owner<'t> {
    members: &'t [Tree],
    class: Option<&'t ClassDecl>,
}

impl<'t> Owner<'t> {
    /// The body that declares the member, given the member's parent.
    fn of(member: &Tree, parent: &'t Tree) -> Owner<'t> {
        let holds = |members: &[Tree]| members.as_ptr_range().contains(&ptr::from_ref(member));
        match &parent.kind {
            K::Class(c) if holds(&c.members) => Owner { members: &c.members, class: Some(c) },
            K::Class(c) => {
                let body = c.enum_constants.iter().filter_map(|e| e.body.as_deref()).find(|b| holds(b));
                Owner { members: body.unwrap_or_default(), class: None }
            }
            K::NewClass { body: Some(body), .. } => Owner { members: body, class: None },
            // A method of a compact source file.
            K::CompilationUnit { types, .. } => Owner { members: types, class: None },
            _ => Owner { members: &[], class: None },
        }
    }

    /// The class that declares the members, if it is a class, enum or record (whose instances can be receivers).
    fn instance_class(&self) -> Option<&'t ClassDecl> {
        self.class.filter(|c| matches!(c.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Record))
    }
}

/// A method that encloses the code, for finding recursive calls.
#[derive(Clone, Copy)]
struct Method<'t> {
    tree: &'t Tree,
    declaration: &'t MethodDecl,
    owner: Owner<'t>,
}

impl<'t> Method<'t> {
    /// The tree as a method (not a constructor or an element of an annotation type) of the class body.
    fn new(tree: &'t Tree, owner: Owner<'t>) -> Option<Method<'t>> {
        let K::Method(declaration) = &tree.kind else { return None };
        let is_method = declaration.ret.is_some() && owner.class.is_none_or(|c| c.kind != ClassKind::Annotation);
        is_method.then_some(Method { tree, declaration, owner })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanOperator {
    And,
    Or,
}

/// PMD's visit of a method's code.
struct Visit<'t, 'c> {
    context: &'c Context<'t>,
    complexity: u32,
    nesting: u32,
    boolean_operator: Option<BooleanOperator>,
    methods: Vec<Method<'t>>,
    /// The types of the variables declared so far (`None` for `var`), by name, regardless of their scope.
    variables: HashMap<&'t str, Option<&'t Tree>>,
}

impl<'t> Visit<'t, '_> {
    fn boolean_operation(&mut self, operator: Option<BooleanOperator>) {
        if self.boolean_operator != operator {
            if operator.is_some() {
                self.complexity += 1;
            }
            self.boolean_operator = operator;
        }
    }

    /// An `if`, loop, ...: adds one plus the nesting level and nests.
    fn structural(&mut self) {
        self.complexity += 1 + self.nesting;
        self.nesting += 1;
    }

    fn unnest(&mut self) {
        self.nesting = self.nesting.saturating_sub(1);
    }

    fn declare(&mut self, variable: &'t VarDecl) {
        self.variables.insert(&variable.name, variable.ty.as_deref());
    }

    fn children(&mut self, tree: &'t Tree) {
        tree.for_each_child(&mut |child| self.visit(child));
    }

    fn visit(&mut self, tree: &'t Tree) {
        match &tree.kind {
            K::If { cond, then, else_ } => self.if_statement(cond, then, else_.as_deref(), false),
            K::Block { stmts, .. } => {
                for statement in stmts.iter().filter(|s| !matches!(s.kind, K::Empty)) {
                    self.boolean_operation(None);
                    self.visit(statement);
                }
            }
            K::Break(label) | K::Continue(label) => self.complexity += u32::from(label.is_some()),
            K::MethodInvocation { .. } => {
                if self.recursive_callee(tree, 0).is_some() {
                    self.complexity += 1;
                }
                self.children(tree);
            }
            K::Binary { op, .. } | K::Unary { op, .. } => {
                match op.as_str() {
                    "&&" => self.boolean_operation(Some(BooleanOperator::And)),
                    "||" => self.boolean_operation(Some(BooleanOperator::Or)),
                    "!" => self.boolean_operation(None),
                    _ => {}
                }
                self.children(tree);
            }
            K::Vars(variables) => {
                variables.iter().for_each(|v| self.declare(v));
                self.children(tree);
            }
            K::Var(variable) => {
                self.declare(variable);
                self.children(tree);
            }
            K::ForEach { var, .. } => {
                self.declare(var);
                self.structural();
                self.children(tree);
                self.unnest();
            }
            K::For { .. }
            | K::While { .. }
            | K::DoWhile { .. }
            | K::Switch { .. }
            | K::SwitchExpr { .. }
            | K::Conditional { .. } => {
                self.structural();
                self.children(tree);
                self.unnest();
            }
            K::Try { resources, block, catches, finally } => {
                resources.iter().for_each(|r| self.visit(r));
                self.visit(block);
                for catch in catches {
                    self.structural();
                    self.declare(&catch.param);
                    self.variable(&catch.param);
                    self.visit(&catch.block);
                    self.unnest();
                }
                finally.iter().for_each(|f| self.visit(f));
            }
            K::Lambda { params, .. } => {
                self.nesting += 1;
                // Not the parameters with inferred types.
                params.iter().filter(|p| p.ty.is_some()).for_each(|p| self.declare(p));
                self.children(tree);
                self.unnest();
            }
            K::Class(class) => self.class(class),
            K::NewClass { outer, type_args, ident, args, body } => {
                outer.iter().for_each(|o| self.visit(o));
                type_args.iter().for_each(|t| self.visit(t));
                self.visit(ident);
                args.iter().for_each(|a| self.visit(a));
                if let Some(body) = body {
                    self.nesting += 1;
                    self.members(Owner { members: body, class: None });
                    self.unnest();
                }
            }
            _ => self.children(tree),
        }
    }

    /// An `if` directly in an `if` (its `else if`, or its `then` branch without braces) is an `else if` for PMD: it
    /// adds nothing and does not nest its `then` branch.
    fn if_statement(&mut self, cond: &'t Tree, then: &'t Tree, alternative: Option<&'t Tree>, in_if: bool) {
        self.visit(cond);
        if !in_if {
            self.structural();
        }
        self.branch(then);
        if !in_if {
            self.unnest();
        }
        if let Some(alternative) = alternative {
            self.complexity += 1;
            self.nesting += 1;
            self.branch(alternative);
            self.unnest();
        }
    }

    fn branch(&mut self, statement: &'t Tree) {
        match &statement.kind {
            K::If { cond, then, else_ } => self.if_statement(cond, then, else_.as_deref(), true),
            _ => self.visit(statement),
        }
    }

    /// The annotations, type and initializer of a variable.
    fn variable(&mut self, variable: &'t VarDecl) {
        variable.mods.annotations.iter().for_each(|a| self.visit(a));
        variable.ty.iter().for_each(|t| self.visit(t));
        variable.init.iter().for_each(|i| self.visit(i));
    }

    fn class(&mut self, class: &'t ClassDecl) {
        class.mods.annotations.iter().for_each(|a| self.visit(a));
        class.type_params.iter().for_each(|t| self.visit(t));
        for component in &class.record_components {
            self.declare(component);
            self.variable(component);
        }
        class.extends.iter().for_each(|e| self.visit(e));
        class.implements.iter().chain(&class.permits).for_each(|t| self.visit(t));
        for constant in &class.enum_constants {
            constant.mods.annotations.iter().chain(&constant.args).for_each(|t| self.visit(t));
            if let Some(body) = &constant.body {
                self.nesting += 1;
                self.members(Owner { members: body, class: None });
                self.unnest();
            }
        }
        // The bodies of records, enums and annotation types are not class bodies for PMD.
        let nests = matches!(class.kind, ClassKind::Class | ClassKind::Interface);
        if nests {
            self.nesting += 1;
        }
        self.members(Owner { members: &class.members, class: Some(class) });
        if nests {
            self.unnest();
        }
    }

    /// The members of a class body; fields declare no variables.
    fn members(&mut self, owner: Owner<'t>) {
        for member in owner.members {
            match &member.kind {
                K::Vars(_) => self.children(member),
                K::Method(_) => self.method(member, owner),
                _ => self.visit(member),
            }
        }
    }

    /// A method, constructor or element of an annotation type.
    fn method(&mut self, tree: &'t Tree, owner: Owner<'t>) {
        let K::Method(declaration) = &tree.kind else { return };
        let method = Method::new(tree, owner);
        self.methods.extend(method);
        declaration.params.iter().for_each(|p| self.declare(p));
        self.children(tree);
        if method.is_some() {
            self.methods.pop();
        }
    }

    /// The enclosing method the call calls, if it is a recursive call, as PMD finds with type information: a call
    /// with the name and arity of an enclosing method, without receiver or on a receiver of the method's class
    /// (`this`, a variable or field declared with the class as type, the class name, an enum constant of the class,
    /// ...). If the method is overloaded with an arity that accepts the arguments, it must be the overload a rough
    /// overload resolution chooses, which knows the types of casts, literals and variables declared in the method.
    fn recursive_callee(&self, call: &Tree, depth: usize) -> Option<Method<'t>> {
        let K::MethodInvocation { select, args, .. } = &call.kind else { return None };
        let (name, receiver) = match &select.kind {
            K::Ident(name) => (name, None),
            K::MemberSelect { expr, name } => (name, Some(strip_parens(expr))),
            _ => return None,
        };
        if !self.methods.iter().any(|m| m.declaration.name == *name) {
            return None;
        }
        let is_candidate = |tree: &Tree| match &tree.kind {
            K::Method(m) => m.ret.is_some() && m.name == *name && self.accepts(m, args.len()),
            _ => false,
        };
        self.methods.iter().rev().copied().find(|method| {
            let is_this = |r: &Tree| matches!(&r.kind, K::Ident(n) if n == "this");
            if !is_candidate(method.tree)
                || receiver.is_some_and(|r| !is_this(r) && !self.is_of_class(r, method, depth))
            {
                return false;
            }
            // Overload resolution, roughly: overloads that need no variable arity call first, then the one that matches
            // most argument types (none if that is not clear).
            let overloads: Vec<&Tree> = method.owner.members.iter().filter(|m| is_candidate(m)).collect();
            let fixed: Vec<&Tree> = overloads.iter().copied().filter(|m| parameters(m).len() == args.len()).collect();
            let applicable = if fixed.is_empty() { overloads } else { fixed };
            if let [only] = applicable.as_slice() {
                return ptr::eq(*only, method.tree);
            }
            let argument_types: Vec<Option<String>> =
                args.iter().map(|a| self.argument_type(a, &method.owner)).collect();
            let score = |m: &Tree| self.matching_arguments(m, &argument_types);
            let own = score(method.tree);
            applicable.iter().any(|m| ptr::eq(*m, method.tree))
                && own > 0
                && applicable.iter().all(|m| ptr::eq(*m, method.tree) || score(m) < own)
        })
    }

    /// Whether the receiver of a call has the class that declares the method as type.
    fn is_of_class(&self, receiver: &Tree, method: &Method, depth: usize) -> bool {
        let Some(class) = method.owner.instance_class() else { return false };
        let is_class = |type_name: &str| type_name.rsplit('.').next() == Some(class.name.as_str());
        let text = self.context.text;
        match &receiver.kind {
            K::Ident(name) if name == "super" => false,
            K::Ident(name) => match self.variables.get(name.as_str()) {
                Some(declared) => is_class(&declared_type(*declared, text)),
                None => field_type(class, name, text).is_some_and(|t| is_class(&t)) || *name == class.name,
            },
            K::MemberSelect { expr, name } => {
                matches!(&expr.kind, K::Ident(n) if n == "this")
                    && field_type(class, name, text).is_some_and(|t| is_class(&t))
            }
            K::NewClass { ident, body: None, .. } => is_class(&type_name(ident, text)),
            K::Cast { ty, .. } => is_class(&type_name(ty, text)),
            K::MethodInvocation { .. } if depth < 8 => self
                .recursive_callee(receiver, depth + 1)
                .is_some_and(|callee| callee.declaration.ret.as_deref().is_some_and(|t| is_class(&type_name(t, text)))),
            _ => false,
        }
    }

    /// Whether the method accepts the number of arguments.
    fn accepts(&self, method: &MethodDecl, arguments: usize) -> bool {
        let parameters = method.params.len();
        arguments == parameters || self.is_variable_arity(method) && arguments + 1 >= parameters
    }

    fn is_variable_arity(&self, method: &MethodDecl) -> bool {
        method.params.last().is_some_and(|last| {
            let before_name = self.context.tokens().previous(last.name_pos);
            before_name.is_some_and(|t| &self.context.text[t] == "...")
        })
    }

    /// How many of the arguments have a known type that is the parameter's type: a rough overload resolution.
    fn matching_arguments(&self, tree: &Tree, argument_types: &[Option<String>]) -> usize {
        let K::Method(method) = &tree.kind else { return 0 };
        let text = self.context.text;
        let types: Vec<String> = method.params.iter().map(|p| erasure(&parameter_type(p, text))).collect();
        let is_variable_arity = self.is_variable_arity(method);
        let last = types.len().saturating_sub(1);
        let matches = |(i, argument): (usize, &Option<String>)| {
            let Some(argument_type) = argument else { return false };
            match types.get(i.min(last)) {
                Some(t) if is_variable_arity && i >= last => {
                    argument_type == t || Some(argument_type.as_str()) == t.strip_suffix("[]")
                }
                Some(t) => argument_type == t,
                None => false,
            }
        };
        argument_types.iter().enumerate().filter(|a| matches(*a)).count()
    }

    /// The type of an argument, if obvious: a cast, a literal, a variable declared in the method, or a call of a
    /// method of the class (`owner`) whose overloads all return the same type.
    fn argument_type(&self, argument: &Tree, owner: &Owner) -> Option<String> {
        let argument = strip_parens(argument);
        let text = self.context.text;
        let source = &text[argument.start..argument.end];
        let known = match &argument.kind {
            K::Cast { ty, .. } => return Some(type_name(ty, text)),
            K::Literal(kind) => match kind {
                LitKind::String => "String",
                LitKind::Boolean => "boolean",
                LitKind::Char => "char",
                LitKind::Int => "int",
                LitKind::Long => "long",
                // Not hexadecimal floating-point literals, which PMD's grammar treats apart.
                LitKind::Float | LitKind::Double if source.starts_with("0x") || source.starts_with("0X") => {
                    return None;
                }
                LitKind::Float => "float",
                LitKind::Double => "double",
                LitKind::Null => return None,
            },
            K::Ident(name) => return self.variables.get(name.as_str()).map(|t| declared_type(*t, text)),
            K::MethodInvocation { select, .. } => {
                let name = match &select.kind {
                    K::Ident(name) => name,
                    K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(n) if n == "this") => name,
                    _ => return None,
                };
                let mut types = owner.members.iter().filter_map(|m| match &m.kind {
                    K::Method(m) if m.ret.is_some() && m.name == *name => m.ret.as_deref().map(|t| type_name(t, text)),
                    _ => None,
                });
                let first = types.next()?;
                return types.all(|t| t == first).then_some(first);
            }
            _ => return None,
        };
        Some(known.to_string())
    }
}

/// The parameters of a method (not its receiver).
fn parameters(tree: &Tree) -> &[VarDecl] {
    match &tree.kind {
        K::Method(m) => &m.params,
        _ => &[],
    }
}

/// The type of a field declared in the class, or the class's name for one of its enum constants.
fn field_type(class: &ClassDecl, name: &str, text: &str) -> Option<String> {
    if class.enum_constants.iter().any(|e| e.name == name) {
        return Some(class.name.clone());
    }
    class.members.iter().find_map(|member| match &member.kind {
        K::Vars(vars) => vars.iter().find(|v| v.name == name).and_then(|v| v.ty.as_deref()).map(|t| type_name(t, text)),
        _ => None,
    })
}

/// The type a declaration gives its variable (`var` if it is inferred), without type arguments.
fn declared_type(ty: Option<&Tree>, text: &str) -> String {
    ty.map_or_else(|| "var".to_string(), |t| type_name(t, text))
}

/// The type of a parameter as PMD displays it: `Map<K, ? extends V>`, `String[]` for `String...`.
fn parameter_type(parameter: &VarDecl, text: &str) -> String {
    let mut code = String::new();
    if let Some(ty) = &parameter.ty {
        append_type(ty, text, &mut code);
    }
    code
}

/// The text of a type without type arguments.
fn type_name(ty: &Tree, text: &str) -> String {
    let mut code = String::new();
    append_type(ty, text, &mut code);
    erasure(&code)
}

/// Appends the text of a type without annotations and comments, its tokens separated like PMD does.
fn append_type(ty: &Tree, text: &str, code: &mut String) {
    let list = |types: &[Tree], separator: &str, code: &mut String| {
        for (i, t) in types.iter().enumerate() {
            if i > 0 {
                code.push_str(separator);
            }
            append_type(t, text, code);
        }
    };
    match &ty.kind {
        K::Ident(name) | K::Primitive(name) => code.push_str(name),
        K::MemberSelect { expr, name } => {
            append_type(expr, text, code);
            code.push('.');
            code.push_str(name);
        }
        K::ArrayType { elem, dims } => {
            append_type(elem, text, code);
            dims.iter().for_each(|_| code.push_str("[]"));
        }
        K::Parameterized { base, args } => {
            append_type(base, text, code);
            code.push('<');
            list(args, ", ", code);
            code.push('>');
        }
        K::Wildcard { bound } => {
            code.push('?');
            if let Some((extends, bound)) = bound {
                code.push_str(if *extends { " extends " } else { " super " });
                append_type(bound, text, code);
            }
        }
        K::Annotated { base, .. } => append_type(base, text, code),
        K::Union(types) => list(types, "|", code),
        K::Intersection(types) => list(types, "&", code),
        _ => code.push_str(&text[ty.start..ty.end]),
    }
}

/// A type without its type arguments: `List` for `List<String>`.
fn erasure(code: &str) -> String {
    let mut depth = 0;
    let mut erased = String::new();
    for c in code.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ if depth == 0 => erased.push(c),
            _ => {}
        }
    }
    erased
}
