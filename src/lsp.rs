//! `jtk lsp`: a language server over stdin and stdout. It reports the lint violations of open Java files, offers
//! their fixes, a fix-all action for saving and suppression comments as code actions, and formats documents and
//! ranges. The nearest `jtk.toml` of each file applies, as on the command line; the client's settings (sent as
//! initialization options) set the formatting steps and whether fix-all applies unsafe fixes.

use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics, ShowMessage,
};
use lsp_types::request::{CodeActionRequest, Formatting, RangeFormatting, Request as _};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentRangeFormattingParams, MessageType, NumberOrString, OneOf, Position,
    PublishDiagnosticsParams, Range, ServerCapabilities, ServerInfo, ShowMessageParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit,
};
use serde::Deserialize;
use serde_json::Value;

use crate::config::{Config, Configs, LintArguments};
use crate::formatter::{FormatOptions, format, format_replacements_with_reflow};
use crate::lint::{self, Diagnostic, Settings};

/// The kind of the action that applies all fixes, for `editor.codeActionsOnSave`.
const FIX_ALL: &str = "source.fixAll.jtk";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Serves on stdin and stdout until the client exits; returns the exit code.
pub fn run(err: &mut dyn Write) -> i32 {
    let (connection, io_threads) = Connection::stdio();
    let result = serve(&connection);
    drop(connection);
    let result = result.and_then(|()| io_threads.join().map_err(Into::into));
    match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "jtk lsp: {e}");
            1
        }
    }
}

/// Initializes the connection and handles messages until the client shuts the server down.
pub fn serve(connection: &Connection) -> Result<()> {
    let (id, params) = connection.initialize_start()?;
    let options = params.get("initializationOptions").cloned().unwrap_or(Value::Null);
    let client = ClientSettings::deserialize(&options).unwrap_or_default();
    let initialize = serde_json::json!({
        "capabilities": capabilities(),
        "serverInfo": ServerInfo { name: "jtk".into(), version: Some(env!("CARGO_PKG_VERSION").into()) },
    });
    connection.initialize_finish(id, initialize)?;
    let mut server =
        Server { connection, client, documents: HashMap::new(), configs: Configs::new(None)?, shown_error: None };
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                server.handle_request(request)?;
            }
            Message::Notification(notification) => server.handle_notification(notification)?,
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::QUICKFIX, CodeActionKind::new(FIX_ALL)]),
            ..CodeActionOptions::default()
        })),
        ..ServerCapabilities::default()
    }
}

/// The settings of the editor extension.
#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ClientSettings {
    sort_imports: bool,
    remove_unused_imports: bool,
    reflow_long_strings: bool,
    format_javadoc: bool,
    /// Whether the fix-all action also applies the unsafe fixes.
    unsafe_fixes: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        ClientSettings {
            sort_imports: true,
            remove_unused_imports: true,
            reflow_long_strings: true,
            format_javadoc: false,
            unsafe_fixes: false,
        }
    }
}

struct Server<'c> {
    connection: &'c Connection,
    client: ClientSettings,
    /// The text of each open document.
    documents: HashMap<Url, String>,
    configs: Configs,
    /// The configuration error last shown, which is not shown again on every change.
    shown_error: Option<String>,
}

