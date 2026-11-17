// Smoke test of the VS Code integration against a minimal stand-in for the `vscode` module.

import * as assert from "node:assert/strict";
import * as path from "node:path";
import { describe, it } from "node:test";

/* eslint-disable @typescript-eslint/no-explicit-any */
const Module = require("node:module") as any;

const binary =
  process.env.JTK_BIN ??
  path.join(__dirname, "..", "..", "..", "..", "target", "release", process.platform === "win32" ? "jtk.exe" : "jtk");

class Position {
  constructor(
    readonly line: number,
    readonly character: number,
  ) {}
}
class Range {
  constructor(
    readonly start: Position,
    readonly end: Position,
  ) {}
}
class TextEdit {
  constructor(
    readonly range: Range,
    readonly newText: string,
  ) {}
}

const registered: { document?: any; range?: any } = {};
const statusMessages: string[] = [];
const errorMessages: string[] = [];
let config: Record<string, unknown> = {};

const vscodeStub = {
  Position,
  Range,
  TextEdit,
  window: {
    createOutputChannel: () => ({ appendLine() {}, show() {}, dispose() {} }),
    setStatusBarMessage: (message: string) => statusMessages.push(message),
    showErrorMessage: (message: string) => {
      errorMessages.push(message);
      return Promise.resolve(undefined);
    },
  },
  commands: { registerCommand: () => ({ dispose() {} }), executeCommand: () => Promise.resolve() },
  languages: {
    registerDocumentFormattingEditProvider: (_selector: unknown, provider: unknown) => {
      registered.document = provider;
      return { dispose() {} };
    },
    registerDocumentRangeFormattingEditProvider: (_selector: unknown, provider: unknown) => {
      registered.range = provider;
      return { dispose() {} };
    },
  },
  workspace: {
    getConfiguration: () => ({ get: (key: string, fallback: unknown) => (key in config ? config[key] : fallback) }),
    asRelativePath: (uri: { path: string }) => uri.path,
  },
};

const originalLoad = Module._load;
Module._load = function (request: string, ...rest: unknown[]) {
  return request === "vscode" ? vscodeStub : originalLoad.call(this, request, ...rest);
};
const extension = require("../extension");

/** A text document over a string, with UTF-16 offsets like VS Code. */
function document(text: string): any {
  const lineStarts = [0];
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "\n") {
      lineStarts.push(i + 1);
    }
  }
  return {
    uri: { path: "/test/A.java" },
    version: 1,
    getText: () => text,
    positionAt(offset: number) {
      let line = 0;
      while (line + 1 < lineStarts.length && lineStarts[line + 1] <= offset) {
        line++;
      }
      return new Position(line, offset - lineStarts[line]);
    },
    offsetAt: (p: Position) => lineStarts[p.line] + p.character,
  };
}

function applyEdits(doc: any, edits: TextEdit[]): string {
  let text: string = doc.getText();
  const offsets = edits.map((e) => ({ start: doc.offsetAt(e.range.start), end: doc.offsetAt(e.range.end), text: e.newText }));
  for (const e of offsets.sort((a, b) => b.start - a.start)) {
    text = text.slice(0, e.start) + e.text + text.slice(e.end);
  }
  return text;
}

const token = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() {} }) };
const context = { subscriptions: [] as unknown[], extensionPath: "/nonexistent-extension-dir" };

describe("extension", () => {
  extension.activate(context);

  it("registers document and range formatters", () => {
    assert.ok(registered.document && registered.range);
  });

  it("formats a document", async () => {
    config = { path: binary };
    const doc = document("class A {\n  int x=1;\n}\n");
    const edits: TextEdit[] = await registered.document.provideDocumentFormattingEdits(doc, {}, token);
    assert.equal(applyEdits(doc, edits), "class A {\n    int x = 1;\n}\n");
    // The edit is minimal: the unchanged start of the document is not rewritten.
    assert.ok(edits.every((e) => e.range.start.line >= 1), JSON.stringify(edits));
  });

  it("reports syntax errors in the status bar and leaves the document alone", async () => {
    config = { path: binary };
    const edits = await registered.document.provideDocumentFormattingEdits(document("class {\n"), {}, token);
    assert.deepEqual(edits, []);
    assert.match(statusMessages.at(-1) ?? "", /jtk: line 1: /);
    assert.equal(errorMessages.length, 0);
  });

  it("shows an error message if jtk cannot be found", async () => {
    config = { path: "/nonexistent/jtk" };
    const edits = await registered.document.provideDocumentFormattingEdits(document("class A {}\n"), {}, token);
    assert.deepEqual(edits, []);
    assert.match(errorMessages.at(-1) ?? "", /jtk executable not found/);
  });

  it("formats a selection", async () => {
    config = { path: binary };
    const doc = document("class A {\n  int x=1;\n    int y=2;\n}\n");
    const range = new Range(new Position(2, 4), new Position(2, 9));
    const edits: TextEdit[] = await registered.range.provideDocumentRangeFormattingEdits(doc, range, {}, token);
    assert.equal(applyEdits(doc, edits), "class A {\n  int x=1;\n    int y = 2;\n}\n");
  });
});
