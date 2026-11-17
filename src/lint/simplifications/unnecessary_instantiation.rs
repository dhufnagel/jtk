//! S019: instances of `String` and the primitive wrappers created with `new` (PMD StringInstantiation,
//! PrimitiveWrapperInstantiation): `new String("x")` is `"x"`, `new Integer(5)` is better `Integer.valueOf(5)`,
//! which may reuse cached instances, and `new Boolean(true)` or `Boolean.valueOf(true)` is `Boolean.TRUE`.
//!
//! Like the replacements PMD suggests, the fixes give up object identity: code comparing the instances with `==` or
//! synchronizing on them behaves differently. Without type resolution, `new String(x)` is only reported if `x` is a
//! string literal, a concatenation with one, or a variable declared as `String` (a `char[]` or `byte[]` copy is
//! fine), and only the literal is fixed: `new String(null)` throws. `new Float(x)` is only fixed if `x` cannot be a
//! `double`, which `Float.valueOf` does not accept.

use tree_sitter::Node;

use super::comment_in;
use crate::lint::syntax::{declared_type, named_children, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

const WRAPPERS: &[&str] = &["Integer", "Long", "Short", "Byte", "Character", "Double", "Float"];

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "method_invocation" {
        check_boolean_value_of(node, text, diagnostics);
        return;
    }
    let Some(created) = node.child_by_field_name("type") else { return };
    let written = source(&created, text);
    let simple = written.strip_prefix("java.lang.").unwrap_or(written);
    if !(simple == "String" || simple == "Boolean" || WRAPPERS.contains(&simple))
        || simple == written && is_shadowed(simple, text)
        || named_children(node).iter().any(|c| c.kind() == "class_body")
        || node.child_by_field_name("type_arguments").is_some()
    {
        return;
    }
    let Some(arguments) = node.child_by_field_name("arguments") else { return };
    let arguments = named_children(&arguments);
    let keyword_and_type = node.start_byte()..created.end_byte();
    let can_rewrite = !comment_in(node, keyword_and_type.clone()) && node.child(0).is_some_and(|c| c.kind() == "new");
    match simple {
        "String" => check_string(node, &arguments, text, diagnostics),
        "Boolean" => {
            let [argument] = &arguments[..] else { return };
            let (shown, constant) = literal_value(argument, text);
            let message =
                format!("Do not use `new Boolean({shown})`, prefer `{}`", constant.unwrap_or("Boolean.valueOf"));
            let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, node.start_byte(), message);
            let fix = match constant {
                Some(constant) if !comment_in(node, node.byte_range()) => {
                    Some(Edit::new(node.byte_range(), format!("{}.{}", written, &constant["Boolean.".len()..])))
                }
                Some(_) => None,
                None => can_rewrite.then(|| Edit::new(keyword_and_type, format!("{written}.valueOf"))),
            };
            diagnostics.push(with_optional_fix(node, text, diagnostic, fix));
        }
        _ if WRAPPERS.contains(&simple) => {
            let message = format!("Do not use `new {simple}(...)`, prefer `{simple}.valueOf(...)`");
            let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, node.start_byte(), message);
            let accepts = match &arguments[..] {
                [argument] => simple != "Float" || is_float_compatible(argument, text),
                _ => false,
            };
            let fix = (can_rewrite && accepts).then(|| Edit::new(keyword_and_type, format!("{written}.valueOf")));
            diagnostics.push(with_optional_fix(node, text, diagnostic, fix));
        }
        _ => {}
    }
}

fn check_string(creation: &Node, arguments: &[Node], text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let message = "Avoid instantiating String objects; this is usually unnecessary";
    let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, creation.start_byte(), message);
    let fix = match arguments {
        [] => Some("\"\"".to_string()),
        [argument] if argument.kind() == "string_literal" => Some(source(argument, text).to_string()),
        [argument] if is_string(argument, text) => None,
        _ => return,
    };
    let fix = fix.filter(|_| !comment_in(creation, creation.byte_range())).map(|r| Edit::new(creation.byte_range(), r));
    diagnostics.push(with_optional_fix(creation, text, diagnostic, fix));
}

/// Whether the expression is known to be a `String`.
fn is_string(expression: &Node, text: &str) -> bool {
    let expression = strip_parentheses(expression);
    match expression.kind() {
        "string_literal" => true,
        "binary_expression" if operator(&expression, text) == "+" => ["left", "right"]
            .iter()
            .any(|side| expression.child_by_field_name(side).is_some_and(|s| is_string(&s, text))),
        "identifier" => declared_type(&expression, source(&expression, text), text)
            .is_some_and(|t| matches!(source(&t, text), "String" | "java.lang.String")),
        "array_access" => {
            let mut array = expression;
            let mut depth = 0;
            while array.kind() == "array_access" {
                let Some(inner) = array.child_by_field_name("array") else { return false };
                array = strip_parentheses(&inner);
                depth += 1;
            }
            array.kind() == "identifier"
                && declared_type(&array, source(&array, text), text).is_some_and(|t| {
                    let declared: String = source(&t, text).split_whitespace().collect();
                    declared.strip_prefix("String").is_some_and(|dims| dims == "[]".repeat(depth))
                })
        }
        "method_invocation" => {
            let name = expression.child_by_field_name("name").map_or("", |n| source(&n, text));
            STRING_METHODS.contains(&name)
        }
        _ => false,
    }
}