impl Server<'_> {
    fn handle_request(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        let result = match request.method.as_str() {
            Formatting::METHOD => {
                let params: DocumentFormattingParams = serde_json::from_value(request.params)?;
                self.format(&params.text_document.uri, None).map(|edits| serde_json::to_value(edits).unwrap())
            }
            RangeFormatting::METHOD => {
                let params: DocumentRangeFormattingParams = serde_json::from_value(request.params)?;
                let range = Some(params.range);
                self.format(&params.text_document.uri, range).map(|edits| serde_json::to_value(edits).unwrap())
            }
            CodeActionRequest::METHOD => {
                let params: CodeActionParams = serde_json::from_value(request.params)?;
                Ok(serde_json::to_value(self.code_actions(&params)).unwrap())
            }
            _ => Err((ErrorCode::MethodNotFound, format!("unknown request: {}", request.method))),
        };
        let response = match result {
            Ok(value) => Response::new_ok(id, value),
            Err((code, message)) => Response::new_err(id, code as i32, message),
        };
        self.send(response.into())
    }

    fn handle_notification(&mut self, notification: Notification) -> Result<()> {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
                self.documents.insert(params.text_document.uri.clone(), params.text_document.text);
                self.publish(&params.text_document.uri)
            }
            DidChangeTextDocument::METHOD => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
                // With full synchronization, the last change holds the whole text.
                if let Some(change) = params.content_changes.into_iter().last() {
                    self.documents.insert(params.text_document.uri.clone(), change.text);
                }
                self.publish(&params.text_document.uri)
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;
                self.documents.remove(&params.text_document.uri);
                self.send_diagnostics(params.text_document.uri, Vec::new())
            }
            // A `jtk.toml` changed: the client watches them.
            DidChangeWatchedFiles::METHOD => {
                self.configs = Configs::new(None)?;
                self.shown_error = None;
                let uris: Vec<Url> = self.documents.keys().cloned().collect();
                uris.iter().try_for_each(|uri| self.publish(uri))
            }
            _ => Ok(()),
        }
    }

    /// Publishes the violations in a document. A document with syntax errors, which are left to the Java language
    /// server, gets none.
    fn publish(&mut self, uri: &Url) -> Result<()> {
        let diagnostics = match self.lint(uri) {
            Some((text, _, diagnostics)) => {
                let lines = LineIndex::new(&text);
                diagnostics.iter().map(|d| to_lsp_diagnostic(d, &text, &lines)).collect()
            }
            None => Vec::new(),
        };
        self.send_diagnostics(uri.clone(), diagnostics)
    }

    /// The document's text, its settings and its violations, unless it is excluded, has syntax errors or its
    /// configuration is invalid (which is shown to the user).
    fn lint(&mut self, uri: &Url) -> Option<(String, Settings, Vec<Diagnostic>)> {
        let text = self.documents.get(uri)?.clone();
        let path = uri.to_file_path().ok();
        let (config, settings) = match self.settings(path.as_ref()) {
            Ok(found) => found,
            Err(message) => {
                self.show_error(message);
                return None;
            }
        };
        if path.as_ref().is_some_and(|p| config.is_excluded(p)) {
            return None;
        }
        let diagnostics = lint::lint_file(&text, path.as_deref(), &settings).ok()?;
        Some((text, settings, diagnostics))
    }

    /// The configuration of a file (of the working directory for unsaved documents) and its lint settings.
    fn settings(&mut self, path: Option<&PathBuf>) -> std::result::Result<(std::sync::Arc<Config>, Settings), String> {
        let path = path.cloned().unwrap_or_else(|| PathBuf::from("Untitled.java"));
        let config = self.configs.for_file(&path)?;
        let settings = config.lint_settings(&path, &LintArguments::default())?;
        Ok((config, settings))
    }

    /// The quick fixes and suppressions of the violations in the requested range, and the fix-all action.
    fn code_actions(&mut self, params: &CodeActionParams) -> Vec<CodeActionOrCommand> {
        let uri = &params.text_document.uri;
        let Some((text, settings, diagnostics)) = self.lint(uri) else { return Vec::new() };
        let lines = LineIndex::new(&text);
        let wants = |kind: &CodeActionKind| {
            params.context.only.as_ref().is_none_or(|only| only.iter().any(|o| kind.as_str().starts_with(o.as_str())))
        };
        let mut actions = Vec::new();
        if wants(&CodeActionKind::QUICKFIX) {
            let (start, end) = (lines.offset(&text, params.range.start), lines.offset(&text, params.range.end));
            for diagnostic in diagnostics.iter().filter(|d| start <= d.offset && d.offset <= end) {
                let lsp_diagnostic = to_lsp_diagnostic(diagnostic, &text, &lines);
                if !diagnostic.fix.is_empty() {
                    let unsafe_note = if diagnostic.unsafe_fix { " (unsafe)" } else { "" };
                    let title = format!("Fix {}: {}{unsafe_note}", diagnostic.rule.code(), diagnostic.message);
                    let edit = replace_changed(&text, &lint::apply_fix(&text, diagnostic), &lines);
                    let action = action(title, CodeActionKind::QUICKFIX, uri, edit, Some(lsp_diagnostic.clone()));
                    actions.push(CodeAction { is_preferred: Some(!diagnostic.unsafe_fix), ..action });
                }
                let title = format!("Suppress {} on this line", diagnostic.rule.code());
                let edit = suppression_edit(&text, diagnostic, &lines);
                actions.push(action(title, CodeActionKind::QUICKFIX, uri, edit, Some(lsp_diagnostic)));
            }
        }
        let kind = CodeActionKind::new(FIX_ALL);
        let fixed = (wants(&kind) && diagnostics.iter().any(|d| !d.fix.is_empty()))
            .then(|| lint::fix_with(&text, &settings, self.client.unsafe_fixes).ok())
            .flatten()
            .filter(|fixed| *fixed != text);
        if let Some(fixed) = fixed {
            let edit = replace_changed(&text, &fixed, &lines);
            actions.push(action("Fix all jtk violations".into(), kind, uri, edit, None));
        }
        actions.into_iter().map(CodeActionOrCommand::CodeAction).collect()
    }

    /// Formats the document, or the given range of it, with the steps of its configuration.
    fn format(&mut self, uri: &Url, range: Option<Range>) -> std::result::Result<Vec<TextEdit>, (ErrorCode, String)> {
        let failed = |message: String| (ErrorCode::RequestFailed, message);
        let text = self.documents.get(uri).cloned().ok_or_else(|| failed(format!("unknown document: {uri}")))?;
        let path = uri.to_file_path().unwrap_or_else(|()| PathBuf::from("Untitled.java"));
        let config = self.configs.for_file(&path).map_err(failed)?;
        let options = config.format_options(self.format_options());
        let lines = LineIndex::new(&text);
        let error = |e: crate::formatter::FormatError| failed(format!("{}:{e}", path.display()));
        match range {
            None => {
                let formatted = format(&text, &options).map_err(error)?;
                Ok(if formatted == text { Vec::new() } else { vec![replace_changed(&text, &formatted, &lines)] })
            }
            Some(range) => {
                let byte_range = (lines.offset(&text, range.start), lines.offset(&text, range.end));
                let replacements = format_replacements_with_reflow(&text, &[byte_range], &options).map_err(error)?;
                let edit = |r: &crate::output::Replacement| TextEdit {
                    range: Range { start: lines.position(&text, r.start), end: lines.position(&text, r.end) },
                    new_text: r.text.clone(),
                };
                Ok(replacements.iter().map(edit).collect())
            }
        }
    }

    fn format_options(&self) -> FormatOptions {
        FormatOptions {
            sort_imports: self.client.sort_imports,
            remove_unused_imports: self.client.remove_unused_imports,
            reflow_long_strings: self.client.reflow_long_strings,
            format_javadoc: self.client.format_javadoc,
            ..FormatOptions::default()
        }
    }

    fn send_diagnostics(&self, uri: Url, diagnostics: Vec<lsp_types::Diagnostic>) -> Result<()> {
        let params = PublishDiagnosticsParams { uri, diagnostics, version: None };
        self.send(Notification::new(PublishDiagnostics::METHOD.into(), params).into())
    }

    fn show_error(&mut self, message: String) {
        if self.shown_error.as_ref() == Some(&message) {
            return;
        }
        self.shown_error = Some(message.clone());
        let params = ShowMessageParams { typ: MessageType::ERROR, message };
        let _ = self.send(Notification::new(ShowMessage::METHOD.into(), params).into());
    }

    fn send(&self, message: Message) -> Result<()> {
        self.connection.sender.send(message).map_err(Into::into)
    }
}

