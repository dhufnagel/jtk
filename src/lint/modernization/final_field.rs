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

use crate::ast::{ClassDecl, ClassKind, K, MethodDecl, Tree, VarDecl};
use crate::lint::tree::{Context, has_modifier, is_type_child};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check<'t>(unit: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let mut analysis = Analysis {
        context,
        uses_lombok: context.text.contains("lombok"),
        types: Vec::new(),
        constructors: Vec::new(),
        writes: HashMap::new(),
        reads: HashMap::new(),
        field_names: HashSet::new(),
    };
    analysis.walk(unit, false, &mut Vec::new());
    analysis.report(diagnostics);
}

struct Analysis<'c, 't> {
    context: &'c Context<'t>,
    uses_lombok: bool,
    types: Vec<Type<'t>>,
    constructors: Vec<Constructor>,
    /// Assignments of fields, by field name.
    writes: HashMap<&'t str, Vec<Access>>,
    /// Reads of fields in constructors and initializers, by field name.
    reads: HashMap<&'t str, Vec<Access>>,
    /// The names of the fields of the types seen so far: a simple name can only refer to one of these.
    field_names: HashSet<&'t str>,
}

struct Type<'t> {
    name: Option<&'t str>,
    effectively_private: bool,
    fields: Vec<Field<'t>>,
    field_names: Vec<&'t str>,
}

struct Field<'t> {
    variables: &'t [VarDecl],
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
enum Place {
    /// In a constructor (its index); whether as a statement of its body.
    Constructor(usize, bool),
    /// In a field's value or an initializer block of the type.
    Initializer(usize),
    Elsewhere,
}

#[derive(Clone, Copy)]
struct Access {
    target: Target,
    place: Place,
    offset: usize,
}

/// A tree on the walk's stack (or a `case` or `catch` clause, without a tree), with the names it declares for its
/// later descendants.
struct Frame<'t> {
    tree: Option<&'t Tree>,
    kind: FrameKind,
    locals: Vec<&'t str>,
}

