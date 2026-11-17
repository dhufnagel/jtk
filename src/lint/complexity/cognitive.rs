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
//! [`is_recursive_call`]).
//!
//! Not in the shared walk of the Checkstyle metrics: those follow Checkstyle's syntax tree, this rule PMD's.

use std::collections::HashMap;

use tree_sitter::{Node, TreeCursor};

use crate::lint::syntax::{is_comment, named_children, operator, parameter_name, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule, RuleOptions};

/// A method that encloses the code, for finding recursive calls.
#[derive(Clone, Copy)]
struct Method<'t> {
    name: &'t str,
    declaration: Node<'t>,
    /// The body of the type that declares it.
    owner: Node<'t>,
}

impl<'t> Method<'t> {
    fn new(declaration: Node<'t>, owner: Node<'t>, text: &'t str) -> Method<'t> {
        let name = declaration.child_by_field_name("name").map_or("", |n| source(&n, text));
        Method { name, declaration, owner }
    }
}

pub fn check(program: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let report_level = options.cognitive_complexity.report_level;
    // The ancestors of the cursor's node, the parent last.
    let mut ancestors: Vec<Node> = Vec::new();
    let mut cursor = program.walk();
    loop {
        let node = cursor.node();
        if matches!(node.kind(), "method_declaration" | "constructor_declaration")
            && node.child_by_field_name("body").is_some()
        {
            let complexity = complexity(&node, &ancestors, text);
            if complexity >= report_level {
                report(&node, (complexity, report_level), text, diagnostics);
            }
        }
        if cursor.goto_first_child() {
            ancestors.push(node);
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
            ancestors.pop();
        }
    }
}

fn parameters<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    declaration.child_by_field_name("parameters").map_or(Vec::new(), |p| {
        named_children(&p).into_iter().filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter")).collect()
    })
}

fn report(declaration: &Node, (complexity, report_level): (u32, u32), text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = declaration.child_by_field_name("name") else { return };
    let kind = if declaration.kind() == "method_declaration" { "method" } else { "constructor" };
    let types: Vec<String> = parameters(declaration).iter().map(|p| parameter_type(p, text)).collect();
    let message = format!(
        "The {kind} '{}({})' has a cognitive complexity of {complexity}, current threshold is {report_level}",
        source(&name, text),
        types.join(", ")
    );
    diagnostics.push(Diagnostic::new(Rule::CognitiveComplexity, name.start_byte(), message));
}

/// The type of a parameter as PMD displays it: `Map<K, ? extends V>`, `String[]` for `String...`.
fn parameter_type(parameter: &Node, text: &str) -> String {
    let mut code = String::new();
    for part in named_children(parameter) {
        if !part.is_extra() && !matches!(part.kind(), "modifiers" | "variable_declarator" | "identifier") {
            append_tokens(&part, text, &mut code);
        }
    }
    if parameter.kind() == "spread_parameter" {
        code.push_str("[]");
    }
    code
}

/// Appends the tokens of a type without annotations and comments, separated like PMD does.
fn append_tokens(type_node: &Node, text: &str, code: &mut String) {
    let mut pending = vec![*type_node];
    while let Some(node) = pending.pop() {
        if node.is_extra() || matches!(node.kind(), "annotation" | "marker_annotation") {
            continue;
        }
        if node.child_count() > 0 {
            pending.extend(node.children(&mut node.walk()).collect::<Vec<_>>().into_iter().rev());
            continue;
        }
        let token = source(&node, text);
        let is_word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '?');
        if is_word(code.chars().last()) && is_word(token.chars().next()) {
            code.push(' ');
        }
        code.push_str(token);
        if token == "," {
            code.push(' ');
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanOperator {
    And,
    Or,
}

/// A step of PMD's visit of the method's code.
enum Step<'t> {
    /// Visits a node with its parent.
    Visit(Node<'t>, Node<'t>),
    /// An `if`, loop, ...: adds one plus the nesting level and nests.
    Structural,
    /// An `else`: adds one and nests.
    Hybrid,
    Nest,
    Unnest,
    EndBooleanSequence,
    LeaveMethod,
}

struct State<'t> {
    complexity: u32,
    nesting: u32,
    boolean_operator: Option<BooleanOperator>,
    methods: Vec<Method<'t>>,
    /// The declarations of the variables declared so far, by name, regardless of their scope.
    variables: HashMap<&'t str, Node<'t>>,
}

