//! A javac-shaped AST, converted from the tree-sitter concrete syntax tree.
//!
//! The visitor is a port of `JavaInputAstVisitor`, which walks javac trees. Converting tree-sitter's CST into a
//! similar shape here keeps the visitor close to the original and confines grammar differences to this module.

use tree_sitter::Node;

pub type B = Box<Tree>;

#[derive(Debug, Clone)]
pub struct Tree {
    pub start: usize,
    pub end: usize,
    pub kind: K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    Int,
    Long,
    Float,
    Double,
    Char,
    String,
    Boolean,
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
}

#[derive(Debug, Clone)]
pub struct Mods {
    pub start: usize,
    pub annotations: Vec<Tree>,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub start: usize,
    pub end: usize,
    pub mods: Mods,
    /// `None` for inferred lambda parameters and `var`.
    pub ty: Option<B>,
    pub name: String,
    pub init: Option<B>,
    pub is_record_component: bool,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub kind: ClassKind,
    pub mods: Mods,
    pub name: String,
    pub type_params: Vec<Tree>,
    pub extends: Option<B>,
    pub implements: Vec<Tree>,
    pub permits: Vec<Tree>,
    pub record_components: Vec<VarDecl>,
    pub enum_constants: Vec<EnumConstant>,
    pub members: Vec<Tree>,
}

#[derive(Debug, Clone)]
pub struct EnumConstant {
    pub mods: Mods,
    pub name: String,
    pub args: Vec<Tree>,
    pub body: Option<Vec<Tree>>,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub mods: Mods,
    /// Annotations appearing after the type parameters (they annotate the return type).
    pub return_type_annotations: Vec<Tree>,
    pub type_params: Vec<Tree>,
    /// `None` for constructors.
    pub ret: Option<B>,
    pub name: String,
    pub receiver: Option<Box<VarDecl>>,
    pub params: Vec<VarDecl>,
    pub throws: Vec<Tree>,
    pub body: Option<B>,
    pub default_value: Option<B>,
    pub compact_ctor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Statement,
    Rule,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub start: usize,
    pub labels: Vec<Tree>,
    pub is_default: bool,
    pub guard: Option<B>,
    pub kind: CaseKind,
    pub stmts: Vec<Tree>,
    pub body: Option<B>,
}

#[derive(Debug, Clone)]
pub struct Catch {
    pub start: usize,
    pub param: VarDecl,
    /// Union type alternatives, if more than one.
    pub union: Vec<Tree>,
    pub block: B,
}

#[derive(Debug, Clone)]
pub enum K {
    CompilationUnit {
        package: Option<(Vec<Tree>, B)>,
        imports: Vec<Tree>,
        types: Vec<Tree>,
        module: Option<B>,
    },
    Import {
        is_static: bool,
        name: B,
    },
    Class(Box<ClassDecl>),
    Method(Box<MethodDecl>),
    /// One or more variables declared together (fields and locals).
    Vars(Vec<VarDecl>),
    Block {
        is_static: bool,
        stmts: Vec<Tree>,
    },
    Empty,
    ExprStmt(B),
    If {
        cond: B,
        then: B,
        else_: Option<B>,
    },
    While {
        cond: B,
        body: B,
    },
    DoWhile {
        body: B,
        cond: B,
    },
    For {
        init: Vec<Tree>,
        cond: Option<B>,
        update: Vec<Tree>,
        body: B,
    },
    ForEach {
        var: Box<VarDecl>,
        expr: B,
        body: B,
    },
    Labeled {
        label: String,
        body: B,
    },
    Switch {
        selector: B,
        cases: Vec<Case>,
    },
    Try {
        resources: Vec<Tree>,
        block: B,
        catches: Vec<Catch>,
        finally: Option<B>,
    },
    Throw(B),
    Return(Option<B>),
    Break(Option<String>),
    Continue(Option<String>),
    Synchronized {
        lock: B,
        block: B,
    },
    Assert {
        cond: B,
        detail: Option<B>,
    },
    Yield(B),
    Var(Box<VarDecl>),
    // Expressions
    Ident(String),
    MemberSelect {
        expr: B,
        name: String,
    },
    MethodInvocation {
        select: B,
        type_args: Vec<Tree>,
        args: Vec<Tree>,
    },
    ArrayAccess {
        expr: B,
        index: B,
    },
    Literal(LitKind),
    Binary {
        op: String,
        lhs: B,
        rhs: B,
    },
    Unary {
        op: String,
        postfix: bool,
        expr: B,
    },
    Assign {
        op: String,
        lhs: B,
        rhs: B,
    },
    Conditional {
        cond: B,
        then: B,
        else_: B,
    },
    InstanceOf {
        expr: B,
        target: B,
    },
    Cast {
        ty: B,
        expr: B,
    },
    Parens(B),
    NewClass {
        outer: Option<B>,
        type_args: Vec<Tree>,
        ident: B,
        args: Vec<Tree>,
        body: Option<Vec<Tree>>,
    },
    NewArray {
        elem: Option<B>,
        dims: Vec<Tree>,
        dim_annotations: Vec<Vec<Tree>>,
        init: Option<Vec<Tree>>,
    },
    Lambda {
        params: Vec<VarDecl>,
        body: B,
    },
    MemberRef {
        qual: B,
        type_args: Vec<Tree>,
        name: Option<String>,
    },
    SwitchExpr {
        selector: B,
        cases: Vec<Case>,
    },
    // Types
    Primitive(String),
    ArrayType {
        elem: B,
        dims: Vec<Vec<Tree>>,
    },
    Parameterized {
        base: B,
        args: Vec<Tree>,
    },
    Wildcard {
        bound: Option<(bool, B)>,
    },
    Annotated {
        annotations: Vec<Tree>,
        base: B,
    },
    Union(Vec<Tree>),
    Intersection(Vec<Tree>),
    Annotation {
        ty: B,
        args: Vec<Tree>,
    },
    TypeParameter {
        annotations: Vec<Tree>,
        name: String,
        bounds: Vec<Tree>,
    },
    // Patterns and case labels
    BindingPattern {
        var: Box<VarDecl>,
    },
    RecordPattern {
        ty: B,
        nested: Vec<Tree>,
    },
    /// The unnamed pattern `_` (Java 22).
    AnyPattern,
    DefaultCaseLabel,
    // Modules
    Module {
        annotations: Vec<Tree>,
        open: bool,
        name: B,
        directives: Vec<Tree>,
    },
    Directive {
        keyword: &'static str,
        name: B,
        separator: &'static str,
        items: Option<Vec<Tree>>,
    },
    Requires {
        name: B,
    },
    Uses {
        name: B,
    },
}

impl Tree {
    pub fn new(start: usize, end: usize, kind: K) -> Tree {
        Tree { start, end, kind }
    }