fn action(
    title: String,
    kind: CodeActionKind,
    uri: &Url,
    edit: TextEdit,
    diagnostic: Option<lsp_types::Diagnostic>,
) -> CodeAction {
    CodeAction {
        title,
        kind: Some(kind),
        diagnostics: diagnostic.map(|d| vec![d]),
        edit: Some(WorkspaceEdit { changes: Some(HashMap::from([(uri.clone(), vec![edit])])), ..Default::default() }),
        ..CodeAction::default()
    }
}

/// A violation is reported at an offset; the editor marks the word (or character) there.
fn to_lsp_diagnostic(diagnostic: &Diagnostic, text: &str, lines: &LineIndex) -> lsp_types::Diagnostic {
    let rest = &text[diagnostic.offset..];
    let word = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
    let length = if word > 0 { word } else { rest.chars().next().map_or(0, char::len_utf8) };
    let end = if rest[..length].contains(['\n', '\r']) { diagnostic.offset } else { diagnostic.offset + length };
    lsp_types::Diagnostic {
        range: Range { start: lines.position(text, diagnostic.offset), end: lines.position(text, end) },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(diagnostic.rule.code().into())),
        source: Some("jtk".into()),
        message: diagnostic.message.clone(),
        ..lsp_types::Diagnostic::default()
    }
}

