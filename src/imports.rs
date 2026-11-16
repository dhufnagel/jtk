//! Import handling: ordering (port of `ImportOrderer.java`, Google/Palantir style) and removal of unused imports
//! (port of `RemoveUnusedImports.java`).

use std::collections::HashSet;

use tree_sitter::Node;

use crate::input::{Tok, build_toks};
use crate::newlines;

// ------------------------------------------------------------------------------------------------------------------
// Ordering
// ------------------------------------------------------------------------------------------------------------------

struct Import {
    imported: String,
    is_static: bool,
    trailing: String,
}

struct Orderer<'a> {
    text: &'a str,
    toks: Vec<Tok>,
    line_separator: &'static str,
}

fn is_java_identifier_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

/// Reorders the imports in `text` (a complete Java program) in Palantir (= Google) style.
pub fn reorder_imports(text: &str) -> Result<String, String> {
    let Ok(mut toks) = build_toks(text) else {
        // Lexing errors are reported later by the formatter.
        return Ok(text.to_string());
    };
    // Stop at the first type declaration: imports must precede it.
    if let Some(stop) =
        toks.iter().position(|t| t.is_token && matches!(t.text.as_str(), "class" | "interface" | "enum"))
    {
        let position = toks[stop].position;
        toks.truncate(stop);
        toks.push(Tok { index: -1, text: String::new(), position, column: 0, is_token: true });
    }
    Orderer { text, toks, line_separator: newlines::guess_line_separator(text) }.reorder()
}