    /// Calls `f` on each direct subtree, in source order. Annotations in modifiers, the types and initializers of
    /// variables and the parts of cases, catches, classes and methods are subtrees; names stored as strings are not.
    pub fn for_each_child<'t>(&'t self, f: &mut impl FnMut(&'t Tree)) {
        fn var<'t>(v: &'t VarDecl, f: &mut impl FnMut(&'t Tree)) {
            v.mods.annotations.iter().for_each(&mut *f);
            v.ty.iter().for_each(|t| f(t));
            v.init.iter().for_each(|t| f(t));
        }
        fn case<'t>(c: &'t Case, f: &mut impl FnMut(&'t Tree)) {
            c.labels.iter().for_each(&mut *f);
            c.guard.iter().for_each(|t| f(t));
            c.stmts.iter().for_each(&mut *f);
            c.body.iter().for_each(|t| f(t));
        }
        match &self.kind {
            K::CompilationUnit { package, imports, types, module } => {
                if let Some((annotations, name)) = package {
                    annotations.iter().for_each(&mut *f);
                    f(name);
                }
                imports.iter().for_each(&mut *f);
                types.iter().for_each(&mut *f);
                module.iter().for_each(|t| f(t));
            }
            K::Import { name, .. } => f(name),
            K::Class(c) => {
                c.mods.annotations.iter().for_each(&mut *f);
                c.type_params.iter().for_each(&mut *f);
                c.record_components.iter().for_each(|v| var(v, f));
                c.extends.iter().for_each(|t| f(t));
                c.implements.iter().for_each(&mut *f);
                c.permits.iter().for_each(&mut *f);
                for e in &c.enum_constants {
                    e.mods.annotations.iter().for_each(&mut *f);
                    e.args.iter().for_each(&mut *f);
                    e.body.iter().flatten().for_each(&mut *f);
                }
                c.members.iter().for_each(&mut *f);
            }
            K::Method(m) => {
                m.mods.annotations.iter().for_each(&mut *f);
                m.type_params.iter().for_each(&mut *f);
                m.return_type_annotations.iter().for_each(&mut *f);
                m.ret.iter().for_each(|t| f(t));
                m.receiver.iter().for_each(|v| var(v, f));
                m.params.iter().for_each(|v| var(v, f));
                m.throws.iter().for_each(&mut *f);
                m.body.iter().for_each(|t| f(t));
                m.default_value.iter().for_each(|t| f(t));
            }
            K::Vars(vars) => vars.iter().for_each(|v| var(v, f)),
            K::Var(v) => var(v, f),
            K::Block { stmts, .. } => stmts.iter().for_each(&mut *f),
            K::Empty
            | K::Ident(_)
            | K::Literal(_)
            | K::Primitive(_)
            | K::AnyPattern
            | K::DefaultCaseLabel
            | K::Break(_)
            | K::Continue(_) => {}
            K::ExprStmt(e) | K::Throw(e) | K::Yield(e) | K::Parens(e) => f(e),
            K::If { cond, then, else_ } => {
                f(cond);
                f(then);
                else_.iter().for_each(|t| f(t));
            }
            K::While { cond, body } => {
                f(cond);
                f(body);
            }
            K::DoWhile { body, cond } => {
                f(body);
                f(cond);
            }
            K::For { init, cond, update, body } => {
                init.iter().for_each(&mut *f);
                cond.iter().for_each(|t| f(t));
                update.iter().for_each(&mut *f);
                f(body);
            }
            K::ForEach { var: v, expr, body } => {
                var(v, f);
                f(expr);
                f(body);
            }
            K::Labeled { body, .. } => f(body),
            K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
                f(selector);
                cases.iter().for_each(|c| case(c, f));
            }
            K::Try { resources, block, catches, finally } => {
                resources.iter().for_each(&mut *f);
                f(block);
                for c in catches {
                    var(&c.param, f);
                    f(&c.block);
                }
                finally.iter().for_each(|t| f(t));
            }
            K::Return(e) => e.iter().for_each(|t| f(t)),
            K::Synchronized { lock, block } => {
                f(lock);
                f(block);
            }
            K::Assert { cond, detail } => {
                f(cond);
                detail.iter().for_each(|t| f(t));
            }
            K::MemberSelect { expr, .. } => f(expr),
            K::MethodInvocation { select, type_args, args } => {
                f(select);
                type_args.iter().for_each(&mut *f);
                args.iter().for_each(&mut *f);
            }
            K::ArrayAccess { expr, index } => {
                f(expr);
                f(index);
            }
            K::Binary { lhs, rhs, .. } | K::Assign { lhs, rhs, .. } => {
                f(lhs);
                f(rhs);
            }
            K::Unary { expr, .. } => f(expr),
            K::Conditional { cond, then, else_ } => {
                f(cond);
                f(then);
                f(else_);
            }
            K::InstanceOf { expr, target } => {
                f(expr);
                f(target);
            }
            K::Cast { ty, expr } => {
                f(ty);
                f(expr);
            }
            K::NewClass { outer, type_args, ident, args, body } => {
                outer.iter().for_each(|t| f(t));
                type_args.iter().for_each(&mut *f);
                f(ident);
                args.iter().for_each(&mut *f);
                body.iter().flatten().for_each(&mut *f);
            }
            K::NewArray { elem, dims, dim_annotations, init } => {
                elem.iter().for_each(|t| f(t));
                dims.iter().for_each(&mut *f);
                dim_annotations.iter().flatten().for_each(&mut *f);
                init.iter().flatten().for_each(&mut *f);
            }
            K::Lambda { params, body } => {
                params.iter().for_each(|v| var(v, f));
                f(body);
            }
            K::MemberRef { qual, type_args, .. } => {
                f(qual);
                type_args.iter().for_each(&mut *f);
            }
            K::ArrayType { elem, dims } => {
                f(elem);
                dims.iter().flatten().for_each(&mut *f);
            }
            K::Parameterized { base, args } => {
                f(base);
                args.iter().for_each(&mut *f);
            }
            K::Wildcard { bound } => bound.iter().for_each(|(_, t)| f(t)),
            K::Annotated { annotations, base } => {
                annotations.iter().for_each(&mut *f);
                f(base);
            }
            K::Union(types) | K::Intersection(types) => types.iter().for_each(&mut *f),
            K::Annotation { ty, args } => {
                f(ty);
                args.iter().for_each(&mut *f);
            }
            K::TypeParameter { annotations, bounds, .. } => {
                annotations.iter().for_each(&mut *f);
                bounds.iter().for_each(&mut *f);
            }
            K::BindingPattern { var: v } => var(v, f),
            K::RecordPattern { ty, nested } => {
                f(ty);
                nested.iter().for_each(&mut *f);
            }
            K::Module { annotations, name, directives, .. } => {
                annotations.iter().for_each(&mut *f);
                f(name);
                directives.iter().for_each(&mut *f);
            }
            K::Directive { name, items, .. } => {
                f(name);
                items.iter().flatten().for_each(&mut *f);
            }
            K::Requires { name } | K::Uses { name } => f(name),
        }
    }

    pub fn is_expression(&self) -> bool {
        matches!(
            self.kind,
            K::Ident(_)
                | K::MemberSelect { .. }
                | K::MethodInvocation { .. }
                | K::ArrayAccess { .. }
                | K::Literal(_)
                | K::Binary { .. }
                | K::Unary { .. }
                | K::Assign { .. }
                | K::Conditional { .. }
                | K::InstanceOf { .. }
                | K::Cast { .. }
                | K::Parens(_)
                | K::NewClass { .. }
                | K::NewArray { .. }
                | K::Lambda { .. }
                | K::MemberRef { .. }
                | K::SwitchExpr { .. }
                | K::Annotated { .. }
                | K::Annotation { .. }
        )
    }

    /// The javac `Tree.Kind`, as used by heuristics comparing node kinds.
    pub fn javac_kind(&self) -> String {
        match &self.kind {
            K::Literal(l) => format!("{l:?}_LITERAL"),
            K::Binary { op, .. } => format!("BINARY{op}"),
            K::Unary { op, postfix, .. } => format!("UNARY{op}{postfix}"),
            K::Assign { op, .. } => format!("ASSIGN{op}"),
            other => {
                let s = format!("{other:?}");
                s.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string()
            }
        }
    }
}

/// Converts a tree-sitter tree into the javac-shaped AST.
pub struct Converter<'s> {
    pub src: &'s str,
}

fn is_comment(n: &Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

fn named_children<'t>(n: &Node<'t>) -> Vec<Node<'t>> {
    children_without_comments(n, true)
}

fn all_children<'t>(n: &Node<'t>) -> Vec<Node<'t>> {
    children_without_comments(n, false)
}

/// Looking a child up by index walks the preceding children, but needs no cursor (an allocation): used for nodes
/// with few children.
const FEW_CHILDREN: usize = 8;

fn children_without_comments<'t>(n: &Node<'t>, named: bool) -> Vec<Node<'t>> {
    let count = if named { n.named_child_count() } else { n.child_count() as usize };
    let mut out = Vec::with_capacity(count);
    if count <= FEW_CHILDREN {
        for i in 0..count {
            let c = if named { n.named_child(i as u32) } else { n.child(i as u32) };
            out.extend(c.filter(|c| !is_comment(c)));
        }
    } else {
        let mut cursor = n.walk();
        if named {
            out.extend(n.named_children(&mut cursor).filter(|c| !is_comment(c)));
        } else {
            out.extend(n.children(&mut cursor).filter(|c| !is_comment(c)));
        }
    }
    out
}

/// Start of the first non-comment token of the node.
fn start_of(n: &Node) -> usize {
    let mut node = *n;
    loop {
        // Most first children are not comments; checking that needs no cursor.
        let first = match node.child(0) {
            None => return node.start_byte(),
            Some(c) if !is_comment(&c) => Some(c),
            Some(_) => {
                let mut cursor = node.walk();
                node.children(&mut cursor).find(|c| !is_comment(c))
            }
        };
        match first {
            Some(c) => node = c,
            None => return node.start_byte(),
        }
    }
}

pub type ConvertError = crate::formatter::FormatError;

type R<T> = Result<T, ConvertError>;

fn err<T>(n: &Node, what: &str) -> R<T> {
    Err(ConvertError::at(
        n.start_position().row + 1,
        n.start_position().column + 1,
        format!("unsupported {what} '{}'", n.kind()),
    ))
}

