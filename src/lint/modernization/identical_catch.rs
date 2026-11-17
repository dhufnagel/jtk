//! M010: `catch` clause with the same body as an earlier one of the same `try` (PMD IdenticalCatchBranches, Eclipse
//! quick fix "Combine catch blocks"), Java 7: a multi-catch says it once.
//!
//! As in PMD, bodies are compared token by token, with the earlier clause's parameter name standing for the later
//! one's; comments do not count. PMD also requires the calls to resolve to the same methods, which needs types:
//! instead, bodies that pass the parameter to a method, or call a method on it, that is declared more than once in
//! the file (overloads, or methods of the exception classes) are taken to differ.
//!
//! Unsafe fix, for a clause identical to the one directly before it with the same parameter name: the types join
//! the earlier clause (`catch (A | B e)`) and the clause is removed. Not if the body assigns the parameter (a
//! multi-catch parameter is final), or with comments in the removed clause. Unsafe because a multi-catch of types of
//! which one extends the other does not compile, and types are needed to know.

use tree_sitter::Node;

use crate::lint::syntax::{has_comment, is_comment, is_name_reference_in, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(try_statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let clauses: Vec<Catch> = named_children(try_statement).iter().filter_map(|c| Catch::new(c, text)).collect();
    if clauses.len() < 2 {
        return;
    }
    // The methods declared more than once in the file, looked up when needed.
    let mut overloaded: Option<Vec<&str>> = None;
    let mut is_overloaded = |method: &str| {
        let methods = overloaded.get_or_insert_with(|| {
            let root = std::iter::successors(Some(*try_statement), |n| n.parent()).last().unwrap_or(*try_statement);
            overloaded_methods(&root, text)
        });
        methods.contains(&method)
    };
    // The equivalence classes, each the indexes of its clauses in order.
    let mut classes: Vec<Vec<usize>> = Vec::new();
    for (index, clause) in clauses.iter().enumerate() {
        let same = |class: &&mut Vec<usize>| clauses[class[0]].same_body(clause);
        let class = classes.iter_mut().find(same).filter(|_| !clause.called_methods.iter().any(|m| is_overloaded(m)));
        match class {
            Some(class) => class.push(index),
            None => classes.push(vec![index]),
        }
    }
    for class in classes {
        let first = &clauses[class[0]];
        for pair in class.windows(2) {
            let (previous, clause) = (&clauses[pair[0]], &clauses[pair[1]]);
            let message = format!("'catch' branch identical to '{}' branch", first.types_code);
            let diagnostic = Diagnostic::new(Rule::IdenticalCatch, clause.node.start_byte(), message);
            let fixable = pair[1] == pair[0] + 1
                && previous.name == clause.name
                && !has_comment(&clause.node)
                && !text[previous.node.end_byte()..clause.node.start_byte()].contains("/")
                && !assigns(&clause.body, clause.name, text);
            diagnostics.push(if fixable {
                let types = format!("{} | {}", previous.types_code, clause.types_code);
                diagnostic.with_unsafe_fix(vec![
                    Edit::new(previous.types.clone(), types),
                    Edit::new(previous.node.end_byte()..clause.node.end_byte(), ""),
                ])
            } else {
                diagnostic
            });
        }
    }
}

struct Catch<'t, 'a> {
    node: Node<'t>,
    name: &'a str,
    /// The range of the caught types (`A | B`), and their code.
    types: std::ops::Range<usize>,
    types_code: &'a str,
    body: Node<'t>,
    /// The body's tokens without comments.
    tokens: Vec<Node<'t>>,
    /// Whether each token is a reference to the parameter.
    parameter_references: Vec<bool>,
    /// The methods called with the parameter as receiver or argument.
    called_methods: Vec<&'a str>,
    text: &'a str,
}

impl<'t, 'a> Catch<'t, 'a> {
    fn new(clause: &Node<'t>, text: &'a str) -> Option<Catch<'t, 'a>> {
        if clause.kind() != "catch_clause" {
            return None;
        }
        let parameter = named_children(clause).into_iter().find(|c| c.kind() == "catch_formal_parameter")?;
        let name = parameter.child_by_field_name("name")?;
        let types = named_children(&parameter).into_iter().find(|c| c.kind() == "catch_type")?;
        let body = clause.child_by_field_name("body")?;
        let name = source(&name, text);
        let (mut tokens, mut parameter_references, mut called_methods) = (Vec::new(), Vec::new(), Vec::new());
        let mut cursor = body.walk();
        let mut ancestors = vec![body];
        'walk: loop {
            let node = cursor.node();
            if is_comment(&node) {
                // Skip the comment's subtree.
            } else if node.child_count() == 0 {
                let parent = ancestors[ancestors.len().saturating_sub(2)];
                let is_reference =
                    node.kind() == "identifier" && source(&node, text) == name && is_name_reference_in(&node, &parent);
                if is_reference {
                    let call = match parent.kind() {
                        "method_invocation" => Some(parent),
                        "argument_list" => ancestors.len().checked_sub(3).map(|i| ancestors[i]),
                        _ => None,
                    };
                    if let Some(method) = call.and_then(|c| c.child_by_field_name("name")) {
                        called_methods.push(source(&method, text));
                    }
                }
                tokens.push(node);
                parameter_references.push(is_reference);
            } else if cursor.goto_first_child() {
                ancestors.push(cursor.node());
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    *ancestors.last_mut().expect("the body") = cursor.node();
                    break;
                }
                if !cursor.goto_parent() || cursor.node() == body {
                    break 'walk;
                }
                ancestors.pop();
            }
        }
        Some(Catch {
            node: *clause,
            name,
            types: types.byte_range(),
            types_code: source(&types, text),
            body,
            tokens,
            parameter_references,
            called_methods,
            text,
        })
    }

    /// Whether the bodies are the same token by token, with the parameter names standing for each other.
    fn same_body(&self, other: &Catch) -> bool {
        self.tokens.len() == other.tokens.len()
            && (0..self.tokens.len()).all(|i| {
                let (a, b) = (self.tokens[i], other.tokens[i]);
                let (a_reference, b_reference) = (self.parameter_references[i], other.parameter_references[i]);
                a.kind_id() == b.kind_id()
                    && a_reference == b_reference
                    && (a_reference || source(&a, self.text) == source(&b, other.text))
            })
    }
}

/// The names of the methods declared more than once.
fn overloaded_methods<'a>(root: &Node, text: &'a str) -> Vec<&'a str> {
    let mut declared = std::collections::HashSet::new();
    let mut overloaded = Vec::new();
    crate::lint::syntax::for_each_descendant(root, &[], |node| {
        if node.kind() == "method_declaration"
            && let Some(name) = node.child_by_field_name("name")
            && !declared.insert(source(&name, text))
        {
            overloaded.push(source(&name, text));
        }
    });
    overloaded
}

/// Whether the body assigns the variable (`e = ...`, `e++`).
fn assigns(body: &Node, name: &str, text: &str) -> bool {
    let mut found = false;
    crate::lint::syntax::for_each_descendant(body, &[], |node| {
        let target = match node.kind() {
            "assignment_expression" => node.child_by_field_name("left"),
            "update_expression" => node.named_child(0),
            _ => None,
        };
        found |= target.is_some_and(|t| t.kind() == "identifier" && source(&t, text) == name);
    });
    found
}