impl<'t> State<'t> {
    fn boolean_operation(&mut self, operator: Option<BooleanOperator>) {
        if self.boolean_operator != operator {
            if operator.is_some() {
                self.complexity += 1;
            }
            self.boolean_operator = operator;
        }
    }

    /// The type of a variable declared in the method, without type arguments.
    fn variable_type(&self, name: &str, text: &str) -> Option<String> {
        self.variables.get(name).and_then(|declaration| declared_type(declaration, text))
    }
}

/// The ids of the node kinds the visit handles: comparing ids is much cheaper than comparing kind names.
struct Kinds {
    if_statement: u16,
    blocks: [u16; 2],
    jumps: [u16; 2],
    binary_expression: u16,
    unary_expression: u16,
    method_declaration: u16,
    declarations: [u16; 5],
    method_invocation: u16,
    enhanced_for_statement: u16,
    structural: [u16; 7],
    record_declaration: u16,
    class_body: u16,
    nesting: [u16; 3],
}

impl Kinds {
    fn get() -> &'static Kinds {
        static KINDS: std::sync::OnceLock<Kinds> = std::sync::OnceLock::new();
        KINDS.get_or_init(|| {
            let language: tree_sitter::Language = tree_sitter_java_orchard::LANGUAGE.into();
            let id = |kind: &str| language.id_for_node_kind(kind, true);
            Kinds {
                if_statement: id("if_statement"),
                blocks: [id("block"), id("constructor_body")],
                jumps: [id("break_statement"), id("continue_statement")],
                binary_expression: id("binary_expression"),
                unary_expression: id("unary_expression"),
                method_declaration: id("method_declaration"),
                declarations: [
                    id("local_variable_declaration"),
                    id("formal_parameter"),
                    id("spread_parameter"),
                    id("catch_formal_parameter"),
                    id("resource"),
                ],
                method_invocation: id("method_invocation"),
                enhanced_for_statement: id("enhanced_for_statement"),
                structural: [
                    id("for_statement"),
                    id("enhanced_for_statement"),
                    id("while_statement"),
                    id("do_statement"),
                    id("switch_expression"),
                    id("catch_clause"),
                    id("ternary_expression"),
                ],
                record_declaration: id("record_declaration"),
                class_body: id("class_body"),
                nesting: [id("lambda_expression"), id("class_body"), id("interface_body")],
            }
        })
    }
}

/// Pushes the visits of the node's children, each preceded by `before`, in reverse order as the steps are a stack.
/// Without `before`, children without named children are left out: they add nothing.
fn push_children<'t>(
    steps: &mut Vec<Step<'t>>,
    cursor: &mut TreeCursor<'t>,
    node: &Node<'t>,
    before: Option<fn() -> Step<'t>>,
) {
    let start = steps.len();
    for child in node.named_children(cursor) {
        if child.is_extra() || before.is_none() && child.named_child_count() == 0 {
            continue;
        }
        if let Some(before) = before {
            steps.push(before());
        }
        steps.push(Step::Visit(child, *node));
    }
    steps[start..].reverse();
}