impl Orderer<'_> {
    fn token_at(&self, i: usize) -> &str {
        self.toks.get(i).map(|t| t.text.as_str()).unwrap_or("")
    }

    fn is_identifier(&self, i: usize) -> bool {
        self.token_at(i).chars().next().is_some_and(is_java_identifier_start)
    }

    fn is_space(&self, i: usize) -> bool {
        self.token_at(i).chars().next().is_some_and(|c| matches!(c, ' ' | '\t' | '\x0c'))
    }

    fn is_newline(&self, i: usize) -> bool {
        self.toks.get(i).is_some_and(Tok::is_newline)
    }

    fn is_slash_slash_comment(&self, i: usize) -> bool {
        self.toks.get(i).is_some_and(Tok::is_slash_slash_comment)
    }

    fn tok_string(&self, start: usize, end: usize) -> String {
        self.toks[start..end].iter().map(|t| t.text.as_str()).collect()
    }

    fn find_identifier(&self, start: usize) -> Option<usize> {
        (start..self.toks.len())
            .find(|&i| self.is_identifier(i) && matches!(self.token_at(i), "import" | "class" | "interface" | "enum"))
    }

    fn reorder(&self) -> Result<String, String> {
        let first_import = match self.find_identifier(0) {
            Some(i) if self.token_at(i) == "import" => i,
            _ => return Ok(self.text.to_string()),
        };
        let unindented_first_import =
            if first_import > 0 && self.is_space(first_import - 1) { first_import - 1 } else { first_import };
        let (imports, after_last_import) = self.scan_imports(first_import)?;
        if let Some(i) = self.find_identifier(after_last_import)
            && self.token_at(i) == "import"
        {
            return Err("Imports not contiguous (perhaps a comment separates them?)".into());
        }
        let mut result = String::new();
        let prefix = self.tok_string(0, unindented_first_import);
        result.push_str(&prefix);
        if !prefix.is_empty() && newlines::line_ending(&prefix).is_none() {
            result.push_str(self.line_separator);
            result.push_str(self.line_separator);
        }
        result.push_str(&self.reordered_imports_string(&imports));
        let mut tail = vec![self.tok_string(after_last_import, self.toks.len()).trim_start().to_string()];
        if let Some(last) = self.toks.last() {
            tail.push(self.text[last.position + last.text.len()..].to_string());
        }
        if tail.iter().any(|s| !s.is_empty()) {
            result.push_str(self.line_separator);
            for t in tail {
                result.push_str(&t);
            }
        }
        Ok(result)
    }

    fn scan_imports(&self, mut i: usize) -> Result<(Vec<Import>, usize), String> {
        let mut after_last_import = i;
        let mut imports: Vec<Import> = Vec::new();
        while i < self.toks.len() && self.token_at(i) == "import" {
            i += 1;
            if self.is_space(i) {
                i += 1;
            }
            let is_static = self.token_at(i) == "static";
            if is_static {
                i += 1;
                if self.is_space(i) {
                    i += 1;
                }
            }
            if !self.is_identifier(i) {
                return Err(format!("Unexpected token after import: {}", self.token_at(i)));
            }
            let (imported, next) = self.scan_imported(i)?;
            i = next;
            if self.is_space(i) {
                i += 1;
            }
            if self.token_at(i) != ";" {
                return Err("Expected ; after import".into());
            }
            while self.token_at(i) == ";" {
                i += 1;
            }
            let mut trailing = String::new();
            if self.is_space(i) {
                trailing.push_str(self.token_at(i));
                i += 1;
            }
            if self.is_newline(i) {
                trailing.push_str(self.token_at(i));
                i += 1;
            }
            while self.is_slash_slash_comment(i) {
                trailing.push_str(self.token_at(i));
                i += 1;
                if self.is_newline(i) {
                    trailing.push_str(self.token_at(i));
                    i += 1;
                }
            }
            // A sorted set: the first of several equal imports is kept.
            if !imports.iter().any(|x| x.is_static == is_static && x.imported == imported) {
                imports.push(Import { imported, is_static, trailing });
            }
            after_last_import = i;
            while self.is_newline(i) || self.is_space(i) {
                i += 1;
            }
        }
        imports.sort_by(|a, b| b.is_static.cmp(&a.is_static).then_with(|| java_cmp(&a.imported, &b.imported)));
        Ok((imports, after_last_import))
    }

    fn scan_imported(&self, start: usize) -> Result<(String, usize), String> {
        let mut i = start;
        let mut imported = String::new();
        loop {
            imported.push_str(self.token_at(i));
            i += 1;
            if self.token_at(i) != "." {
                return Ok((imported, i));
            }
            imported.push('.');
            i += 1;
            if self.token_at(i) == "*" {
                imported.push('*');
                return Ok((imported, i + 1));
            } else if !self.is_identifier(i) {
                return Err(format!("Could not parse imported name, at: {}", self.token_at(i)));
            }
        }
    }

    fn reordered_imports_string(&self, imports: &[Import]) -> String {
        let mut sb = String::new();
        let mut prev_static = imports.first().is_some_and(|i| i.is_static);
        for imp in imports {
            if prev_static && !imp.is_static {
                sb.push_str(self.line_separator);
            }
            sb.push_str("import ");
            if imp.is_static {
                sb.push_str("static ");
            }
            sb.push_str(&imp.imported);
            sb.push(';');
            if imp.trailing.trim_matches(|c: char| c <= ' ').is_empty() {
                sb.push_str(self.line_separator);
            } else {
                sb.push_str(&imp.trailing);
            }
            prev_static = imp.is_static;
        }
        sb
    }
}

/// Java's `String.compareTo` (UTF-16 code unit order).
fn java_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

// ------------------------------------------------------------------------------------------------------------------
// Removing unused imports
// ------------------------------------------------------------------------------------------------------------------