/// Methods that return a `String` whatever their receiver (`toString`) or on the receivers they are usually called
/// on (`String`, `System`).
const STRING_METHODS: &[&str] = &[
    "toString",
    "substring",
    "trim",
    "strip",
    "toUpperCase",
    "toLowerCase",
    "concat",
    "replace",
    "replaceAll",
    "replaceFirst",
    "intern",
    "getProperty",
    "getenv",
];

/// Whether a class of the `java.lang` name may be imported or declared in the file, so that the simple name means
/// another class.
fn is_shadowed(name: &str, text: &str) -> bool {
    let imported = text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("import ") && line.ends_with(&format!(".{name};")) && !line.contains("java.lang.")
    });
    imported
        || ["class ", "interface ", "enum ", "record "].iter().any(|keyword| {
            let declaration = format!("{keyword}{name}");
            text.match_indices(&declaration).any(|(i, _)| {
                !text[i + declaration.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
            })
        })
}

/// `Boolean.valueOf(true)` and `Boolean.valueOf("false")`, whose value is a constant.
fn check_boolean_value_of(call: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if call.child_by_field_name("name").is_none_or(|n| source(&n, text) != "valueOf") {
        return;
    }
    let Some(object) = call.child_by_field_name("object") else { return };
    let written = source(&object, text);
    if !(written == "java.lang.Boolean" || written == "Boolean" && !is_shadowed("Boolean", text))
        || call.child_by_field_name("type_arguments").is_some()
    {
        return;
    }
    let arguments = call.child_by_field_name("arguments").map(|a| named_children(&a)).unwrap_or_default();
    let [argument] = &arguments[..] else { return };
    if !matches!(argument.kind(), "string_literal" | "true" | "false") {
        return;
    }
    let (shown, constant) = literal_value(argument, text);
    let message = match constant {
        Some(constant) => format!("Do not use `Boolean.valueOf({shown})`, prefer `{constant}`"),
        None => format!("Do not use `Boolean.valueOf({shown})`, prefer `Boolean.valueOf`"),
    };
    let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, call.start_byte(), message);
    let fix = constant.filter(|_| !comment_in(call, call.byte_range())).map(|constant| {
        Edit::new(call.byte_range(), format!("{}.{}", source(&object, text), &constant["Boolean.".len()..]))
    });
    diagnostics.push(with_optional_fix(call, text, diagnostic, fix));
}

/// How PMD shows a `Boolean` argument, and the constant it equals if it is `true`, `false`, `"true"` or `"false"`.
fn literal_value(argument: &Node, text: &str) -> (&'static str, Option<&'static str>) {
    match (argument.kind(), source(argument, text)) {
        ("true", _) => ("true", Some("Boolean.TRUE")),
        ("false", _) => ("false", Some("Boolean.FALSE")),
        ("string_literal", "\"true\"") => ("\"true\"", Some("Boolean.TRUE")),
        ("string_literal", "\"false\"") => ("\"false\"", Some("Boolean.FALSE")),
        ("string_literal", _) => ("\"...\"", None),
        _ => ("...", None),
    }
}

/// Whether the argument of `new Float` is not a `double`, so that `Float.valueOf` accepts it.
fn is_float_compatible(argument: &Node, text: &str) -> bool {
    let argument = strip_parentheses(argument);
    match argument.kind() {
        "string_literal" | "character_literal" => true,
        "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal" => true,
        "decimal_floating_point_literal" | "hex_floating_point_literal" => {
            source(&argument, text).ends_with(['f', 'F'])
        }
        _ => false,
    }
}

/// The diagnostic with the fix, unless the instance is compared with `==` or `!=`, where the identity of the new
/// instance matters, or is a statement of its own, which a literal or constant cannot be.
fn with_optional_fix(node: &Node, text: &str, diagnostic: Diagnostic, fix: Option<Edit>) -> Diagnostic {
    let fix = fix.filter(|_| {
        node.parent().is_none_or(|p| {
            p.kind() != "expression_statement"
                && !(p.kind() == "binary_expression" && matches!(operator(&p, text), "==" | "!="))
        })
    });
    match fix {
        Some(fix) => diagnostic.with_fix(vec![fix]),
        None => diagnostic,
    }
}