/// The cognitive complexity of a method or constructor with the given ancestors.
fn complexity<'t>(declaration: &Node<'t>, ancestors: &[Node<'t>], text: &'t str) -> u32 {
    let kinds = Kinds::get();
    // Like PMD, the enclosing methods count for recursive calls.
    let methods = ancestors
        .iter()
        .enumerate()
        .filter(|(i, n)| *i > 0 && n.kind_id() == kinds.method_declaration)
        .map(|(i, n)| Method::new(*n, ancestors[i - 1], text))
        .collect();
    let mut state = State { complexity: 0, nesting: 0, boolean_operator: None, methods, variables: HashMap::new() };
    let Some(parent) = ancestors.last() else { return 0 };
    let mut steps = vec![Step::Visit(*declaration, *parent)];
    let mut cursor = declaration.walk();
    while let Some(step) = steps.pop() {
        let (node, parent) = match step {
            Step::Visit(node, parent) => (node, parent),
            Step::Structural => {
                state.complexity += 1 + state.nesting;
                state.nesting += 1;
                continue;
            }
            Step::Hybrid => {
                state.complexity += 1;
                state.nesting += 1;
                continue;
            }
            Step::Nest => {
                state.nesting += 1;
                continue;
            }
            Step::Unnest => {
                state.nesting = state.nesting.saturating_sub(1);
                continue;
            }
            Step::EndBooleanSequence => {
                state.boolean_operation(None);
                continue;
            }
            Step::LeaveMethod => {
                state.methods.pop();
                continue;
            }
        };
        let kind = node.kind_id();
        if kind == kinds.if_statement {
            // An `if` directly in an `if` (its `else if`, or its `then` branch without braces) is an `else if` for
            // PMD: it adds nothing and does not nest its `then` branch. Pushed in reverse order.
            let field = |name| node.child_by_field_name(name).map(|n| Step::Visit(n, node));
            let in_if = parent.kind_id() == kinds.if_statement;
            if let Some(alternative) = field("alternative") {
                steps.extend([Step::Unnest, alternative, Step::Hybrid]);
            }
            if !in_if {
                steps.push(Step::Unnest);
            }
            steps.extend(field("consequence"));
            if !in_if {
                steps.push(Step::Structural);
            }
            steps.extend(field("condition"));
        } else if kinds.blocks.contains(&kind) {
            push_children(&mut steps, &mut cursor, &node, Some(|| Step::EndBooleanSequence));
        } else if kinds.jumps.contains(&kind) {
            // With a label.
            if node.named_children(&mut node.walk()).any(|c| !c.is_extra()) {
                state.complexity += 1;
            }
        } else if kind == kinds.method_invocation {
            if is_recursive_call(&node, &state, text) {
                state.complexity += 1;
            }
            push_children(&mut steps, &mut cursor, &node, None);
        } else if kind == kinds.binary_expression || kind == kinds.unary_expression {
            match operator(&node, text) {
                "&&" => state.boolean_operation(Some(BooleanOperator::And)),
                "||" => state.boolean_operation(Some(BooleanOperator::Or)),
                "!" if kind == kinds.unary_expression => state.boolean_operation(None),
                _ => {}
            }
            push_children(&mut steps, &mut cursor, &node, None);
        } else if kinds.declarations.contains(&kind) {
            declare_variables(&node, &mut state.variables, text);
            push_children(&mut steps, &mut cursor, &node, None);
        } else if kinds.structural.contains(&kind) {
            if kind == kinds.enhanced_for_statement {
                declare_variables(&node, &mut state.variables, text);
            }
            steps.push(Step::Unnest);
            push_children(&mut steps, &mut cursor, &node, None);
            steps.push(Step::Structural);
        } else if kinds.nesting.contains(&kind)
            && !(kind == kinds.class_body && parent.kind_id() == kinds.record_declaration)
        {
            // A record's body is not a class body for PMD.
            steps.push(Step::Unnest);
            push_children(&mut steps, &mut cursor, &node, None);
            steps.push(Step::Nest);
        } else if kind == kinds.method_declaration {
            state.methods.push(Method::new(node, parent, text));
            steps.push(Step::LeaveMethod);
            push_children(&mut steps, &mut cursor, &node, None);
        } else {
            push_children(&mut steps, &mut cursor, &node, None);
        }
    }
    state.complexity
}

/// Records the variables a declaration declares.
fn declare_variables<'t>(declaration: &Node<'t>, variables: &mut HashMap<&'t str, Node<'t>>, text: &'t str) {
    let mut declare = |name: Option<Node>| {
        if let Some(name) = name {
            variables.insert(source(&name, text), *declaration);
        }
    };
    match declaration.kind() {
        "local_variable_declaration" => {
            for declarator in named_children(declaration).iter().filter(|d| d.kind() == "variable_declarator") {
                declare(declarator.child_by_field_name("name"));
            }
        }
        "formal_parameter" | "spread_parameter" => declare(parameter_name(declaration)),
        _ => declare(declaration.child_by_field_name("name")),
    }
}