/// `Node::child_by_field_name` looks the name up in the grammar on every call; field ids are cached instead.
fn child_by_field<'t>(n: &Node<'t>, name: &'static str) -> Option<Node<'t>> {
    thread_local! {
        static IDS: std::cell::RefCell<std::collections::HashMap<&'static str, Option<std::num::NonZeroU16>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    let id = IDS.with_borrow_mut(|ids| {
        *ids.entry(name)
            .or_insert_with(|| tree_sitter::Language::new(tree_sitter_java_orchard::LANGUAGE).field_id_for_name(name))
    })?;
    n.child_by_field_id(id.get())
}

fn field<'t>(n: &Node<'t>, name: &'static str) -> R<Node<'t>> {
    child_by_field(n, name).ok_or_else(|| ConvertError::new(format!("missing field {name} in {}", n.kind())))
}

fn is_annotation(n: &Node) -> bool {
    matches!(n.kind(), "annotation" | "marker_annotation")
}

impl<'s> Converter<'s> {
    fn text(&self, n: &Node) -> &'s str {
        &self.src[n.start_byte()..n.end_byte()]
    }

    fn mk(&self, n: &Node, kind: K) -> Tree {
        Tree::new(start_of(n), n.end_byte(), kind)
    }

    pub fn compilation_unit(&self, root: &Node) -> R<Tree> {
        if root.has_error() {
            let error = first_error(root);
            return Err(ConvertError::at_offset(self.src, error.start_byte(), "syntax error"));
        }
        let mut package = None;
        let mut imports = Vec::new();
        let mut types = Vec::new();
        let mut module = None;
        for c in named_children(root) {
            match c.kind() {
                "package_declaration" => {
                    let mut annotations = Vec::new();
                    let mut name = None;
                    for p in named_children(&c) {
                        if is_annotation(&p) {
                            annotations.push(self.annotation(&p)?);
                        } else {
                            name = Some(Box::new(self.name(&p)?));
                        }
                    }
                    package = Some((annotations, name.ok_or_else(|| ConvertError::new("package"))?));
                }
                "import_declaration" => {
                    let is_static = all_children(&c).iter().any(|x| x.kind() == "static");
                    let parts = named_children(&c);
                    let mut name = self.name(&parts[0])?;
                    if let Some(last) = parts.get(1)
                        && last.kind() == "asterisk"
                    {
                        name = Tree::new(
                            name.start,
                            last.end_byte(),
                            K::MemberSelect { expr: Box::new(name), name: "*".into() },
                        );
                    }
                    imports.push(self.mk(&c, K::Import { is_static, name: Box::new(name) }));
                }
                "module_declaration" => module = Some(Box::new(self.module(&c)?)),
                ";" => {}
                _ => types.push(self.declaration(&c)?),
            }
        }
        Ok(self.mk(root, K::CompilationUnit { package, imports, types, module }))
    }

    fn module(&self, n: &Node) -> R<Tree> {
        let mut annotations = Vec::new();
        for c in named_children(n) {
            if is_annotation(&c) {
                annotations.push(self.annotation(&c)?);
            }
        }
        let open = all_children(n).iter().any(|c| c.kind() == "open");
        let name = Box::new(self.name(&field(n, "name")?)?);
        let mut directives = Vec::new();
        for d in named_children(&field(n, "body")?) {
            let names = |f: &str| -> R<Vec<Tree>> {
                let mut cursor = d.walk();
                d.children_by_field_name(f, &mut cursor).map(|x| self.name(&x)).collect()
            };
            let kind = match d.kind() {
                "requires_module_directive" => K::Requires { name: Box::new(self.name(&field(&d, "module")?)?) },
                "uses_module_directive" => K::Uses { name: Box::new(self.name(&field(&d, "type")?)?) },
                "exports_module_directive" | "opens_module_directive" => {
                    let items = names("modules")?;
                    K::Directive {
                        keyword: if d.kind() == "exports_module_directive" { "exports" } else { "opens" },
                        name: Box::new(self.name(&field(&d, "package")?)?),
                        separator: "to",
                        items: if items.is_empty() { None } else { Some(items) },
                    }
                }
                "provides_module_directive" => {
                    let provided = field(&d, "provided")?;
                    let items = named_children(&d)
                        .iter()
                        .filter(|x| x.id() != provided.id())
                        .map(|x| self.name(x))
                        .collect::<R<Vec<_>>>()?;
                    K::Directive {
                        keyword: "provides",
                        name: Box::new(self.name(&field(&d, "provided")?)?),
                        separator: "with",
                        items: if items.is_empty() { None } else { Some(items) },
                    }
                }
                _ => return err(&d, "module directive"),
            };
            directives.push(self.mk(&d, kind));
        }
        Ok(self.mk(n, K::Module { annotations, open, name, directives }))
    }

    /// A (possibly qualified) name, as a chain of `MemberSelect`s.
    fn name(&self, n: &Node) -> R<Tree> {
        match n.kind() {
            "identifier" | "type_identifier" | "this" | "super" => Ok(self.mk(n, K::Ident(self.text(n).into()))),
            "scoped_identifier" => {
                let scope = self.name(&field(n, "scope")?)?;
                let name = self.text(&field(n, "name")?).to_string();
                Ok(self.mk(n, K::MemberSelect { expr: Box::new(scope), name }))
            }
            _ => err(n, "name"),
        }
    }

    fn mods(&self, n: &Node) -> R<Mods> {
        let mut annotations = Vec::new();
        let mut start = None;
        for c in named_children(n) {
            if c.kind() == "modifiers" {
                start.get_or_insert(start_of(&c));
                for a in named_children(&c) {
                    if is_annotation(&a) {
                        annotations.push(self.annotation(&a)?);
                    }
                }
            }
        }
        Ok(Mods { start: start.unwrap_or(start_of(n)), annotations })
    }

    fn annotation(&self, n: &Node) -> R<Tree> {
        let ty = Box::new(self.name(&field(n, "name")?)?);
        let mut args = Vec::new();
        if n.kind() == "annotation" {
            for a in named_children(&field(n, "arguments")?) {
                args.push(self.element_value(&a)?);
            }
        }
        Ok(self.mk(n, K::Annotation { ty, args }))
    }

    fn element_value(&self, n: &Node) -> R<Tree> {
        match n.kind() {
            "element_value_pair" => {
                let key = field(n, "key")?;
                let lhs = Box::new(self.mk(&key, K::Ident(self.text(&key).into())));
                let rhs = Box::new(self.element_value(&field(n, "value")?)?);
                Ok(self.mk(n, K::Assign { op: "=".into(), lhs, rhs }))
            }
            "element_value_array_initializer" => {
                let init = named_children(n).iter().map(|c| self.element_value(c)).collect::<R<Vec<_>>>()?;
                Ok(self.mk(n, K::NewArray { elem: None, dims: vec![], dim_annotations: vec![], init: Some(init) }))
            }
            "annotation" | "marker_annotation" => self.annotation(n),
            _ => self.expr(n),
        }
    }

    fn declaration(&self, n: &Node) -> R<Tree> {
        match n.kind() {
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "annotation_type_declaration"
            | "record_declaration" => self.class(n),
            "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => self.method(n),
            "annotation_type_element_declaration" => self.method(n),
            "field_declaration" | "constant_declaration" | "local_variable_declaration" => self.vars(n),
            "static_initializer" => {
                let block = named_children(n)[0];
                let mut b = self.block(&block)?;
                if let K::Block { is_static, .. } = &mut b.kind {
                    *is_static = true;
                }
                b.start = start_of(n);
                Ok(b)
            }
            "block" => self.block(n),
            ";" => Ok(self.mk(n, K::Empty)),
            _ => err(n, "declaration"),
        }
    }

    fn class(&self, n: &Node) -> R<Tree> {
        let kind = match n.kind() {
            "class_declaration" => ClassKind::Class,
            "interface_declaration" => ClassKind::Interface,
            "enum_declaration" => ClassKind::Enum,
            "annotation_type_declaration" => ClassKind::Annotation,
            _ => ClassKind::Record,
        };
        let mods = self.mods(n)?;
        let name = self.text(&field(n, "name")?).to_string();
        let type_params = match child_by_field(n, "type_parameters") {
            Some(tp) => named_children(&tp).iter().map(|t| self.type_parameter(t)).collect::<R<_>>()?,
            None => vec![],
        };
        let extends = match child_by_field(n, "superclass") {
            Some(s) => Some(Box::new(self.ty(&named_children(&s)[0])?)),
            None => None,
        };
        let type_list = |node: Option<Node>| -> R<Vec<Tree>> {
            match node {
                Some(x) => {
                    let list = named_children(&x)[0];
                    named_children(&list).iter().map(|t| self.ty(t)).collect()
                }
                None => Ok(vec![]),
            }
        };
        let mut implements = type_list(child_by_field(n, "interfaces"))?;
        if kind == ClassKind::Interface {
            let ext = named_children(n).into_iter().find(|c| c.kind() == "extends_interfaces");
            implements = type_list(ext)?;
        }
        let permits = type_list(child_by_field(n, "permits"))?;
        let mut record_components = Vec::new();
        if kind == ClassKind::Record {
            for p in named_children(&field(n, "parameters")?) {
                let mut v = self.formal_parameter(&p)?;
                v.is_record_component = true;
                record_components.push(v);
            }
        }
        let body = field(n, "body")?;
        let mut members = Vec::new();
        let mut enum_constants = Vec::new();
        for c in named_children(&body) {
            match c.kind() {
                "enum_constant" => enum_constants.push(self.enum_constant(&c)?),
                "enum_body_declarations" => {
                    for m in named_children(&c) {
                        members.push(self.declaration(&m)?);
                    }
                }
                _ => members.push(self.declaration(&c)?),
            }
        }
        Ok(self.mk(
            n,
            K::Class(Box::new(ClassDecl {
                kind,
                mods,
                name,
                type_params,
                extends,
                implements,
                permits,
                record_components,
                enum_constants,
                members,
            })),
        ))
    }

    fn enum_constant(&self, n: &Node) -> R<EnumConstant> {
        let mods = self.mods(n)?;
        let name = self.text(&field(n, "name")?).to_string();
        let args = match child_by_field(n, "arguments") {
            Some(a) => self.args(&a)?,
            None => vec![],
        };
        let body = match child_by_field(n, "body") {
            Some(b) => Some(named_children(&b).iter().map(|m| self.declaration(m)).collect::<R<_>>()?),
            None => None,
        };
        Ok(EnumConstant { mods, name, args, body })
    }

    fn args(&self, n: &Node) -> R<Vec<Tree>> {
        named_children(n).iter().map(|a| self.expr(a)).collect()
    }

    fn type_parameter(&self, n: &Node) -> R<Tree> {
        let mut annotations = Vec::new();
        let mut name = String::new();
        let mut bounds = Vec::new();
        for c in named_children(n) {
            match c.kind() {
                "annotation" | "marker_annotation" => annotations.push(self.annotation(&c)?),
                "type_identifier" | "identifier" => name = self.text(&c).into(),
                "type_bound" => {
                    for t in named_children(&c) {
                        bounds.push(self.ty(&t)?);
                    }
                }
                _ => return err(&c, "type parameter"),
            }
        }
        Ok(self.mk(n, K::TypeParameter { annotations, name, bounds }))
    }

    fn method(&self, n: &Node) -> R<Tree> {
        let mods = self.mods(n)?;
        let mut return_type_annotations = Vec::new();
        for c in named_children(n) {
            if is_annotation(&c) {
                return_type_annotations.push(self.annotation(&c)?);
            }
        }
        let type_params = match child_by_field(n, "type_parameters") {
            Some(tp) => named_children(&tp).iter().map(|t| self.type_parameter(t)).collect::<R<_>>()?,
            None => vec![],
        };
        let mut ret = match child_by_field(n, "type") {
            Some(t) if n.kind() != "constructor_declaration" => Some(self.ty(&t)?),
            _ => None,
        };
        if let Some(dims) = child_by_field(n, "dimensions") {
            ret = match ret {
                Some(r) => Some(self.add_dims(r, &dims)?),
                None => None,
            };
        }
        let name = self.text(&field(n, "name")?).to_string();
        let mut params = Vec::new();
        let mut receiver = None;
        if let Some(ps) = child_by_field(n, "parameters") {
            for p in named_children(&ps) {
                if p.kind() == "receiver_parameter" {
                    receiver = Some(Box::new(self.receiver_parameter(&p)?));
                } else {
                    params.push(self.formal_parameter(&p)?);
                }
            }
        }
        let mut throws = Vec::new();
        for c in named_children(n) {
            if c.kind() == "throws" {
                for t in named_children(&c) {
                    throws.push(self.ty(&t)?);
                }
            }
        }
        let body = match child_by_field(n, "body") {
            Some(b) => Some(Box::new(self.block(&b)?)),
            None => None,
        };
        let default_value = match child_by_field(n, "value") {
            Some(v) => Some(Box::new(self.element_value(&v)?)),
            None => None,
        };
        Ok(self.mk(
            n,
            K::Method(Box::new(MethodDecl {
                mods,
                return_type_annotations,
                type_params,
                ret: ret.map(Box::new),
                name,
                receiver,
                params,
                throws,
                body,
                default_value,
                compact_ctor: n.kind() == "compact_constructor_declaration",
            })),
        ))
    }

    fn receiver_parameter(&self, n: &Node) -> R<VarDecl> {
        let mut annotations = Vec::new();
        let mut ty = None;
        let mut name_parts = Vec::new();
        for c in named_children(n) {
            if is_annotation(&c) && ty.is_none() {
                annotations.push(self.annotation(&c)?);
            } else if ty.is_none() {
                ty = Some(Box::new(self.ty(&c)?));
            } else {
                name_parts.push(self.text(&c).to_string());
            }
        }
        Ok(VarDecl {
            start: start_of(n),
            end: n.end_byte(),
            mods: Mods { start: start_of(n), annotations },
            ty,
            name: name_parts.join("."),
            init: None,
            is_record_component: false,
        })
    }

    fn formal_parameter(&self, n: &Node) -> R<VarDecl> {
        let mods = self.mods(n)?;
        if n.kind() == "spread_parameter" {
            // `Type... name`: children are modifiers?, type, annotations?, variable_declarator.
            let mut ty = None;
            let mut decl = None;
            for c in named_children(n) {
                match c.kind() {
                    "modifiers" => {}
                    "variable_declarator" => decl = Some(c),
                    "annotation" | "marker_annotation" => {}
                    _ => ty = Some(self.ty(&c)?),
                }
            }
            let decl = decl.ok_or_else(|| ConvertError::new("spread"))?;
            let ty = ty.ok_or_else(|| ConvertError::new("spread type"))?;
            let varargs_dims = vec![self.annotations_in(n, "variadic")?];
            let mut ty =
                Tree::new(ty.start, decl.start_byte(), K::ArrayType { elem: Box::new(ty), dims: varargs_dims });
            if let Some(d) = child_by_field(&decl, "dimensions") {
                ty = self.add_dims(ty, &d)?;
            }
            return Ok(VarDecl {
                start: mods.start.min(start_of(n)),
                end: n.end_byte(),
                mods,
                ty: Some(Box::new(ty)),
                name: self.text(&field(&decl, "name")?).into(),
                init: None,
                is_record_component: false,
            });
        }
        let mut ty = self.ty(&field(n, "type")?)?;
        if let Some(d) = child_by_field(n, "dimensions") {
            ty = self.add_dims(ty, &d)?;
        }
        Ok(VarDecl {
            start: start_of(n),
            end: n.end_byte(),
            mods,
            ty: Some(Box::new(ty)),
            name: self.text(&field(n, "name")?).into(),
            init: None,
            is_record_component: false,
        })
    }

    /// Annotations that are direct children of a spread parameter (annotating the varargs dimension).
    fn annotations_in(&self, n: &Node, _what: &str) -> R<Vec<Tree>> {
        let mut out = Vec::new();
        for c in named_children(n) {
            if is_annotation(&c) {
                out.push(self.annotation(&c)?);
            }
        }
        Ok(out)
    }

    /// Wraps `ty` with the array dimensions of a `dimensions` node.
    fn add_dims(&self, ty: Tree, dims: &Node) -> R<Tree> {
        let mut groups: Vec<Vec<Tree>> = Vec::new();
        let mut current = Vec::new();
        for c in all_children(dims) {
            match c.kind() {
                "[" => groups.push(std::mem::take(&mut current)),
                "]" => {}
                _ if is_annotation(&c) => current.push(self.annotation(&c)?),
                _ => {}
            }
        }
        let start = ty.start;
        let end = dims.end_byte();
        match ty.kind {
            K::ArrayType { elem, dims: mut existing } => {
                existing.extend(groups);
                Ok(Tree::new(start, end, K::ArrayType { elem, dims: existing }))
            }
            other => {
                let elem = Box::new(Tree::new(start, ty.end, other));
                Ok(Tree::new(start, end, K::ArrayType { elem, dims: groups }))
            }
        }
    }

    fn vars(&self, n: &Node) -> R<Tree> {
        let mods = self.mods(n)?;
        let ty_node = field(n, "type")?;
        let base_ty = if self.text(&ty_node) == "var" { None } else { Some(self.ty(&ty_node)?) };
        let mut cursor = n.walk();
        let declarators: Vec<Node> = n.children_by_field_name("declarator", &mut cursor).collect();
        let mut vars = Vec::new();
        for d in declarators {
            let mut ty = base_ty.clone();
            if let (Some(t), Some(dims)) = (ty.take(), child_by_field(&d, "dimensions")) {
                ty = Some(self.add_dims(t, &dims)?);
            } else {
                ty = base_ty.clone();
            }
            let init = match child_by_field(&d, "value") {
                Some(v) => Some(Box::new(self.var_init(&v)?)),
                None => None,
            };
            vars.push(VarDecl {
                start: start_of(n),
                end: d.end_byte(),
                mods: mods.clone(),
                ty: ty.map(Box::new),
                name: self.text(&field(&d, "name")?).into(),
                init,
                is_record_component: false,
            });
        }
        Ok(self.mk(n, K::Vars(vars)))
    }

    fn var_init(&self, n: &Node) -> R<Tree> {
        if n.kind() == "array_initializer" { self.array_initializer(n) } else { self.expr(n) }
    }

    fn array_initializer(&self, n: &Node) -> R<Tree> {
        let init = named_children(n).iter().map(|c| self.var_init(c)).collect::<R<Vec<_>>>()?;
        Ok(self.mk(n, K::NewArray { elem: None, dims: vec![], dim_annotations: vec![], init: Some(init) }))
    }

    fn block(&self, n: &Node) -> R<Tree> {
        let mut stmts = Vec::new();
        for c in all_children(n) {
            if c.is_named() || c.kind() == ";" {
                self.push_stmt(&c, &mut stmts)?;
            }
        }
        Ok(self.mk(n, K::Block { is_static: false, stmts }))
    }

    /// Converts a statement. javac parses `switch (x) {...};` as a switch statement followed by an empty statement,
    /// while tree-sitter sees an expression statement.
    fn push_stmt(&self, n: &Node, out: &mut Vec<Tree>) -> R<()> {
        if n.kind() == "expression_statement"
            && let [e] = named_children(n).as_slice()
            && e.kind() == "switch_expression"
        {
            out.push(self.stmt(e)?);
            out.push(Tree::new(n.end_byte() - 1, n.end_byte(), K::Empty));
            return Ok(());
        }
        out.push(self.stmt(n)?);
        Ok(())
    }

    pub fn stmt(&self, n: &Node) -> R<Tree> {
        let kind = match n.kind() {
            "block" | "constructor_body" => return self.block(n),
            ";" => K::Empty,
            "expression_statement" => K::ExprStmt(Box::new(self.expr(&named_children(n)[0])?)),
            "explicit_constructor_invocation" => {
                let ctor = field(n, "constructor")?;
                let mut select = self.mk(&ctor, K::Ident(self.text(&ctor).into()));
                if let Some(obj) = child_by_field(n, "object") {
                    let obj = self.expr(&obj)?;
                    select = Tree::new(
                        obj.start,
                        ctor.end_byte(),
                        K::MemberSelect { expr: Box::new(obj), name: self.text(&ctor).into() },
                    );
                }
                let type_args = self.type_args(child_by_field(n, "type_arguments"))?;
                let args = self.args(&field(n, "arguments")?)?;
                let inv = Tree::new(
                    start_of(n),
                    n.end_byte(),
                    K::MethodInvocation { select: Box::new(select), type_args, args },
                );
                K::ExprStmt(Box::new(inv))
            }
            "local_variable_declaration" => return self.vars(n),
            "if_statement" => K::If {
                cond: Box::new(self.expr(&field(n, "condition")?)?),
                then: Box::new(self.stmt(&field(n, "consequence")?)?),
                else_: match child_by_field(n, "alternative") {
                    Some(a) => Some(Box::new(self.stmt(&a)?)),
                    None => None,
                },
            },
            "while_statement" => K::While {
                cond: Box::new(self.expr(&field(n, "condition")?)?),
                body: Box::new(self.stmt(&field(n, "body")?)?),
            },
            "do_statement" => K::DoWhile {
                body: Box::new(self.stmt(&field(n, "body")?)?),
                cond: Box::new(self.expr(&field(n, "condition")?)?),
            },
            "for_statement" => {
                let mut cursor = n.walk();
                let init = n
                    .children_by_field_name("init", &mut cursor)
                    .map(|c| {
                        if c.kind() == "local_variable_declaration" {
                            self.vars(&c)
                        } else {
                            let e = self.expr(&c)?;
                            Ok(Tree::new(e.start, e.end, K::ExprStmt(Box::new(e))))
                        }
                    })
                    .collect::<R<Vec<_>>>()?;
                let mut cursor = n.walk();
                let update =
                    n.children_by_field_name("update", &mut cursor).map(|c| self.expr(&c)).collect::<R<Vec<_>>>()?;
                K::For {
                    init,
                    cond: match child_by_field(n, "condition") {
                        Some(c) => Some(Box::new(self.expr(&c)?)),
                        None => None,
                    },
                    update,
                    body: Box::new(self.stmt(&field(n, "body")?)?),
                }
            }
            "enhanced_for_statement" => {
                let mods = self.mods(n)?;
                let ty_node = field(n, "type")?;
                let mut ty = if self.text(&ty_node) == "var" { None } else { Some(self.ty(&ty_node)?) };
                if let (Some(t), Some(d)) = (ty.take(), child_by_field(n, "dimensions")) {
                    ty = Some(self.add_dims(t, &d)?);
                } else if self.text(&ty_node) != "var" {
                    ty = Some(self.ty(&ty_node)?);
                }
                let name_node = field(n, "name")?;
                let var = VarDecl {
                    start: mods.start.min(start_of(&ty_node)),
                    end: name_node.end_byte(),
                    mods,
                    ty: ty.map(Box::new),
                    name: self.text(&name_node).into(),
                    init: None,
                    is_record_component: false,
                };
                K::ForEach {
                    var: Box::new(var),
                    expr: Box::new(self.expr(&field(n, "value")?)?),
                    body: Box::new(self.stmt(&field(n, "body")?)?),
                }
            }
            "labeled_statement" => {
                let ch = named_children(n);
                K::Labeled { label: self.text(&ch[0]).into(), body: Box::new(self.stmt(&ch[1])?) }
            }
            "switch_expression" => {
                let (selector, cases) = self.switch(n)?;
                K::Switch { selector, cases }
            }
            "try_statement" | "try_with_resources_statement" => {
                let mut resources = Vec::new();
                if let Some(rs) = child_by_field(n, "resources") {
                    for r in named_children(&rs) {
                        resources.push(self.resource(&r)?);
                    }
                }
                let block = Box::new(self.block(&field(n, "body")?)?);
                let mut catches = Vec::new();
                let mut finally = None;
                for c in named_children(n) {
                    match c.kind() {
                        "catch_clause" => catches.push(self.catch(&c)?),
                        "finally_clause" => finally = Some(Box::new(self.block(&named_children(&c)[0])?)),
                        _ => {}
                    }
                }
                K::Try { resources, block, catches, finally }
            }
            "throw_statement" => K::Throw(Box::new(self.expr(&named_children(n)[0])?)),
            "return_statement" => K::Return(match named_children(n).first() {
                Some(e) => Some(Box::new(self.expr(e)?)),
                None => None,
            }),
            "break_statement" => K::Break(named_children(n).first().map(|l| self.text(l).to_string())),
            "continue_statement" => K::Continue(named_children(n).first().map(|l| self.text(l).to_string())),
            "synchronized_statement" => {
                let ch = named_children(n);
                K::Synchronized { lock: Box::new(self.expr(&ch[0])?), block: Box::new(self.block(&field(n, "body")?)?) }
            }
            "assert_statement" => {
                let ch = named_children(n);
                K::Assert {
                    cond: Box::new(self.expr(&ch[0])?),
                    detail: match ch.get(1) {
                        Some(d) => Some(Box::new(self.expr(d)?)),
                        None => None,
                    },
                }
            }
            "yield_statement" => K::Yield(Box::new(self.expr(&named_children(n)[0])?)),
            _ => return self.declaration(n),
        };
        Ok(self.mk(n, kind))
    }

    fn resource(&self, n: &Node) -> R<Tree> {
        if let Some(ty_node) = child_by_field(n, "type") {
            let mods = self.mods(n)?;
            let mut ty = if self.text(&ty_node) == "var" { None } else { Some(self.ty(&ty_node)?) };
            if let (Some(t), Some(d)) = (ty.take(), child_by_field(n, "dimensions")) {
                ty = Some(self.add_dims(t, &d)?);
            } else if self.text(&ty_node) != "var" {
                ty = Some(self.ty(&ty_node)?);
            }
            let var = VarDecl {
                start: start_of(n),
                end: n.end_byte(),
                mods,
                ty: ty.map(Box::new),
                name: self.text(&field(n, "name")?).into(),
                init: match child_by_field(n, "value") {
                    Some(v) => Some(Box::new(self.expr(&v)?)),
                    None => None,
                },
                is_record_component: false,
            };
            return Ok(self.mk(n, K::Var(Box::new(var))));
        }
        self.expr(&named_children(n)[0])
    }

    fn catch(&self, n: &Node) -> R<Catch> {
        let param = named_children(n).into_iter().find(|c| c.kind() == "catch_formal_parameter").unwrap();
        let mods = self.mods(&param)?;
        let catch_type = named_children(&param).into_iter().find(|c| c.kind() == "catch_type").unwrap();
        let alternatives = named_children(&catch_type).iter().map(|t| self.ty(t)).collect::<R<Vec<_>>>()?;
        let ty = if alternatives.len() == 1 {
            alternatives[0].clone()
        } else {
            self.mk(&catch_type, K::Union(alternatives.clone()))
        };
        let var = VarDecl {
            start: start_of(&param),
            end: param.end_byte(),
            mods,
            ty: Some(Box::new(ty)),
            name: self.text(&field(&param, "name")?).into(),
            init: None,
            is_record_component: false,
        };
        Ok(Catch {
            start: start_of(n),
            param: var,
            union: if alternatives.len() > 1 { alternatives } else { vec![] },
            block: Box::new(self.block(&field(n, "body")?)?),
        })
    }

    fn switch(&self, n: &Node) -> R<(B, Vec<Case>)> {
        let selector = Box::new(self.expr(&field(n, "condition")?)?);
        let mut cases = Vec::new();
        for c in named_children(&field(n, "body")?) {
            let mut labels = Vec::new();
            let mut is_default = false;
            let mut guard = None;
            let ch: Vec<Node> = all_children(&c).into_iter().filter(|x| x.is_named() || x.kind() == ";").collect();
            let mut stmts = Vec::new();
            let mut body = None;
            for x in &ch {
                if c.kind() == "switch_rule" && x.kind() == ";" {
                    continue;
                }
                if x.kind() == "switch_label" {
                    let label_children = all_children(x);
                    if label_children.iter().any(|y| y.kind() == "default") && labels.is_empty() {
                        if label_children.len() == 1 {
                            is_default = true;
                        } else {
                            // `case null, default`
                            for y in &label_children {
                                if y.kind() == "default" {
                                    labels.push(self.mk(y, K::DefaultCaseLabel));
                                }
                            }
                        }
                    }
                    for y in named_children(x) {
                        match y.kind() {
                            "guard" => guard = Some(Box::new(self.expr(&named_children(&y)[0])?)),
                            "pattern" => labels.push(self.pattern(&named_children(&y)[0])?),
                            "type_pattern" | "record_pattern" => labels.push(self.pattern(&y)?),
                            _ => labels.push(self.expr(&y)?),
                        }
                    }
                    // `case null, default`: the default label comes after the expressions.
                    if !is_default && label_children.iter().any(|y| y.kind() == "default") {
                        labels.retain(|l| !matches!(l.kind, K::DefaultCaseLabel));
                        for y in &label_children {
                            if y.kind() == "default" {
                                labels.push(self.mk(y, K::DefaultCaseLabel));
                            }
                        }
                    }
                } else if c.kind() == "switch_rule" {
                    body = Some(Box::new(match x.kind() {
                        "expression_statement" => self.expr(&named_children(x)[0])?,
                        _ => self.stmt(x)?,
                    }));
                } else {
                    self.push_stmt(x, &mut stmts)?;
                }
            }
            cases.push(Case {
                start: start_of(&c),
                labels,
                is_default,
                guard,
                kind: if c.kind() == "switch_rule" { CaseKind::Rule } else { CaseKind::Statement },
                stmts,
                body,
            });
        }
        Ok((selector, cases))
    }

    fn pattern(&self, n: &Node) -> R<Tree> {
        match n.kind() {
            "pattern" => self.pattern(&named_children(n)[0]),
            "type_pattern" => {
                let ch = named_children(n);
                let mods = self.mods(n)?;
                let ty_nodes: Vec<&Node> = ch.iter().filter(|c| c.kind() != "modifiers").collect();
                let ty_node = ty_nodes[0];
                let name_node = ty_nodes[ty_nodes.len() - 1];
                let ty = if self.text(ty_node) == "var" { None } else { Some(Box::new(self.ty(ty_node)?)) };
                let var = VarDecl {
                    start: start_of(n),
                    end: n.end_byte(),
                    mods,
                    ty,
                    name: self.text(name_node).into(),
                    init: None,
                    is_record_component: false,
                };
                Ok(self.mk(n, K::BindingPattern { var: Box::new(var) }))
            }
            "record_pattern" => {
                let ch = named_children(n);
                let ty = Box::new(self.ty(&ch[0])?);
                let mut nested = Vec::new();
                if let Some(body) = ch.iter().find(|c| c.kind() == "record_pattern_body") {
                    for p in named_children(body) {
                        nested.push(self.pattern(&p)?);
                    }
                }
                Ok(self.mk(n, K::RecordPattern { ty, nested }))
            }
            "record_pattern_component" => {
                let mods = self.mods(n)?;
                let ch: Vec<Node> = named_children(n).into_iter().filter(|c| c.kind() != "modifiers").collect();
                if ch.len() == 1 && ch[0].kind() == "underscore_pattern" {
                    return Ok(self.mk(n, K::AnyPattern));
                }
                if ch.len() == 1 {
                    // a lone identifier
                    let var = VarDecl {
                        start: start_of(n),
                        end: n.end_byte(),
                        mods: Mods { start: start_of(n), annotations: vec![] },
                        ty: None,
                        name: self.text(&ch[0]).into(),
                        init: None,
                        is_record_component: false,
                    };
                    return Ok(self.mk(n, K::BindingPattern { var: Box::new(var) }));
                }
                let ty = if self.text(&ch[0]) == "var" { None } else { Some(Box::new(self.ty(&ch[0])?)) };
                let var = VarDecl {
                    start: start_of(n),
                    end: n.end_byte(),
                    mods,
                    ty,
                    name: self.text(&ch[1]).into(),
                    init: None,
                    is_record_component: false,
                };
                Ok(self.mk(n, K::BindingPattern { var: Box::new(var) }))
            }
            _ => err(n, "pattern"),
        }
    }

    fn type_args(&self, n: Option<Node>) -> R<Vec<Tree>> {
        match n {
            Some(n) => named_children(&n).iter().map(|t| self.ty(t)).collect(),
            None => Ok(vec![]),
        }
    }

    pub fn ty(&self, n: &Node) -> R<Tree> {
        let kind = match n.kind() {
            "integral_type" | "floating_point_type" | "boolean_type" | "void_type" => K::Primitive(self.text(n).into()),
            "type_identifier" | "identifier" => K::Ident(self.text(n).into()),
            "scoped_type_identifier" => {
                let ch = named_children(n);
                let scope = self.ty(&ch[0])?;
                let annotations = ch[1..ch.len() - 1].iter().map(|a| self.annotation(a)).collect::<R<Vec<_>>>()?;
                let name = self.text(&ch[ch.len() - 1]).to_string();
                let select = self.mk(n, K::MemberSelect { expr: Box::new(scope), name });
                if annotations.is_empty() {
                    return Ok(select);
                }
                K::Annotated { annotations, base: Box::new(select) }
            }
            "generic_type" => {
                let ch = named_children(n);
                let base = Box::new(self.ty(&ch[0])?);
                let args = self.type_args(ch.get(1).copied())?;
                K::Parameterized { base, args }
            }
            "array_type" => {
                let elem = self.ty(&field(n, "element")?)?;
                return self.add_dims(elem, &field(n, "dimensions")?);
            }
            "annotated_type" => {
                let ch = named_children(n);
                let annotations = ch[..ch.len() - 1].iter().map(|a| self.annotation(a)).collect::<R<Vec<_>>>()?;
                let base = self.ty(&ch[ch.len() - 1])?;
                if let K::ArrayType { elem, dims } = base.kind {
                    // `@A int[]`: the annotations belong to the element type.
                    let start = start_of(n);
                    let elem = Tree::new(start, elem.end, K::Annotated { annotations, base: elem });
                    return Ok(Tree::new(start, n.end_byte(), K::ArrayType { elem: Box::new(elem), dims }));
                }
                K::Annotated { annotations, base: Box::new(base) }
            }
            "wildcard" => {
                let all = all_children(n);
                let mut annotations = Vec::new();
                let mut bound = None;
                let mut extends = true;
                for c in &all {
                    match c.kind() {
                        "super" => extends = false,
                        "extends" => extends = true,
                        "annotation" | "marker_annotation" => annotations.push(self.annotation(c)?),
                        "?" => {}
                        _ if c.is_named() => bound = Some(Box::new(self.ty(c)?)),
                        _ => {}
                    }
                }
                let w = self.mk(n, K::Wildcard { bound: bound.map(|b| (extends, b)) });
                if annotations.is_empty() {
                    return Ok(w);
                }
                K::Annotated { annotations, base: Box::new(w) }
            }
            _ => return self.expr(n),
        };
        Ok(self.mk(n, kind))
    }

    pub fn expr(&self, n: &Node) -> R<Tree> {
        let kind = match n.kind() {
            "identifier" | "this" | "super" | "type_identifier" => K::Ident(self.text(n).into()),
            "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal" => {
                if self.text(n).ends_with(['l', 'L']) {
                    K::Literal(LitKind::Long)
                } else {
                    K::Literal(LitKind::Int)
                }
            }
            "decimal_floating_point_literal" | "hex_floating_point_literal" => {
                if self.text(n).ends_with(['f', 'F']) {
                    K::Literal(LitKind::Float)
                } else {
                    K::Literal(LitKind::Double)
                }
            }
            "character_literal" => K::Literal(LitKind::Char),
            "string_literal" => K::Literal(LitKind::String),
            "true" | "false" => K::Literal(LitKind::Boolean),
            "null_literal" => K::Literal(LitKind::Null),
            "parenthesized_expression" => K::Parens(Box::new(self.expr(&named_children(n)[0])?)),
            "field_access" => {
                let obj = self.expr(&field(n, "object")?)?;
                let mut expr = obj;
                // `Outer.super.field`
                let ch = named_children(n);
                if ch.len() == 3 && ch[1].kind() == "super" {
                    expr = Tree::new(
                        expr.start,
                        ch[1].end_byte(),
                        K::MemberSelect { expr: Box::new(expr), name: "super".into() },
                    );
                }
                K::MemberSelect { expr: Box::new(expr), name: self.text(&field(n, "field")?).into() }
            }
            "scoped_identifier" => return self.name(n),
            "class_literal" => {
                let ty = self.ty(&named_children(n)[0])?;
                K::MemberSelect { expr: Box::new(ty), name: "class".into() }
            }
            "method_invocation" => {
                let name_node = field(n, "name")?;
                let name = self.text(&name_node).to_string();
                let type_args = self.type_args(child_by_field(n, "type_arguments"))?;
                let select = match child_by_field(n, "object") {
                    Some(obj) => {
                        let mut expr = self.expr(&obj)?;
                        // `Outer.super.method()`
                        let supers: Vec<Node> = named_children(n).into_iter().filter(|c| c.kind() == "super").collect();
                        if obj.kind() != "super"
                            && let Some(s) = supers.first()
                        {
                            expr = Tree::new(
                                expr.start,
                                s.end_byte(),
                                K::MemberSelect { expr: Box::new(expr), name: "super".into() },
                            );
                        }
                        Tree::new(expr.start, name_node.end_byte(), K::MemberSelect { expr: Box::new(expr), name })
                    }
                    None => self.mk(&name_node, K::Ident(name)),
                };
                K::MethodInvocation { select: Box::new(select), type_args, args: self.args(&field(n, "arguments")?)? }
            }
            "array_access" => K::ArrayAccess {
                expr: Box::new(self.expr(&field(n, "array")?)?),
                index: Box::new(self.expr(&field(n, "index")?)?),
            },
            "binary_expression" => {
                let binary = Tree::new(
                    start_of(n),
                    n.end_byte(),
                    K::Binary {
                        op: self.text(&field(n, "operator")?).into(),
                        lhs: Box::new(self.expr(&field(n, "left")?)?),
                        rhs: Box::new(self.expr(&field(n, "right")?)?),
                    },
                );
                return Ok(rebalance(binary));
            }
            "unary_expression"
                if self.text(&field(n, "operator")?) == "-"
                    && field(n, "operand")?.kind() == "decimal_integer_literal" =>
            {
                let operand = field(n, "operand")?;
                if self.text(&operand).ends_with(['l', 'L']) {
                    K::Literal(LitKind::Long)
                } else {
                    K::Literal(LitKind::Int)
                }
            }
            "unary_expression" => K::Unary {
                op: self.text(&field(n, "operator")?).into(),
                postfix: false,
                expr: Box::new(self.expr(&field(n, "operand")?)?),
            },
            "update_expression" => {
                let all = all_children(n);
                let postfix = all[0].is_named();
                let (op, operand) = if postfix { (&all[1], &all[0]) } else { (&all[0], &all[1]) };
                K::Unary { op: self.text(op).into(), postfix, expr: Box::new(self.expr(operand)?) }
            }
            "assignment_expression" => K::Assign {
                op: self.text(&field(n, "operator")?).into(),
                lhs: Box::new(self.expr(&field(n, "left")?)?),
                rhs: Box::new(self.expr(&field(n, "right")?)?),
            },
            "ternary_expression" => K::Conditional {
                cond: Box::new(self.expr(&field(n, "condition")?)?),
                then: Box::new(self.expr(&field(n, "consequence")?)?),
                else_: Box::new(self.expr(&field(n, "alternative")?)?),
            },
            "instanceof_expression" => {
                let expr = Box::new(self.expr(&field(n, "left")?)?);
                let target = if let Some(p) = child_by_field(n, "pattern") {
                    self.pattern(&p)?
                } else {
                    let ty_node = field(n, "right")?;
                    let ty = self.ty(&ty_node)?;
                    match child_by_field(n, "name") {
                        Some(name) => {
                            let mods = self.instanceof_mods(n)?;
                            let var = VarDecl {
                                start: mods.as_ref().map(|m| m.start).unwrap_or(ty.start),
                                end: name.end_byte(),
                                mods: mods.unwrap_or(Mods { start: ty.start, annotations: vec![] }),
                                ty: Some(Box::new(ty)),
                                name: self.text(&name).into(),
                                init: None,
                                is_record_component: false,
                            };
                            Tree::new(var.start, var.end, K::BindingPattern { var: Box::new(var) })
                        }
                        None => ty,
                    }
                };
                K::InstanceOf { expr, target: Box::new(target) }
            }
            // JLS: a cast to a reference type cannot be followed by unary `+`/`-`, so `(a) + b` is an addition.
            // tree-sitter parses it as a cast; javac as a binary expression.
            "cast_expression" if self.misparsed_cast(n) => {
                let ty = field(n, "type")?;
                let value = field(n, "value")?;
                let operand = field(&value, "operand")?;
                let name = self.ty(&ty)?;
                let lhs = Tree::new(start_of(n), ty.end_byte() + 1, K::Parens(Box::new(name)));
                let binary = Tree::new(
                    start_of(n),
                    n.end_byte(),
                    K::Binary {
                        op: self.text(&field(&value, "operator")?).into(),
                        lhs: Box::new(lhs),
                        rhs: Box::new(self.expr(&operand)?),
                    },
                );
                return Ok(binary);
            }
            "cast_expression" => {
                let mut cursor = n.walk();
                let type_nodes: Vec<Node> = n.children_by_field_name("type", &mut cursor).collect();
                // tree-sitter-java-orchard splits `(@A int) x` into an annotation-only `annotated_type` followed by
                // the type; merge them back into one annotated type.
                let mut types = Vec::new();
                let mut pending: Vec<Tree> = Vec::new();
                for t in &type_nodes {
                    let children = named_children(t);
                    if t.kind() == "annotated_type" && children.iter().all(is_annotation) {
                        for a in &children {
                            pending.push(self.annotation(a)?);
                        }
                        continue;
                    }
                    let mut ty = self.ty(t)?;
                    if !pending.is_empty() {
                        let start = pending[0].start;
                        let mut annotations = std::mem::take(&mut pending);
                        ty = match ty.kind {
                            K::Annotated { annotations: inner, base } => {
                                annotations.extend(inner);
                                Tree::new(start, ty.end, K::Annotated { annotations, base })
                            }
                            other => {
                                let base = Box::new(Tree::new(ty.start, ty.end, other));
                                Tree::new(start, ty.end, K::Annotated { annotations, base })
                            }
                        };
                    }
                    types.push(ty);
                }
                let ty = if types.len() == 1 {
                    types.into_iter().next().unwrap()
                } else {
                    let start = types[0].start;
                    let end = types[types.len() - 1].end;
                    Tree::new(start, end, K::Intersection(types))
                };
                K::Cast { ty: Box::new(ty), expr: Box::new(self.expr(&field(n, "value")?)?) }
            }
            "object_creation_expression" => {
                let outer = named_children(n).into_iter().find(|c| {
                    c.end_byte() <= field(n, "type").map(|t| t.start_byte()).unwrap_or(0) && !is_annotation(c)
                });
                let outer = match outer {
                    Some(o) => Some(Box::new(self.expr(&o)?)),
                    None => None,
                };
                let ident_node = field(n, "type")?;
                let mut ident = self.ty(&ident_node)?;
                let annotations: Vec<Tree> = named_children(n)
                    .iter()
                    .filter(|c| is_annotation(c))
                    .map(|a| self.annotation(a))
                    .collect::<R<_>>()?;
                if !annotations.is_empty() {
                    let start = annotations[0].start;
                    ident = Tree::new(start, ident.end, K::Annotated { annotations, base: Box::new(ident) });
                }
                let body = match named_children(n).into_iter().find(|c| c.kind() == "class_body") {
                    Some(b) => Some(named_children(&b).iter().map(|m| self.declaration(m)).collect::<R<_>>()?),
                    None => None,
                };
                K::NewClass {
                    outer,
                    type_args: self.type_args(child_by_field(n, "type_arguments"))?,
                    ident: Box::new(ident),
                    args: self.args(&field(n, "arguments")?)?,
                    body,
                }
            }
            "array_creation_expression" => {
                let mut elem = self.ty(&field(n, "type")?)?;
                let annotations: Vec<Tree> = named_children(n)
                    .iter()
                    .filter(|c| is_annotation(c))
                    .map(|a| self.annotation(a))
                    .collect::<R<_>>()?;
                if !annotations.is_empty() {
                    let start = annotations[0].start;
                    elem = Tree::new(start, elem.end, K::Annotated { annotations, base: Box::new(elem) });
                }
                let elem = Box::new(elem);
                let mut dims = Vec::new();
                let mut dim_annotations = Vec::new();
                let mut cursor = n.walk();
                for d in n.children_by_field_name("dimensions", &mut cursor) {
                    if d.kind() == "dimensions_expr" {
                        let mut annos = Vec::new();
                        for c in named_children(&d) {
                            if is_annotation(&c) {
                                annos.push(self.annotation(&c)?);
                            } else {
                                dims.push(self.expr(&c)?);
                            }
                        }
                        dim_annotations.push(annos);
                    } else {
                        let mut current = Vec::new();
                        for c in all_children(&d) {
                            match c.kind() {
                                "[" => {}
                                "]" => dim_annotations.push(std::mem::take(&mut current)),
                                _ if is_annotation(&c) => current.push(self.annotation(&c)?),
                                _ => {}
                            }
                        }
                    }
                }
                let init = match child_by_field(n, "value") {
                    Some(v) => match self.array_initializer(&v)?.kind {
                        K::NewArray { init, .. } => init,
                        _ => None,
                    },
                    None => None,
                };
                K::NewArray { elem: Some(elem), dims, dim_annotations, init }
            }
            "array_initializer" => return self.array_initializer(n),
            "lambda_expression" => {
                let p = field(n, "parameters")?;
                let params = match p.kind() {
                    "identifier" => vec![self.inferred_param(&p)],
                    "inferred_parameters" => named_children(&p).iter().map(|c| self.inferred_param(c)).collect(),
                    _ => {
                        let mut ps = Vec::new();
                        for c in named_children(&p) {
                            ps.push(self.formal_parameter(&c)?);
                        }
                        ps
                    }
                };
                let b = field(n, "body")?;
                let body = if b.kind() == "block" { self.block(&b)? } else { self.expr(&b)? };
                K::Lambda { params, body: Box::new(body) }
            }
            "method_reference" => {
                let all = all_children(n);
                let qual = Box::new(self.ty(&all[0])?);
                let mut type_args = Vec::new();
                let mut name = None;
                for c in &all[1..] {
                    match c.kind() {
                        "type_arguments" => type_args = self.type_args(Some(*c))?,
                        "identifier" => name = Some(self.text(c).to_string()),
                        _ => {}
                    }
                }
                K::MemberRef { qual, type_args, name }
            }
            "switch_expression" => {
                let (selector, cases) = self.switch(n)?;
                K::SwitchExpr { selector, cases }
            }
            "integral_type"
            | "floating_point_type"
            | "boolean_type"
            | "void_type"
            | "generic_type"
            | "array_type"
            | "scoped_type_identifier"
            | "annotated_type" => return self.ty(n),
            _ => return err(n, "expression"),
        };
        Ok(self.mk(n, kind))
    }

    fn misparsed_cast(&self, n: &Node) -> bool {
        let mut cursor = n.walk();
        let types: Vec<Node> = n.children_by_field_name("type", &mut cursor).collect();
        let Some(value) = child_by_field(n, "value") else { return false };
        types.len() == 1
            && matches!(types[0].kind(), "type_identifier" | "scoped_type_identifier")
            && value.kind() == "unary_expression"
            && child_by_field(&value, "operator").is_some_and(|op| matches!(self.text(&op), "+" | "-"))
    }

    fn instanceof_mods(&self, n: &Node) -> R<Option<Mods>> {
        // `x instanceof final Foo f`: the modifiers are a child of the instanceof expression.
        match named_children(n).into_iter().find(|c| c.kind() == "modifiers") {
            Some(m) => {
                let mut annotations = Vec::new();
                for a in named_children(&m) {
                    if is_annotation(&a) {
                        annotations.push(self.annotation(&a)?);
                    }
                }
                Ok(Some(Mods { start: start_of(&m), annotations }))
            }
            None => Ok(None),
        }
    }

    fn inferred_param(&self, n: &Node) -> VarDecl {
        VarDecl {
            start: start_of(n),
            end: n.end_byte(),
            mods: Mods { start: start_of(n), annotations: vec![] },
            ty: None,
            name: self.text(n).into(),
            init: None,
            is_record_component: false,
        }
    }
}