/// One edit replacing the part of `text` that differs in `changed`.
fn replace_changed(text: &str, changed: &str, lines: &LineIndex) -> TextEdit {
    let prefix = common_length(text.char_indices(), changed.char_indices());
    let suffix = common_length(text[prefix..].char_indices().rev(), changed[prefix..].char_indices().rev());
    let end = text.len() - suffix;
    TextEdit {
        range: Range { start: lines.position(text, prefix), end: lines.position(text, end) },
        new_text: changed[prefix..changed.len() - suffix].to_string(),
    }
}

/// The length in bytes of the common start of two sequences of characters with their offsets (from the end, if
/// reversed).
fn common_length(a: impl Iterator<Item = (usize, char)>, b: impl Iterator<Item = (usize, char)>) -> usize {
    a.zip(b).take_while(|((_, x), (_, y))| x == y).map(|((_, c), _)| c.len_utf8()).sum()
}

/// Adds the rule to the line's `// jtk:ignore[…]` comment, or appends one.
fn suppression_edit(text: &str, diagnostic: &Diagnostic, lines: &LineIndex) -> TextEdit {
    let code = diagnostic.rule.code();
    let line_end = text[diagnostic.offset..].find(['\n', '\r']).map_or(text.len(), |i| diagnostic.offset + i);
    let line_start = text[..diagnostic.offset].rfind(['\n', '\r']).map_or(0, |i| i + 1);
    let line = &text[line_start..line_end];
    let (offset, new_text) = match line.find("jtk:ignore[").and_then(|i| line[i..].find(']').map(|j| i + j)) {
        Some(bracket) => (line_start + bracket, format!(", {code}")),
        None => (line_end, format!(" // jtk:ignore[{code}]")),
    };
    let position = lines.position(text, offset);
    TextEdit { range: Range { start: position, end: position }, new_text }
}

/// Converts between byte offsets and LSP positions: lines and UTF-16 code units.
struct LineIndex {
    /// The byte offset at which each line starts.
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> LineIndex {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        for (i, &b) in bytes.iter().enumerate() {
            let ends_line = b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
            if ends_line {
                starts.push(i + 1);
            }
        }
        LineIndex { starts }
    }

    fn position(&self, text: &str, offset: usize) -> Position {
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let character = text[self.starts[line]..offset].encode_utf16().count();
        Position { line: line as u32, character: character as u32 }
    }

    /// The byte offset of a position; positions past the end of a line or the text are clamped to it.
    fn offset(&self, text: &str, position: Position) -> usize {
        let Some(&start) = self.starts.get(position.line as usize) else { return text.len() };
        let end = self.starts.get(position.line as usize + 1).map_or(text.len(), |&s| s);
        let line = text[start..end].trim_end_matches(['\n', '\r']);
        let mut units = 0;
        for (i, c) in line.char_indices() {
            if units >= position.character as usize {
                return start + i;
            }
            units += c.len_utf16();
        }
        start + line.len()
    }
}
