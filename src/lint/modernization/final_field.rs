//! M012: private field that is only assigned in its declaration or its class's constructors, but not `final` (PMD
//! ImmutableField, Eclipse save action "Add final modifier to private fields").
//!
//! As in PMD: fields that are private, or whose class is private, local or anonymous; not `static`, `volatile` or
//! `final`; not in a class annotated with Lombok's `@Builder`, `@Data`, `@Setter` or `@Value`, nor annotated with
//! `@Setter` or picocli's `@Option` or `@Parameters`. Such a field is reported if it is never assigned but has a
//! value, or if it has no value and each constructor assigns it exactly once (constructors that call `this(...)`
//! not at all). Assignments in lambdas, initializer blocks or nested classes, also in constructors, count as
//! assignments elsewhere.
//!
//! Without types, assignments are matched to fields by name and scope: `x` and `this.x` refer to the innermost field
//! of that name unless a local variable or parameter hides it, `Outer.this.x` to the field of class `Outer`, and
//! `other.x` to every field named `x`. PMD follows the data flow through the constructor, so that an assignment in
//! each branch of an `if` counts as one; jtk only accepts an assignment as a statement of the constructor body, and
//! no `return` in the constructor. A field that is assigned in a constructor and also has a value is not reported
//! (it could not be final; PMD reports some). On the other hand, PMD does not see that a constructor calling
//! `this(...)` assigns the field through the other constructor.
//!
//! The fix adds `final` to declarations whose variables are all reported, unless a constructor or initializer reads
//! a field without value before it is assigned, which is a compile error for a final field.

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::lint::syntax::{
    has_modifier, is_comment, is_name_reference_in, modifiers, named_children, operator, source,
};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut analysis = Analysis {
        text,
        uses_lombok: text.contains("lombok"),
        types: Vec::new(),
        constructors: Vec::new(),
        writes: HashMap::new(),
        reads: HashMap::new(),
        field_names: HashSet::new(),
    };
    analysis.walk(program);
    analysis.report(diagnostics);
}

struct Analysis<'t, 'a> {
    text: &'a str,
    uses_lombok: bool,
    types: Vec<Type<'t, 'a>>,
    constructors: Vec<Constructor>,
    /// Assignments of fields, by field name.
    writes: HashMap<&'a str, Vec<Access>>,
    /// Reads of fields in constructors and initializers, by field name.
    reads: HashMap<&'a str, Vec<Access>>,
    /// The names of the fields of the types seen so far: a simple name can only refer to one of these.
    field_names: HashSet<&'a str>,
}

struct Type<'t, 'a> {
    name: Option<&'a str>,
    effectively_private: bool,
    fields: Vec<Field<'t, 'a>>,
    field_names: Vec<&'a str>,
}

struct Field<'t, 'a> {
    declaration: Node<'t>,
    /// Each variable's name node, name and whether it has a value.
    variables: Vec<(Node<'t>, &'a str, bool)>,
    candidate: bool,
}

struct Constructor {
    type_index: usize,
    delegates: bool,
    has_return: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Type(usize),
    /// `other.x`: any field of the name.
    Any,
}

#[derive(Clone, Copy, PartialEq)]
enum Context {
    /// In a constructor (its index); whether as a statement of its body.
    Constructor(usize, bool),
    /// In a field's value or an initializer block of the type.
    Initializer(usize),
    Elsewhere,
}

#[derive(Clone, Copy)]
struct Access {
    target: Target,
    context: Context,
    offset: usize,
}

/// A node on the walk's stack, with the names it declares for its later descendants.
struct Frame<'t, 'a> {
    node: Node<'t>,
    kind: FrameKind,
    locals: Vec<&'a str>,
}

#[derive(Clone, Copy, PartialEq)]
enum FrameKind {
    /// A type body: the type's index.
    Type(usize),
    Constructor(usize),
    Lambda,
    /// An initializer block or a field's declarator: the type's index.
    Initializer(usize),
    Other,
}

const LOMBOK_CLASS_ANNOTATIONS: &[&str] = &["Builder", "Data", "Setter", "Value"];
const INVALIDATING_FIELD_ANNOTATIONS: &[&str] = &["Setter", "Option", "Parameters"];
const TYPE_BODIES: &[&str] = &["class_body", "interface_body", "enum_body", "annotation_type_body"];