/// The type a declaration gives its variables, without type arguments.
fn declared_type(declaration: &Node, text: &str) -> Option<String> {
    match declaration.kind() {
        "formal_parameter" | "spread_parameter" => Some(erasure(&parameter_type(declaration, text))),
        "catch_formal_parameter" => {
            named_children(declaration).iter().find(|c| c.kind() == "catch_type").map(|t| type_name(t, text))
        }
        _ => declaration.child_by_field_name("type").map(|t| type_name(&t, text)),
    }
}

/// The text of a type without type arguments.
fn type_name(type_node: &Node, text: &str) -> String {
    let mut code = String::new();
    append_tokens(type_node, text, &mut code);
    erasure(&code)
}

/// Whether the call is a call of an enclosing method, as PMD finds with type information: a call with the name and
/// arity of an enclosing method, without receiver or on a receiver of the method's class (`this`, a variable or field
/// declared with the class as type, the class name, an enum constant of the class, ...). If the method is overloaded
/// with an arity that accepts the arguments, it must be the overload a rough overload resolution chooses, which knows
/// the types of casts, literals and variables declared in the method.
fn is_recursive_call(call: &Node, state: &State, text: &str) -> bool {
    recursive_callee(call, state, text, 0).is_some()
}

/// The enclosing method the call calls, if it is a recursive call (see [`is_recursive_call`]).
fn recursive_callee<'t>(call: &Node, state: &State<'t>, text: &str, depth: usize) -> Option<Method<'t>> {
    let name = call.child_by_field_name("name").map_or("", |n| source(&n, text));
    if !state.methods.iter().any(|m| m.name == name) {
        return None;
    }
    let arguments: Vec<Node> = call
        .child_by_field_name("arguments")
        .map_or(Vec::new(), |a| named_children(&a).into_iter().filter(|c| !is_comment(c)).collect());
    let is_candidate = |declaration: &Node| {
        declaration.kind() == "method_declaration"
            && declaration.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
            && accepts(declaration, arguments.len())
    };
    let receiver = call.child_by_field_name("object").map(|o| strip_parentheses(&o));
    state.methods.iter().rev().copied().find(|method| {
        if !is_candidate(&method.declaration)
            || receiver.is_some_and(|r| r.kind() != "this" && !is_of_class(&r, method, state, text, depth))
        {
            return false;
        }
        // Overload resolution, roughly: overloads that need no variable arity call first, then the one that matches
        // most argument types (none if that is not clear).
        let overloads: Vec<Node> = named_children(&method.owner).into_iter().filter(|m| is_candidate(m)).collect();
        let fixed: Vec<&Node> = overloads.iter().filter(|m| parameters(m).len() == arguments.len()).collect();
        let applicable: Vec<&Node> = if fixed.is_empty() { overloads.iter().collect() } else { fixed };
        if let [only] = applicable.as_slice() {
            return **only == method.declaration;
        }
        let argument_types: Vec<Option<String>> =
            arguments.iter().map(|a| argument_type(a, &method.owner, state, text)).collect();
        let score = |m: &Node| matching_arguments(m, &argument_types, text);
        let own = score(&method.declaration);
        applicable.contains(&&method.declaration)
            && own > 0
            && applicable.iter().all(|m| **m == method.declaration || score(m) < own)
    })
}

