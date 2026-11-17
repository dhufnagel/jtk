//! The language server, driven like an editor over an in-memory connection.

use std::thread::JoinHandle;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{Value, json};

const URI: &str = "file:///tmp/jtk-lsp-test/A.java";

/// `x == "a"` (B007, unsafe fix) and a missing brace (S001, safe fix).
const SOURCE: &str =
    "class A {\n    boolean f(String x) {\n        if (x == \"a\") return true;\n        return false;\n    }\n}\n";

struct Client {
    connection: Connection,
    server: Option<JoinHandle<()>>,
    next_id: i32,
}

impl Client {
    fn start(options: Value) -> Client {
        let (server, connection) = Connection::memory();
        let handle = std::thread::spawn(move || jtk::lsp::serve(&server).unwrap());
        let mut client = Client { connection, server: Some(handle), next_id: 0 };
        client.request("initialize", json!({ "capabilities": {}, "initializationOptions": options }));
        client.notify("initialized", json!({}));
        client
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.connection.sender.send(Request::new(id.clone(), method.into(), params).into()).unwrap();
        loop {
            match self.connection.receiver.recv().unwrap() {
                Message::Response(response) if response.id == id => {
                    return response.response_result.unwrap();
                }
                _ => {}
            }
        }
    }

    fn notify(&self, method: &str, params: Value) {
        self.connection.sender.send(Notification::new(method.into(), params).into()).unwrap();
    }

    /// The codes of the next published diagnostics.
    fn diagnostics(&self) -> Vec<String> {
        loop {
            if let Message::Notification(n) = self.connection.receiver.recv().unwrap()
                && n.method == "textDocument/publishDiagnostics"
            {
                let diagnostics = n.params["diagnostics"].as_array().unwrap().clone();
                return diagnostics.iter().map(|d| d["code"].as_str().unwrap().to_string()).collect();
            }
        }
    }

    fn open(&self, text: &str) {
        let document = json!({ "uri": URI, "languageId": "java", "version": 1, "text": text });
        self.notify("textDocument/didOpen", json!({ "textDocument": document }));
    }

    fn code_actions(&mut self, line: u32, only: Option<&str>) -> Vec<Value> {
        let range = json!({ "start": { "line": line, "character": 0 }, "end": { "line": line, "character": 200 } });
        let context = match only {
            Some(kind) => json!({ "diagnostics": [], "only": [kind] }),
            None => json!({ "diagnostics": [] }),
        };
        let params = json!({ "textDocument": { "uri": URI }, "range": range, "context": context });
        self.request("textDocument/codeAction", params).as_array().unwrap().clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        self.server.take().unwrap().join().unwrap();
    }
}

fn titles(actions: &[Value]) -> Vec<&str> {
    actions.iter().map(|a| a["title"].as_str().unwrap()).collect()
}

/// The text of the action's single edit.
fn new_text(action: &Value) -> &str {
    action["edit"]["changes"][URI][0]["newText"].as_str().unwrap()
}

#[test]
fn publishes_violations_on_open_and_change() {
    let client = Client::start(Value::Null);
    client.open(SOURCE);
    assert_eq!(client.diagnostics(), ["S001", "B007"]);
    let change = json!({
        "textDocument": { "uri": URI, "version": 2 },
        "contentChanges": [{ "text": "class A {}\n" }],
    });
    client.notify("textDocument/didChange", change);
    assert_eq!(client.diagnostics(), Vec::<String>::new());
}

#[test]
fn offers_fixes_suppressions_and_fix_all() {
    let mut client = Client::start(Value::Null);
    client.open(SOURCE);
    client.diagnostics();
    let actions = client.code_actions(2, None);
    assert_eq!(
        titles(&actions),
        [
            "Fix S001: 'if' body without braces",
            "Suppress S001 on this line",
            "Fix B007: String literal compared with == or != instead of equals() (unsafe)",
            "Suppress B007 on this line",
            "Fix all jtk violations",
        ]
    );
    assert_eq!(new_text(&actions[2]), "\"a\".equals(x)");
    assert_eq!(new_text(&actions[3]), " // jtk:ignore[B007]");
    // Fix-all on save applies only the safe fixes, unless the client enables the unsafe ones.
    let fix_all = client.code_actions(0, Some("source.fixAll"));
    assert_eq!(titles(&fix_all), ["Fix all jtk violations"]);
    assert!(!new_text(&fix_all[0]).contains("equals"));
    drop(client);

    let mut client = Client::start(json!({ "unsafeFixes": true }));
    client.open(SOURCE);
    client.diagnostics();
    let fix_all = client.code_actions(0, Some("source.fixAll.jtk"));
    assert!(new_text(&fix_all[0]).contains("\"a\".equals(x)"));
}

#[test]
fn formats_documents_and_ranges() {
    let mut client = Client::start(json!({ "sortImports": false }));
    client.open("import b.B;\nimport a.A;\nclass C { A a; B b;\n  int x=1;\n    int y=2; }\n");
    client.diagnostics();
    let options = json!({ "tabSize": 4, "insertSpaces": true });
    let edits =
        client.request("textDocument/formatting", json!({ "textDocument": { "uri": URI }, "options": options }));
    let edits = edits.as_array().unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["start"], json!({ "line": 2, "character": 0 }));
    assert_eq!(edits[0]["newText"], "\nclass C {\n    A a;\n    B b;\n    int x = 1;\n    int y = 2;\n");

    let range = json!({ "start": { "line": 3, "character": 0 }, "end": { "line": 3, "character": 8 } });
    let params = json!({ "textDocument": { "uri": URI }, "range": range, "options": options });
    let edits = client.request("textDocument/rangeFormatting", params);
    let range = json!({ "start": { "line": 2, "character": 19 }, "end": { "line": 3, "character": 10 } });
    assert_eq!(edits, json!([{ "range": range, "newText": "\n    int x = 1;" }]));
}

#[test]
fn positions_count_utf16_code_units() {
    let mut client = Client::start(Value::Null);
    // 𝄞 is two UTF-16 code units, ä one, but both more than one byte.
    client.open("class A {\n    String s = \"ä𝄞\"; boolean f(String x) { return x == \"a\"; }\n}\n");
    let range = loop {
        if let Message::Notification(n) = client.connection.receiver.recv().unwrap()
            && n.method == "textDocument/publishDiagnostics"
        {
            break n.params["diagnostics"][0]["range"].clone();
        }
    };
    // B007 is reported at `==`, after 53 code units (56 bytes).
    assert_eq!(range, json!({ "start": { "line": 1, "character": 53 }, "end": { "line": 1, "character": 54 } }));
    let actions = client.code_actions(1, None);
    let edit = &actions[0]["edit"]["changes"][URI][0];
    assert_eq!(
        (&edit["range"]["start"], &edit["newText"]),
        (&json!({ "line": 1, "character": 51 }), &json!("\"a\".equals(x)"))
    );
}