/// Removes unused imports. Imports used only in javadoc references are kept.
pub fn remove_unused_imports(contents: &str) -> String {
    let Some(tree) = crate::parse::parse(contents) else {
        return contents.to_string();
    };
    let root = tree.root_node();
    if root.has_error() {
        // Errors are reported by the formatter.
        return contents.to_string();
    }
    let mut used = HashSet::new();
    collect_used_names(root, contents, &mut used);
    let mut cursor = root.walk();
    let children: Vec<Node> = root.children(&mut cursor).collect();
    let package = children.iter().find(|c| c.kind() == "package_declaration").and_then(|p| {
        let mut cursor = p.walk();
        p.named_children(&mut cursor)
            .find(|c| matches!(c.kind(), "identifier" | "scoped_identifier"))
            .map(|n| contents[n.byte_range()].to_string())
    });
    let sep = newlines::guess_line_separator(contents);
    let mut replacements = Vec::new();
    for imp in children.iter().filter(|c| c.kind() == "import_declaration") {
        let mut cursor = imp.walk();
        let parts: Vec<Node> = imp.named_children(&mut cursor).filter(|c| !is_comment(c)).collect();
        let Some(name) = parts.first() else { continue };
        let is_wildcard = parts.iter().any(|p| p.kind() == "asterisk");
        let (qualifier, simple_name) = match name.kind() {
            "scoped_identifier" => (
                strip_ws(&contents[name.child_by_field_name("scope").unwrap().byte_range()]),
                contents[name.child_by_field_name("name").unwrap().byte_range()].to_string(),
            ),
            _ => (String::new(), contents[name.byte_range()].to_string()),
        };
        let qualifier = if is_wildcard { strip_ws(&contents[name.byte_range()]) } else { qualifier };
        let unused = if qualifier == "java.lang" || package.as_deref().map(strip_ws).as_deref() == Some(&qualifier) {
            true
        } else if is_wildcard {
            false
        } else {
            !used.contains(&simple_name)
        };
        if !unused {
            continue;
        }
        let mut end = imp.end_byte();
        if let Some(i) = contents[end..].find(|c| c != ' ') {
            end += i;
        }
        if end + sep.len() < contents.len() && contents[end..].starts_with(sep) {
            end += sep.len();
        }
        replacements.push((imp.start_byte(), end));
    }
    let mut result = contents.to_string();
    for (start, end) in replacements.into_iter().rev() {
        result.replace_range(start..end, "");
    }
    result
}

fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn is_comment(n: &Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

/// Is this identifier node one that javac represents as an `IdentifierTree` (a simple name reference)?
fn is_name_reference(n: &Node) -> bool {
    let Some(parent) = n.parent() else { return true };
    let is_field = |f: &str| parent.child_by_field_name(f).is_some_and(|c| c.id() == n.id());
    match parent.kind() {
        // Qualified names: only the leftmost segment is an identifier reference.
        "scoped_identifier" => !is_field("name"),
        "scoped_type_identifier" => {
            let mut cursor = parent.walk();
            parent
                .named_children(&mut cursor)
                .find(|c| !is_comment(c) && !c.kind().ends_with("annotation"))
                .map(|c| c.id())
                == Some(n.id())
        }
        "field_access" => !is_field("field"),
        "method_invocation" => !(is_field("name") && parent.child_by_field_name("object").is_some()),
        "method_reference" => {
            // The name after `::` is a member name.
            let mut cursor = parent.walk();
            let after_colons = parent.children(&mut cursor).skip_while(|c| c.kind() != "::").any(|c| c.id() == n.id());
            !after_colons
        }
        // Declarations
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration"
        | "method_declaration"
        | "constructor_declaration"
        | "compact_constructor_declaration"
        | "annotation_type_element_declaration"
        | "variable_declarator"
        | "formal_parameter"
        | "catch_formal_parameter"
        | "enum_constant"
        | "enhanced_for_statement"
        | "resource"
        | "instanceof_expression" => !is_field("name"),
        "element_value_pair" => !is_field("key"),
        "type_parameter" => {
            let mut cursor = parent.walk();
            parent.named_children(&mut cursor).find(|c| c.kind() == "type_identifier").map(|c| c.id()) != Some(n.id())
        }
        "lambda_expression"
        | "inferred_parameters"
        | "labeled_statement"
        | "break_statement"
        | "continue_statement" => false,
        "type_pattern" | "record_pattern_component" => {
            // The last identifier is the binding name.
            let mut cursor = parent.walk();
            parent.named_children(&mut cursor).last().map(|c| c.id()) != Some(n.id())
        }
        _ => true,
    }
}

fn collect_used_names(n: Node, src: &str, used: &mut HashSet<String>) {
    match n.kind() {
        "import_declaration" => return,
        "identifier" | "type_identifier" => {
            if is_name_reference(&n) {
                used.insert(src[n.byte_range()].to_string());
            }
            return;
        }
        "block_comment" if src[n.byte_range()].starts_with("/**") => {
            collect_javadoc_references(&src[n.byte_range()], used);
            return;
        }
        // Markdown documentation comments (Java 23): block tags as in javadoc, plus reference links like
        // `[Foo]`, `[text][Foo]` or `[Foo#bar(Baz)]`.
        "line_comment" if src[n.byte_range()].starts_with("///") => {
            let text = &src[n.byte_range()];
            collect_javadoc_references(text, used);
            collect_markdown_references(text, used);
            return;
        }
        _ => {}
    }
    let mut cursor = n.walk();
    for c in n.children(&mut cursor) {
        collect_used_names(c, src, used);
    }
}

/// Records names referenced from javadoc `{@link}`, `{@linkplain}`, `{@value}`, `@see`, `@throws` and `@exception`.
fn collect_javadoc_references(doc: &str, used: &mut HashSet<String>) {
    const TAGS: &[&str] = &["@linkplain", "@link", "@value", "@see", "@throws", "@exception", "@inheritDoc"];
    let mut rest = doc;
    while let Some(at) = rest.find('@') {
        rest = &rest[at..];
        let Some(tag) =
            TAGS.iter().find(|t| rest.starts_with(**t) && !rest[t.len()..].starts_with(|c: char| c.is_alphanumeric()))
        else {
            rest = &rest[1..];
            continue;
        };
        rest = rest[tag.len()..].trim_start_matches(|c: char| c.is_whitespace() || c == '*');
        if rest.starts_with('"') || rest.starts_with('<') {
            continue;
        }
        // Qualifier: everything up to `#`, `(`, whitespace or `}`.
        let qualifier_end =
            rest.find(|c: char| c == '#' || c == '(' || c == '}' || c.is_whitespace()).unwrap_or(rest.len());
        let qualifier = &rest[..qualifier_end];
        if let Some(first) = qualifier.split('.').next()
            && !first.is_empty()
        {
            used.insert(first.to_string());
        }
        let after = &rest[qualifier_end..];
        let member_end = after.find(|c: char| c == '(' || c == '}' || c.is_whitespace()).unwrap_or(after.len());
        if after[member_end..].starts_with('(')
            && let Some(close) = after[member_end..].find(')')
        {
            for param in after[member_end + 1..member_end + close].split(',') {
                let ty = param
                    .trim_start_matches(|c: char| c.is_whitespace() || c == '*')
                    .split(|c: char| c.is_whitespace() || c == '<' || c == '[')
                    .next()
                    .unwrap_or("");
                if let Some(first) = ty.trim_end_matches("...").split('.').next()
                    && !first.is_empty()
                {
                    used.insert(first.to_string());
                }
            }
        }
        rest = &rest[qualifier_end..];
    }
}

/// Records names used in markdown reference links: `[Ref]` or `[text][Ref]`.
fn collect_markdown_references(doc: &str, used: &mut HashSet<String>) {
    let mut rest = doc;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        let candidate = &rest[..close];
        // A reference: `Qualified.Name`, optionally `#member(Params)`, without spaces outside the parentheses.
        let head = candidate.split('(').next().unwrap_or("");
        if !head.is_empty()
            && !head.contains(char::is_whitespace)
            && head.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$' || c == '#')
        {
            let link = format!("@link {candidate}}}");
            collect_javadoc_references(&link, used);
        }
        rest = &rest[close..];
    }
}
