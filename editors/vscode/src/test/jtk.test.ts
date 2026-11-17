// Tests finding jtk, and the language server of the real executable (target/release/jtk, or $JTK_BIN) as the
// extension's language client talks to it.

import * as assert from "node:assert/strict";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { after, before, describe, it } from "node:test";
import { MessageConnection, StreamMessageReader, StreamMessageWriter, createMessageConnection } from "vscode-jsonrpc/node";
import { initializationOptions, resolveBinary } from "../jtk";

const binary =
  process.env.JTK_BIN ??
  path.join(__dirname, "..", "..", "..", "..", "target", "release", process.platform === "win32" ? "jtk.exe" : "jtk");

describe("resolveBinary", () => {
  it("prefers the configured path, then the bundled binary, then the PATH", () => {
    assert.equal(resolveBinary(" /opt/jtk ", "/ext", "linux", "x64", () => true), "/opt/jtk");
    assert.equal(resolveBinary("", "/ext", "linux", "x64", () => true), path.join("/ext", "bin", "linux-x64", "jtk"));
    assert.equal(resolveBinary("", "/ext", "win32", "x64", () => false), "jtk.exe");
    assert.equal(resolveBinary("", "/ext", "darwin", "arm64", () => false), "jtk");
  });
});

describe("jtk lsp", () => {
  const uri = "file:///tmp/jtk-vscode-test/A.java";
  const child = spawn(binary, ["lsp"]);
  const connection: MessageConnection = createMessageConnection(
    new StreamMessageReader(child.stdout),
    new StreamMessageWriter(child.stdin),
  );
  const diagnostics: Promise<{ diagnostics: { code: string }[] }> = new Promise((resolve) =>
    connection.onNotification("textDocument/publishDiagnostics", resolve),
  );

  before(async () => {
    connection.listen();
    const options = initializationOptions(<T>(key: string, fallback: T) => (key === "sortImports" ? false : fallback) as T);
    await connection.sendRequest("initialize", { processId: null, rootUri: null, capabilities: {}, initializationOptions: options });
    await connection.sendNotification("initialized", {});
    const text = "import b.B;\nimport a.A;\nclass C { A a; B b;\n  void f(boolean x) { if (x) f(x); } }\n";
    await connection.sendNotification("textDocument/didOpen", {
      textDocument: { uri, languageId: "java", version: 1, text },
    });
  });

  after(async () => {
    await connection.sendRequest("shutdown");
    await connection.sendNotification("exit");
    connection.dispose();
  });

  it("publishes lint violations", async () => {
    assert.deepEqual((await diagnostics).diagnostics.map((d) => d.code), ["S001"]);
  });

  it("offers the fix-all action for saving", async () => {
    const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } };
    const actions: { title: string }[] = await connection.sendRequest("textDocument/codeAction", {
      textDocument: { uri },
      range,
      context: { diagnostics: [], only: ["source.fixAll"] },
    });
    assert.deepEqual(actions.map((a) => a.title), ["Fix all jtk violations"]);
  });

  it("formats with the client's settings", async () => {
    const edits: { newText: string }[] = await connection.sendRequest("textDocument/formatting", {
      textDocument: { uri },
      options: { tabSize: 4, insertSpaces: true },
    });
    assert.equal(edits.length, 1);
    assert.ok(edits[0].newText.startsWith("\nclass C {\n    A a;"), edits[0].newText);
  });
});