/// Whether the receiver of a call has the class that declares the method as type.
fn is_of_class(receiver: &Node, method: &Method, state: &State, text: &str, depth: usize) -> bool {
    // Only the rare calls of a method with an enclosing method's name get here: `Node::parent` is fine.
    let Some(class) = method.owner.parent() else { return false };
    if !matches!(class.kind(), "class_declaration" | "enum_declaration" | "record_declaration" | "enum_body") {
        return false;
    }
    let class = if class.kind() == "enum_body" { class.parent().unwrap_or(class) } else { class };
    let Some(class_name) = class.child_by_field_name("name").map(|n| source(&n, text)) else { return false };
    let is_class = |type_name: &str| type_name.rsplit('.').next() == Some(class_name);
    match receiver.kind() {
        "identifier" => {
            let name = source(receiver, text);
            match state.variables.get(name).map(|d| declared_type(d, text)) {
                Some(declared) => declared.is_some_and(|d| is_class(&d)),
                None => field_type(&class, name, text).is_some_and(|t| is_class(&t)) || name == class_name,
            }
        }
        "field_access" => {
            let is_this = receiver.child_by_field_name("object").is_some_and(|o| o.kind() == "this");
            let field = receiver.child_by_field_name("field").map_or("", |f| source(&f, text));
            is_this && field_type(&class, field, text).is_some_and(|t| is_class(&t))
        }
        "object_creation_expression" => {
            !named_children(receiver).iter().any(|c| c.kind() == "class_body")
                && receiver.child_by_field_name("type").is_some_and(|t| is_class(&type_name(&t, text)))
        }
        "cast_expression" => receiver.child_by_field_name("type").is_some_and(|t| is_class(&type_name(&t, text))),
        "method_invocation" if depth < 8 => recursive_callee(receiver, state, text, depth + 1).is_some_and(|callee| {
            callee.declaration.child_by_field_name("type").is_some_and(|t| is_class(&type_name(&t, text)))
        }),
        _ => false,
    }
}

/// The type of a field declared in the class, or the class's name for one of its enum constants.
fn field_type(class: &Node, name: &str, text: &str) -> Option<String> {
    let body = class.child_by_field_name("body")?;
    let mut members = named_children(&body);
    if let Some(declarations) = members.iter().find(|m| m.kind() == "enum_body_declarations") {
        members.extend(named_children(declarations));
    }
    members.iter().find_map(|member| match member.kind() {
        "enum_constant" if member.child_by_field_name("name").is_some_and(|n| source(&n, text) == name) => {
            class.child_by_field_name("name").map(|n| source(&n, text).to_string())
        }
        "field_declaration" => {
            let declares = named_children(member).iter().any(|d| {
                d.kind() == "variable_declarator"
                    && d.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
            });
            if declares { member.child_by_field_name("type").map(|t| type_name(&t, text)) } else { None }
        }
        _ => None,
    })
}

fn accepts(declaration: &Node, arguments: usize) -> bool {
    let parameters = parameters(declaration);
    let is_variable_arity = parameters.last().is_some_and(|p| p.kind() == "spread_parameter");
    arguments == parameters.len() || is_variable_arity && arguments + 1 >= parameters.len()
}

/// How many of the arguments have a known type that is the parameter's type: a rough overload resolution.
fn matching_arguments(declaration: &Node, argument_types: &[Option<String>], text: &str) -> usize {
    let parameters = parameters(declaration);
    let types: Vec<String> = parameters.iter().map(|p| erasure(&parameter_type(p, text))).collect();
    let is_variable_arity = parameters.last().is_some_and(|p| p.kind() == "spread_parameter");
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

/// The type of an argument, if obvious: a cast, a literal, a variable declared in the method, or a call of a method
/// of the class (`owner`) whose overloads all return the same type.
fn argument_type(argument: &Node, owner: &Node, state: &State, text: &str) -> Option<String> {
    let argument = strip_parentheses(argument);
    let known = match argument.kind() {
        "cast_expression" => return argument.child_by_field_name("type").map(|t| type_name(&t, text)),
        "string_literal" => "String",
        "true" | "false" => "boolean",
        "character_literal" => "char",
        "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal" => {
            if source(&argument, text).ends_with(['l', 'L']) {
                "long"
            } else {
                "int"
            }
        }
        "decimal_floating_point_literal" => {
            if source(&argument, text).ends_with(['f', 'F']) {
                "float"
            } else {
                "double"
            }
        }
        "identifier" => return state.variable_type(source(&argument, text), text),
        "method_invocation" if argument.child_by_field_name("object").is_none_or(|o| o.kind() == "this") => {
            let name = argument.child_by_field_name("name").map(|n| source(&n, text));
            let mut types = named_children(owner)
                .into_iter()
                .filter(|m| {
                    m.kind() == "method_declaration" && m.child_by_field_name("name").map(|n| source(&n, text)) == name
                })
                .map(|m| m.child_by_field_name("type").map(|t| type_name(&t, text)));
            let first = types.next()??;
            return types.all(|t| t.as_deref() == Some(first.as_str())).then_some(first);
        }
        _ => return None,
    };
    Some(known.to_string())
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