impl<'t> Frame<'t> {
    fn new(tree: Option<&'t Tree>, kind: FrameKind) -> Frame<'t> {
        Frame { tree, kind, locals: Vec::new() }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FrameKind {
    /// A type body: the type's index.
    Type(usize),
    Constructor(usize),
    Lambda,
    /// An initializer block or a field's value: the type's index.
    Initializer(usize),
    Other,
}

const LOMBOK_CLASS_ANNOTATIONS: &[&str] = &["Builder", "Data", "Setter", "Value"];
const INVALIDATING_FIELD_ANNOTATIONS: &[&str] = &["Setter", "Option", "Parameters"];

impl<'t> Analysis<'_, 't> {
    /// Walks the tree, whose enclosing trees are on `frames`; `in_type` if it is (in) a type.
    fn walk(&mut self, tree: &'t Tree, in_type: bool, frames: &mut Vec<Frame<'t>>) {
        let mut frame = Frame::new(Some(tree), FrameKind::Other);
        match &tree.kind {
            K::Ident(name) => {
                if !in_type {
                    self.identifier(tree, name, frames);
                }
                return;
            }
            K::MemberSelect { expr, name } if !in_type => self.member_select(tree, expr, name, frames),
            K::Class(class) => return self.class(tree, class, frames),
            K::NewClass { outer, type_args, ident, args, body: Some(body) } => {
                frames.push(frame);
                for child in outer.as_deref().into_iter().chain(type_args).chain(args) {
                    self.walk(child, false, frames);
                }
                self.walk(ident, true, frames);
                let index = self.add_type(body, None, true, &[], ClassKind::Class);
                self.members(body, index, ClassKind::Class, frames);
                frames.pop();
                return;
            }
            K::Method(method) => {
                if method.ret.is_none()
                    && let Some(type_index) = innermost_type(frames)
                {
                    let constructor = constructor(method, type_index);
                    self.constructors.push(constructor);
                    frame.kind = FrameKind::Constructor(self.constructors.len() - 1);
                }
                frame.locals = method.params.iter().map(|p| p.name.as_str()).collect();
            }
            K::Lambda { params, .. } => {
                frame.kind = FrameKind::Lambda;
                frame.locals = params.iter().map(|p| p.name.as_str()).collect();
            }
            K::ForEach { var, .. } => frame.locals.push(&var.name),
            // A local variable is visible after its declaration in the enclosing block, `case` or `for` statement.
            K::Vars(vars) => {
                frames.push(frame);
                self.variables(vars, None, frames);
                frames.pop();
                return;
            }
            // A resource is visible in the rest of the `try` statement.
            K::Var(var) => {
                if let Some(scope) = frames.last_mut() {
                    scope.locals.push(&var.name);
                }
            }
            K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
                frames.push(frame);
                self.walk(selector, false, frames);
                for case in cases {
                    frames.push(Frame::new(None, FrameKind::Other));
                    let parts =
                        case.labels.iter().chain(case.guard.as_deref()).chain(&case.stmts).chain(case.body.as_deref());
                    for child in parts {
                        self.walk(child, false, frames);
                    }
                    frames.pop();
                }
                frames.pop();
                return;
            }
            K::Try { resources, block, catches, finally } => {
                frames.push(frame);
                for child in resources.iter().chain([&**block]) {
                    self.walk(child, false, frames);
                }
                for catch in catches {
                    frames.push(Frame { tree: None, kind: FrameKind::Other, locals: vec![&catch.param.name] });
                    self.variables(std::slice::from_ref(&catch.param), Some(None), frames);
                    self.walk(&catch.block, false, frames);
                    frames.pop();
                }
                if let Some(finally) = finally {
                    self.walk(finally, false, frames);
                }
                frames.pop();
                return;
            }
            _ => {}
        }
        frames.push(frame);
        self.children(tree, in_type, frames);
        frames.pop();
    }

    fn children(&mut self, tree: &'t Tree, in_type: bool, frames: &mut Vec<Frame<'t>>) {
        tree.for_each_child(&mut |child| {
            let child_in_type = !matches!(child.kind, K::Annotation { .. }) && (in_type || is_type_child(tree, child));
            self.walk(child, child_in_type, frames);
        });
    }

    /// Walks the variables of a declaration, whose frame is the last; `initializer` is the type whose fields they are
    /// if their values are initializers, `None` for local variables (which are visible in the frame before).
    fn variables(&mut self, vars: &'t [VarDecl], initializer: Option<Option<usize>>, frames: &mut Vec<Frame<'t>>) {
        for annotation in vars.first().map_or(&[][..], |v| &v.mods.annotations) {
            self.walk(annotation, false, frames);
        }
        for var in vars {
            if initializer.is_none()
                && let [.., scope, _] = &mut frames[..]
            {
                scope.locals.push(&var.name);
            }
            if let Some(ty) = &var.ty {
                self.walk(ty, true, frames);
            }
            let Some(init) = &var.init else { continue };
            match initializer.flatten() {
                Some(index) => {
                    frames.push(Frame::new(None, FrameKind::Initializer(index)));
                    self.walk(init, false, frames);
                    frames.pop();
                }
                None => self.walk(init, false, frames),
            }
        }
    }

    /// Walks a class declaration and its body.
    fn class(&mut self, tree: &'t Tree, class: &'t ClassDecl, frames: &mut Vec<Frame<'t>>) {
        let text = self.context.text;
        let tokens = self.context.tokens();
        // A member of a type or of the compilation unit, or a local class.
        let is_member = frames.last().is_some_and(|f| {
            matches!(f.kind, FrameKind::Type(_)) || f.tree.is_some_and(|t| matches!(t.kind, K::CompilationUnit { .. }))
        });
        let enclosing_private = innermost_type(frames).is_some_and(|i| self.types[i].effectively_private);
        let effectively_private = enclosing_private || !is_member || has_modifier(&class.mods, text, tokens, "private");
        frames.push(Frame::new(Some(tree), FrameKind::Other));
        for annotation in &class.mods.annotations {
            self.walk(annotation, false, frames);
        }
        for component in &class.record_components {
            self.variables(std::slice::from_ref(component), Some(None), frames);
        }
        let types =
            class.type_params.iter().chain(class.extends.as_deref()).chain(&class.implements).chain(&class.permits);
        for ty in types {
            self.walk(ty, true, frames);
        }
        let index =
            self.add_type(&class.members, Some(&class.name), effectively_private, &class.mods.annotations, class.kind);
        frames.push(Frame::new(None, FrameKind::Type(index)));
        for constant in &class.enum_constants {
            frames.push(Frame::new(None, FrameKind::Other));
            for child in constant.mods.annotations.iter().chain(&constant.args) {
                self.walk(child, false, frames);
            }
            if let Some(body) = &constant.body {
                let constant_index = self.add_type(body, None, true, &constant.mods.annotations, ClassKind::Class);
                self.members(body, constant_index, ClassKind::Class, frames);
            }
            frames.pop();
        }
        self.walk_members(&class.members, index, class.kind, frames);
        frames.pop();
        frames.pop();
    }

    /// Walks the members of the body of the type, in a frame of its own.
    fn members(&mut self, members: &'t [Tree], index: usize, kind: ClassKind, frames: &mut Vec<Frame<'t>>) {
        frames.push(Frame::new(None, FrameKind::Type(index)));
        self.walk_members(members, index, kind, frames);
        frames.pop();
    }

    fn walk_members(&mut self, members: &'t [Tree], index: usize, kind: ClassKind, frames: &mut Vec<Frame<'t>>) {
        let is_interface = matches!(kind, ClassKind::Interface | ClassKind::Annotation);
        for member in members {
            match &member.kind {
                // Fields, or constants of an interface.
                K::Vars(vars) => {
                    let text = self.context.text;
                    let is_static = has_modifier(&vars[0].mods, text, self.context.tokens(), "static");
                    let initializer = (!is_interface && !is_static).then_some(index);
                    frames.push(Frame::new(Some(member), FrameKind::Other));
                    self.variables(vars, Some(initializer), frames);
                    frames.pop();
                }
                K::Block { is_static: false, .. } => {
                    frames.push(Frame::new(Some(member), FrameKind::Initializer(index)));
                    self.children(member, false, frames);
                    frames.pop();
                }
                _ => self.walk(member, false, frames),
            }
        }
    }

    /// Adds a type with its fields; returns its index.
    fn add_type(
        &mut self,
        members: &'t [Tree],
        name: Option<&'t str>,
        effectively_private: bool,
        annotations: &[Tree],
        kind: ClassKind,
    ) -> usize {
        let text = self.context.text;
        let tokens = self.context.tokens();
        let lombok_class =
            self.uses_lombok && annotation_names(annotations, text).any(|a| LOMBOK_CLASS_ANNOTATIONS.contains(&a));
        let is_interface = matches!(kind, ClassKind::Interface | ClassKind::Annotation);
        let mut fields = Vec::new();
        let mut field_names = Vec::new();
        // The constants of an interface are not fields here.
        for member in members.iter().filter(|_| !is_interface) {
            let K::Vars(variables) = &member.kind else { continue };
            field_names.extend(variables.iter().map(|v| v.name.as_str()));
            let mods = &variables[0].mods;
            let modifier = |keyword| has_modifier(mods, text, tokens, keyword);
            let candidate = kind != ClassKind::Record
                && !lombok_class
                && !modifier("static")
                && !modifier("final")
                && !modifier("volatile")
                && (effectively_private || modifier("private"))
                && !annotation_names(&mods.annotations, text).any(|a| INVALIDATING_FIELD_ANNOTATIONS.contains(&a));
            fields.push(Field { variables, candidate });
        }
        self.field_names.extend(field_names.iter().copied());
        self.types.push(Type { name, effectively_private, fields, field_names });
        self.types.len() - 1
    }

    /// Records an identifier that may read or assign a field.
    fn identifier(&mut self, identifier: &'t Tree, name: &'t str, frames: &[Frame<'t>]) {
        if matches!(name, "this" | "super") || !self.field_names.contains(name) {
            return;
        }
        let is = |tree: &Tree| std::ptr::eq(tree, identifier);
        let (Some(parent), grandparent) =
            (frames.last().and_then(|f| f.tree), frames.len().checked_sub(2).and_then(|i| frames[i].tree))
        else {
            return;
        };
        let is_name = match &parent.kind {
            // The name of a method.
            K::MethodInvocation { select, .. } => !is(select),
            // `a` of `a.b`, but not of `a.m()`.
            K::MemberSelect { .. } => grandparent.is_some_and(|g| is_method_select(g, parent)),
            // The element of an annotation.
            K::Assign { lhs, .. } if is(lhs) => !grandparent.is_some_and(|g| matches!(g.kind, K::Annotation { .. })),
            _ => true,
        };
        if is_name && let Some(target) = self.resolve(name, frames) {
            self.access(name, identifier, target, frames);
        }
    }

    /// Records a field access `object.name` (not a method call `object.name()`).
    fn member_select(&mut self, select: &'t Tree, object: &'t Tree, name: &'t str, frames: &[Frame<'t>]) {
        if matches!(name, "class" | "this" | "super") {
            return;
        }
        if frames.last().and_then(|f| f.tree).is_some_and(|parent| is_method_select(parent, select)) {
            return;
        }
        let field_of = |index: usize| self.types[index].field_names.contains(&name).then_some(Target::Type(index));
        let target = match &object.kind {
            K::Ident(this) if this == "this" => innermost_type(frames).and_then(field_of),
            // `Outer.this.x`
            K::MemberSelect { expr: class, name: this } if this == "this" => {
                let class = self.context.source(class);
                let mut types = frames.iter().rev().filter_map(|f| f.kind.type_index());
                types.find(|&i| self.types[i].name == Some(class)).and_then(field_of)
            }
            _ => Some(Target::Any),
        };
        if let Some(target) = target {
            self.access(name, select, target, frames);
        }
    }

    /// Records the read or assignment of a field by the expression, a child of the last frame's tree.
    fn access(&mut self, name: &'t str, expression: &Tree, target: Target, frames: &[Frame<'t>]) {
        let is = |tree: &Tree| std::ptr::eq(tree, expression);
        let Some(holder) = frames.last().and_then(|f| f.tree) else { return };
        let (is_write, is_read) = match &holder.kind {
            K::Assign { op, lhs, .. } if is(lhs) => (true, op != "="),
            K::Unary { op, expr, .. } if (op == "++" || op == "--") && is(expr) => (true, true),
            _ => (false, true),
        };
        let offset = expression.start;
        if is_write {
            // A statement of a constructor's body (not of a compact constructor's, as tree-sitter has).
            let in_body = matches!(frames, [.., constructor, body, statement, _]
                if matches!(statement.tree.map(|t| &t.kind), Some(K::ExprStmt(_)))
                    && matches!(body.tree.map(|t| &t.kind), Some(K::Block { .. }))
                    && matches!(constructor.tree.map(|t| &t.kind), Some(K::Method(m)) if !m.compact_ctor)
                    && matches!(constructor.kind, FrameKind::Constructor(_)));
            let place = match frames.iter().rev().find(|f| f.kind != FrameKind::Other).map(|f| f.kind) {
                Some(FrameKind::Constructor(index)) => Place::Constructor(index, in_body),
                _ => Place::Elsewhere,
            };
            self.writes.entry(name).or_default().push(Access { target, place, offset });
        }
        if is_read {
            // Lambdas run later, but cannot read a final field before it is assigned either.
            let place = match frames.iter().rev().find(|f| !matches!(f.kind, FrameKind::Other | FrameKind::Lambda)) {
                Some(Frame { kind: FrameKind::Constructor(index), .. }) => Place::Constructor(*index, false),
                Some(Frame { kind: FrameKind::Initializer(index), .. }) => Place::Initializer(*index),
                _ => return,
            };
            self.reads.entry(name).or_default().push(Access { target, place, offset });
        }
    }