impl<'t, 'a> Analysis<'t, 'a> {
    fn walk(&mut self, program: &Node<'t>) {
        let mut frames: Vec<Frame<'t, 'a>> = Vec::new();
        let mut cursor = program.walk();
        loop {
            let frame = self.enter(cursor.node(), &mut frames);
            frames.push(frame);
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                frames.pop();
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    fn enter(&mut self, node: Node<'t>, frames: &mut [Frame<'t, 'a>]) -> Frame<'t, 'a> {
        let text = self.text;
        let parent_kind = frames.last().map_or("", |f| f.node.kind());
        let mut frame = Frame { node, kind: FrameKind::Other, locals: Vec::new() };
        if !node.is_named() {
            return frame;
        }
        let innermost_type = || frames.iter().rev().find_map(|f| f.kind.type_index());
        match node.kind() {
            kind if TYPE_BODIES.contains(&kind) => frame.kind = FrameKind::Type(self.add_type(&node, frames)),
            "constructor_declaration" | "compact_constructor_declaration" => {
                if let Some(type_index) = innermost_type() {
                    let body = node.child_by_field_name("body");
                    let first = body.and_then(|b| named_children(&b).into_iter().find(|c| !is_comment(c)));
                    let delegates = first.is_some_and(|s| {
                        s.kind() == "explicit_constructor_invocation"
                            && s.child_by_field_name("constructor").is_some_and(|c| c.kind() == "this")
                    });
                    let has_return = body.is_some_and(|b| contains_return(&b));
                    self.constructors.push(Constructor { type_index, delegates, has_return });
                    frame.kind = FrameKind::Constructor(self.constructors.len() - 1);
                }
                frame.locals = parameter_names(&node, text);
            }
            "method_declaration" => frame.locals = parameter_names(&node, text),
            "lambda_expression" => {
                frame.kind = FrameKind::Lambda;
                frame.locals = match node.child_by_field_name("parameters") {
                    Some(p) if p.kind() == "identifier" => vec![source(&p, text)],
                    Some(p) => named_children(&p)
                        .iter()
                        .filter_map(|c| match c.kind() {
                            "identifier" => Some(source(c, text)),
                            _ => c.child_by_field_name("name").map(|n| source(&n, text)),
                        })
                        .collect(),
                    None => Vec::new(),
                };
            }
            "block" if matches!(parent_kind, "class_body" | "enum_body_declarations") => {
                if let Some(index) = innermost_type() {
                    frame.kind = FrameKind::Initializer(index);
                }
            }
            "variable_declarator" if parent_kind == "field_declaration" => {
                let is_static = frames.last().is_some_and(|p| has_modifier(&p.node, text, "static"));
                if let Some(index) = innermost_type().filter(|_| !is_static) {
                    frame.kind = FrameKind::Initializer(index);
                }
            }
            // A local variable is visible after its declarator in the enclosing block or `for` statement.
            "variable_declarator" if parent_kind == "local_variable_declaration" => {
                if let (Some(name), [.., scope, _]) = (node.child_by_field_name("name"), &mut *frames) {
                    scope.locals.push(source(&name, text));
                }
            }
            "catch_formal_parameter" | "resource" => {
                // Visible in the catch clause, or the try statement.
                let depth = if node.kind() == "resource" { 2 } else { 1 };
                if let Some(name) = node.child_by_field_name("name")
                    && frames.len() >= depth
                {
                    let index = frames.len() - depth;
                    frames[index].locals.push(source(&name, text));
                }
            }
            "enhanced_for_statement" | "instanceof_expression" => {
                // A pattern variable is only taken as visible inside its `instanceof`: an assignment elsewhere then
                // counts as one of a field, which keeps the field from being reported.
                if let Some(name) = node.child_by_field_name("name") {
                    frame.locals.push(source(&name, text));
                }
            }
            "type_pattern" | "record_pattern_component" => {
                if let Some(name) = named_children(&node).into_iter().rfind(|c| c.kind() == "identifier") {
                    frame.locals.push(source(&name, text));
                }
            }
            "identifier" => self.access(&node, frames),
            _ => {}
        }
        frame
    }

    /// Adds the type of a body with its fields; returns its index.
    fn add_type(&mut self, body: &Node<'t>, frames: &[Frame<'t, 'a>]) -> usize {
        let text = self.text;
        // The declaration (or anonymous class or enum constant) whose body this is, and what encloses it.
        let declaration = frames.last().map(|f| f.node);
        let enclosing_kind = frames.len().checked_sub(2).map_or("program", |i| frames[i].node.kind());
        let is_named = declaration.is_some_and(|d| d.kind().ends_with("_declaration"));
        let is_member =
            matches!(enclosing_kind, "program" | "enum_body_declarations") || TYPE_BODIES.contains(&enclosing_kind);
        let enclosing_private =
            frames.iter().rev().find_map(|f| f.kind.type_index()).is_some_and(|i| self.types[i].effectively_private);
        let effectively_private = enclosing_private
            || !is_named
            || !is_member
            || declaration.is_some_and(|d| has_modifier(&d, text, "private"));
        let name =
            declaration.filter(|_| is_named).and_then(|d| d.child_by_field_name("name")).map(|n| source(&n, text));
        let lombok_class = self.uses_lombok
            && declaration
                .is_some_and(|d| annotation_names(&d, text).iter().any(|a| LOMBOK_CLASS_ANNOTATIONS.contains(a)));
        let is_interface = matches!(body.kind(), "interface_body" | "annotation_type_body");
        let is_record = declaration.is_some_and(|d| d.kind() == "record_declaration");
        let members = match body.kind() {
            "enum_body" => named_children(body)
                .into_iter()
                .filter(|c| c.kind() == "enum_body_declarations")
                .flat_map(|d| named_children(&d))
                .collect(),
            _ => named_children(body),
        };
        let mut fields = Vec::new();
        let mut field_names = Vec::new();
        for declaration in members.into_iter().filter(|m| m.kind() == "field_declaration") {
            let variables: Vec<(Node, &str, bool)> = named_children(&declaration)
                .into_iter()
                .filter(|d| d.kind() == "variable_declarator")
                .filter_map(|d| {
                    let name = d.child_by_field_name("name")?;
                    Some((name, source(&name, text), d.child_by_field_name("value").is_some()))
                })
                .collect();
            field_names.extend(variables.iter().map(|v| v.1));
            let modifier = |keyword| has_modifier(&declaration, text, keyword);
            let candidate = !is_interface
                && !is_record
                && !lombok_class
                && !modifier("static")
                && !modifier("final")
                && !modifier("volatile")
                && (modifier("private") || effectively_private)
                && !annotation_names(&declaration, text).iter().any(|a| INVALIDATING_FIELD_ANNOTATIONS.contains(a));
            fields.push(Field { declaration, variables, candidate });
        }
        self.field_names.extend(field_names.iter().copied());
        self.types.push(Type { name, effectively_private, fields, field_names });
        self.types.len() - 1
    }

    /// Records an identifier that may read or assign a field.
    fn access(&mut self, identifier: &Node<'t>, frames: &[Frame<'t, 'a>]) {
        let text = self.text;
        let name = source(identifier, text);
        let Some(parent) = frames.last() else { return };
        // The expression that is read or assigned, the field it refers to, and the frames around that expression.
        let (expression, target, outer) = if parent.node.kind() == "field_access" {
            if parent.node.child_by_field_name("field") != Some(*identifier) {
                return;
            }
            let Some(object) = parent.node.child_by_field_name("object") else { return };
            let target = match object.kind() {
                "this" => match frames.iter().rev().find_map(|f| f.kind.type_index()) {
                    Some(index) if self.types[index].field_names.contains(&name) => Target::Type(index),
                    _ => return,
                },
                // `Outer.this.x`
                "field_access" if object.child_by_field_name("field").is_some_and(|f| f.kind() == "this") => {
                    let class = object.child_by_field_name("object").map(|o| source(&o, text));
                    let types = frames.iter().rev().filter_map(|f| f.kind.type_index());
                    match types.into_iter().find(|&i| self.types[i].name.is_some() && self.types[i].name == class) {
                        Some(index) if self.types[index].field_names.contains(&name) => Target::Type(index),
                        _ => return,
                    }
                }
                _ => Target::Any,
            };
            (parent.node, target, &frames[..frames.len() - 1])
        } else {
            if !self.field_names.contains(name) || !is_name_reference_in(identifier, &parent.node) {
                return;
            }
            let Some(target) = self.resolve(name, frames) else { return };
            (*identifier, target, frames)
        };
        let Some(holder) = outer.last() else { return };
        let (is_write, is_read) = match holder.node.kind() {
            "assignment_expression" if holder.node.child_by_field_name("left") == Some(expression) => {
                (true, operator(&holder.node, text) != "=")
            }
            "update_expression" => (true, true),
            _ => (false, true),
        };
        if is_write {
            let in_body = matches!(outer, [.., body, statement, _]
                if statement.node.kind() == "expression_statement" && body.node.kind() == "constructor_body");
            let context = match outer.iter().rev().find(|f| f.kind != FrameKind::Other).map(|f| f.kind) {
                Some(FrameKind::Constructor(index)) => Context::Constructor(index, in_body),
                _ => Context::Elsewhere,
            };
            self.writes.entry(name).or_default().push(Access { target, context, offset: expression.start_byte() });
        }
        if is_read {
            // Lambdas run later, but cannot read a final field before it is assigned either.
            let context = match outer.iter().rev().find(|f| !matches!(f.kind, FrameKind::Other | FrameKind::Lambda)) {
                Some(Frame { kind: FrameKind::Constructor(index), .. }) => Context::Constructor(*index, false),
                Some(Frame { kind: FrameKind::Initializer(index), .. }) => Context::Initializer(*index),
                _ => return,
            };
            self.reads.entry(name).or_default().push(Access { target, context, offset: expression.start_byte() });
        }
    }

    /// The field a simple name refers to: the innermost local variable, parameter or field of the name.
    fn resolve(&self, name: &str, frames: &[Frame<'t, 'a>]) -> Option<Target> {
        for frame in frames.iter().rev() {
            if frame.locals.contains(&name) {
                return None;
            }
            if let FrameKind::Type(index) = frame.kind
                && self.types[index].field_names.contains(&name)
            {
                return Some(Target::Type(index));
            }
        }
        None
    }

    fn report(&self, diagnostics: &mut Vec<Diagnostic>) {
        for (type_index, class) in self.types.iter().enumerate() {
            for field in class.fields.iter().filter(|f| f.candidate) {
                let reported: Vec<(Node, bool)> = field
                    .variables
                    .iter()
                    .filter_map(|&(name_node, name, has_value)| {
                        self.is_immutable(type_index, name, has_value).map(|fixable| (name_node, fixable))
                    })
                    .collect();
                let fixable = reported.len() == field.variables.len() && reported.iter().all(|r| r.1);
                for (name_node, _) in &reported {
                    let name = source(name_node, self.text);
                    let message = format!("Private field '{name}' could be made final");
                    let diagnostic = Diagnostic::new(Rule::FinalField, name_node.start_byte(), message);
                    let insertion = field.declaration.child_by_field_name("type").map(|t| t.start_byte());
                    diagnostics.push(match insertion.filter(|_| fixable) {
                        Some(offset) => diagnostic.with_fix(vec![Edit::new(offset..offset, "final ")]),
                        None => diagnostic,
                    });
                }
            }
        }
    }

    /// Whether the field could be final (`None` if not), and whether adding `final` compiles.
    fn is_immutable(&self, type_index: usize, name: &str, has_value: bool) -> Option<bool> {
        let refers = |access: &&Access| access.target == Target::Type(type_index) || access.target == Target::Any;
        let writes: Vec<&Access> = self.writes.get(name).map_or(Vec::new(), |w| w.iter().filter(refers).collect());
        if writes.is_empty() {
            return has_value.then_some(true);
        }
        if has_value {
            return None;
        }
        // Every constructor of the type assigns the field once as a statement, or calls another one and does not.
        let mut assignments = HashMap::new();
        for write in &writes {
            match write.context {
                Context::Constructor(index, true)
                    if write.target == Target::Type(type_index)
                        && self.constructors[index].type_index == type_index =>
                {
                    if assignments.insert(index, write.offset).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        for (index, constructor) in self.constructors.iter().enumerate().filter(|(_, c)| c.type_index == type_index) {
            let assigned = assignments.contains_key(&index);
            if constructor.has_return || assigned == constructor.delegates {
                return None;
            }
        }
        let read_before = self.reads.get(name).is_some_and(|reads| {
            reads.iter().filter(refers).any(|read| match read.context {
                Context::Initializer(index) => index == type_index,
                Context::Constructor(index, _) => assignments.get(&index).is_some_and(|&offset| read.offset < offset),
                Context::Elsewhere => false,
            })
        });
        Some(!read_before)
    }
}

impl FrameKind {
    fn type_index(self) -> Option<usize> {
        match self {
            FrameKind::Type(index) => Some(index),
            _ => None,
        }
    }
}

fn parameter_names<'a>(declaration: &Node, text: &'a str) -> Vec<&'a str> {
    let Some(parameters) = declaration.child_by_field_name("parameters") else { return Vec::new() };
    named_children(&parameters)
        .iter()
        .filter_map(|p| match p.kind() {
            "formal_parameter" => p.child_by_field_name("name"),
            "spread_parameter" => named_children(p)
                .into_iter()
                .find(|c| c.kind() == "variable_declarator")
                .and_then(|d| d.child_by_field_name("name")),
            _ => None,
        })
        .map(|n| source(&n, text))
        .collect()
}

/// The simple names of a declaration's annotations.
fn annotation_names<'a>(declaration: &Node, text: &'a str) -> Vec<&'a str> {
    let Some(modifiers) = modifiers(declaration) else { return Vec::new() };
    named_children(&modifiers)
        .iter()
        .filter(|m| matches!(m.kind(), "annotation" | "marker_annotation"))
        .filter_map(|a| a.child_by_field_name("name"))
        .map(|n| {
            let name = source(&n, text);
            name.rsplit('.').next().unwrap_or(name)
        })
        .collect()
}

/// Whether a `return` statement is in the constructor body, outside lambdas and nested classes.
fn contains_return(body: &Node) -> bool {
    let mut found = false;
    crate::lint::syntax::for_each_descendant(body, crate::lint::syntax::NESTED_CODE, |n| {
        found |= n.kind() == "return_statement";
    });
    found
}
