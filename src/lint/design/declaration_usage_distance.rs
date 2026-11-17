//! A014: a local variable declared more than `allowed-distance` (3) statements before its first use (Checkstyle
//! VariableDeclarationUsageDistance with its default properties: `ignoreFinal`, without `validateBetweenScopes`).
//!
//! As in Checkstyle, the distance counts the statements of the same block between the declaration and the statement
//! that uses the variable (other declarations do not count; a use in a nested block, also a labeled or
//! `synchronized` one, has distance 0, a use in a loop, `if` or `try` one more), a use is any identifier with the variable's name, and a variable initialized
//! before a sequence of calls on one object (`a.setX(); a.setY(); ... use(variable)`) is not reported.

use std::collections::HashSet;

use tree_sitter::Node;

use super::{declarator_names, has_keyword};
use crate::lint::syntax::{for_each_descendant, is_comment, named_children, source};
use crate::lint::{Diagnostic, Rule, RuleOptions};

/// Checks the variables declared in a block (or constructor body or `case` group).
pub fn check(block: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let statements: Vec<Node> =
        named_children(block).into_iter().filter(|s| !is_comment(s) && s.kind() != "switch_label").collect();
    // Computed lazily: most blocks declare no variable that needs them.
    let mut names_used: Option<Vec<HashSet<&str>>> = None;
    let allowed = options.declaration_usage_distance.allowed_distance;
    for (index, declaration) in statements.iter().enumerate() {
        if declaration.kind() != "local_variable_declaration" || has_keyword(declaration, text, "final") {
            continue;
        }
        let names_used = names_used.get_or_insert_with(|| statements.iter().map(|s| identifiers(s, text)).collect());
        let declarators: Vec<Node> =
            named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
        for (position, declarator) in declarators.iter().enumerate() {
            let Some(name) = declarator.child_by_field_name("name").map(|n| source(&n, text)) else { continue };
            if declarators[position + 1..].iter().any(|d| contains_identifier(d, name, text)) {
                continue;
            }
            let later = statements[index + 1..].iter().zip(&names_used[index + 1..]);
            // A sequence of calls may follow only the first variable of a declaration (see `usage_distance`), so
            // later variables of the declaration interrupt it.
            let is_last = position + 1 == declarators.len();
            let distance = usage_distance(later, name, is_last, text);
            if distance > allowed {
                let message = format!(
                    "Distance between variable '{name}' declaration and its first usage is {distance}, but allowed \
                     {allowed}"
                );
                diagnostics.push(Diagnostic::new(Rule::DeclarationUsageDistance, declaration.start_byte(), message));
            }
        }
    }
}

/// The number of statements up to and including the first that uses the variable, without declarations; 0 if the
/// variable is unused or first used in a nested block, which is another scope. 1 if the statements before it are
/// calls on one object, as when the object is initialized, and the first use is in another such call or a
/// declaration. As in Checkstyle, a declaration of several variables only belongs to such a sequence up to its first
/// variable.
fn usage_distance<'a, 't: 'a>(
    later: impl Iterator<Item = (&'a Node<'t>, &'a HashSet<&'a str>)>,
    name: &str,
    sequence_possible: bool,
    text: &str,
) -> u32 {
    let mut distance = 0;
    // The object of all calls so far, while the statements form a sequence of calls on one object.
    let mut sequence = sequence_possible.then_some(None::<String>);
    for (statement, names) in later {
        let is_declaration = statement.kind() == "local_variable_declaration";
        if !names.contains(name) {
            if is_declaration {
                if declarator_names(statement).len() > 1 {
                    sequence = None;
                }
                continue;
            }
            distance += 1;
            sequence = match (sequence, call_object(statement, text)) {
                (Some(None), Some(object)) => Some(Some(object)),
                (Some(Some(current)), Some(object)) if current == object => Some(Some(current)),
                _ => None,
            };
            continue;
        }
        if is_nested_block(statement) {
            return 0;
        }
        let continues_sequence = match &sequence {
            Some(current) => {
                (is_declaration && first_declarator_uses(statement, name, text))
                    || call_object(statement, text).is_some_and(|object| current.as_ref().is_none_or(|c| *c == object))
            }
            None => false,
        };
        return if continues_sequence { 1 } else { distance + 1 };
    }
    0
}

/// Whether the statement is a block, possibly labeled or `synchronized`.
fn is_nested_block(statement: &Node) -> bool {
    match statement.kind() {
        "block" | "synchronized_statement" => true,
        "labeled_statement" => named_children(statement).last().is_some_and(|s| is_nested_block(s)),
        _ => false,
    }
}

/// Whether the declaration uses the variable before its second variable.
fn first_declarator_uses(declaration: &Node, name: &str, text: &str) -> bool {
    let mut cursor = declaration.walk();
    let mut first = declaration.children(&mut cursor).take_while(|c| c.kind() != ",");
    first.any(|c| source(&c, text) == name || contains_identifier(&c, name, text))
}

/// The object (without whitespace) of a statement `object.method(...);`.
fn call_object(statement: &Node, text: &str) -> Option<String> {
    if statement.kind() != "expression_statement" {
        return None;
    }
    let call = named_children(statement).into_iter().find(|c| !is_comment(c))?;
    if call.kind() != "method_invocation" {
        return None;
    }
    let object = call.child_by_field_name("object")?;
    Some(source(&object, text).split_whitespace().collect())
}

fn identifiers<'a>(node: &Node, text: &'a str) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    for_each_descendant(node, &[], |n| {
        if matches!(n.kind(), "identifier" | "type_identifier") {
            names.insert(source(n, text));
        }
    });
    names
}

fn contains_identifier(node: &Node, name: &str, text: &str) -> bool {
    let mut found = false;
    for_each_descendant(node, &[], |n| {
        found = found || (matches!(n.kind(), "identifier" | "type_identifier") && source(n, text) == name);
    });
    found
}