    /// The field a simple name refers to: the innermost local variable, parameter or field of the name.
    fn resolve(&self, name: &str, frames: &[Frame<'t>]) -> Option<Target> {
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
                let reported: Vec<(&VarDecl, bool)> = field
                    .variables
                    .iter()
                    .filter_map(|var| {
                        self.is_immutable(type_index, &var.name, var.init.is_some()).map(|fixable| (var, fixable))
                    })
                    .collect();
                let fixable = reported.len() == field.variables.len() && reported.iter().all(|r| r.1);
                for (var, _) in &reported {
                    let message = format!("Private field '{}' could be made final", var.name);
                    let diagnostic = Diagnostic::new(Rule::FinalField, var.name_pos, message);
                    let insertion = field.variables[0].ty.as_deref().map(|t| t.start);
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
            match write.place {
                Place::Constructor(index, true)
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
            reads.iter().filter(refers).any(|read| match read.place {
                Place::Initializer(index) => index == type_index,
                Place::Constructor(index, _) => assignments.get(&index).is_some_and(|&offset| read.offset < offset),
                Place::Elsewhere => false,
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

/// Whether `select` is the method selected by the call `tree`: `a.m` of `a.m()`.
fn is_method_select(tree: &Tree, select: &Tree) -> bool {
    matches!(&tree.kind, K::MethodInvocation { select: method, .. } if std::ptr::eq(&**method, select))
}

/// The index of the innermost type around the frames.
fn innermost_type(frames: &[Frame]) -> Option<usize> {
    frames.iter().rev().find_map(|f| f.kind.type_index())
}

/// A constructor of the type: whether it starts with `this(...)` and whether it has a `return`.
fn constructor(method: &MethodDecl, type_index: usize) -> Constructor {
    let statements = match method.body.as_deref().map(|b| &b.kind) {
        Some(K::Block { stmts, .. }) => &stmts[..],
        _ => &[],
    };
    let delegates = statements.first().is_some_and(|s| {
        matches!(&s.kind, K::ExprStmt(call) if matches!(&call.kind,
            K::MethodInvocation { select, .. } if matches!(&select.kind, K::Ident(name) if name == "this")))
    });
    let has_return = statements.iter().any(contains_return);
    Constructor { type_index, delegates, has_return }
}

/// The simple names of annotations.
fn annotation_names<'a>(annotations: &'a [Tree], text: &'a str) -> impl Iterator<Item = &'a str> {
    annotations.iter().filter_map(move |a| match &a.kind {
        K::Annotation { ty, .. } => {
            let name = &text[ty.start..ty.end];
            Some(name.rsplit('.').next().unwrap_or(name))
        }
        _ => None,
    })
}

/// Whether a `return` statement is in the code, outside lambdas and the bodies of classes, records, anonymous
/// classes and enum constants (tree-sitter's class bodies).
fn contains_return(tree: &Tree) -> bool {
    match &tree.kind {
        K::Return(_) => true,
        K::Lambda { .. } | K::NewClass { .. } => false,
        K::Class(class) if matches!(class.kind, ClassKind::Class | ClassKind::Record) => false,
        K::Class(class) => class.members.iter().any(contains_return),
        _ => {
            let mut found = false;
            tree.for_each_child(&mut |child| found = found || contains_return(child));
            found
        }
    }
}