/// The first error or missing node in the tree.
pub(crate) fn first_error<'t>(n: &Node<'t>) -> Node<'t> {
    if n.is_error() || n.is_missing() {
        return *n;
    }
    let mut cursor = n.walk();
    for c in n.children(&mut cursor) {
        if c.has_error() {
            return first_error(&c);
        }
    }
    *n
}

fn binary_precedence(op: &str) -> i32 {
    match op {
        "||" => 4,
        "&&" => 5,
        "|" => 6,
        "^" => 7,
        "&" => 8,
        "==" | "!=" => 9,
        "<" | ">" | "<=" | ">=" => 10,
        "<<" | ">>" | ">>>" => 11,
        "+" | "-" => 12,
        _ => 13,
    }
}

/// Restores operator precedence after a misparsed cast was turned into an addition/subtraction: tree-sitter nests
/// such a cast as a tight operand, so it can end up as the operand of a stronger-binding operator.
fn rebalance(t: Tree) -> Tree {
    let Tree { start, end, kind: K::Binary { op, lhs, rhs } } = t else { return t };
    let prec = binary_precedence(&op);
    if let K::Binary { op: lop, .. } = &lhs.kind
        && binary_precedence(lop) < prec
    {
        // (a lop b) op c  =>  a lop (b op c)
        let K::Binary { op: lop, lhs: a, rhs: b } = lhs.kind else { unreachable!() };
        let inner = rebalance(Tree::new(b.start, end, K::Binary { op, lhs: b, rhs }));
        return Tree::new(start, end, K::Binary { op: lop, lhs: a, rhs: Box::new(inner) });
    }
    if let K::Binary { op: rop, .. } = &rhs.kind
        && binary_precedence(rop) <= prec
    {
        // a op (b rop c)  =>  (a op b) rop c
        let K::Binary { op: rop, lhs: b, rhs: c } = rhs.kind else { unreachable!() };
        let inner = rebalance(Tree::new(start, b.end, K::Binary { op, lhs, rhs: b }));
        return Tree::new(start, end, K::Binary { op: rop, lhs: Box::new(inner), rhs: c });
    }
    Tree::new(start, end, K::Binary { op, lhs, rhs })
}
