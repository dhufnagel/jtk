// Tests the jtk integration against the real executable (target/release/jtk, or $JTK_BIN).

import * as assert from "node:assert/strict";
import * as path from "node:path";
import { describe, it } from "node:test";
import {
  Edit,
  FormatSettings,
  JtkError,
  formatDocument,
  formatRange,
  minimalEdit,
  rangeArgs,
  resolveBinary,
} from "../jtk";

const binary =
  process.env.JTK_BIN ??
  path.join(__dirname, "..", "..", "..", "..", "target", "release", process.platform === "win32" ? "jtk.exe" : "jtk");
const settings: FormatSettings = {
  sortImports: true,
  removeUnusedImports: true,
  reflowLongStrings: true,
  formatJavadoc: false,
  timeoutMs: 10000,
};

const UNFORMATTED = "class A {\n  int x=1;\n    int y=2;\n}\n";
const FORMATTED = "class A {\n    int x = 1;\n    int y = 2;\n}\n";

function apply(text: string, edits: Edit[]): string {
  return [...edits].sort((a, b) => b.start - a.start).reduce((t, e) => t.slice(0, e.start) + e.text + t.slice(e.end), text);
}

describe("resolveBinary", () => {
  it("prefers the configured path, then the bundled binary, then the PATH", () => {
    assert.equal(resolveBinary(" /opt/jtk ", "/ext", "linux", "x64", () => true), "/opt/jtk");
    assert.equal(resolveBinary("", "/ext", "linux", "x64", () => true), path.join("/ext", "bin", "linux-x64", "jtk"));
    assert.equal(resolveBinary("", "/ext", "win32", "x64", () => false), "jtk.exe");
    assert.equal(resolveBinary("", "/ext", "darwin", "arm64", () => false), "jtk");
  });
});

describe("rangeArgs", () => {
  it("converts the half-open selection into jtk's closed character range", () => {
    assert.deepEqual(rangeArgs(settings, 10, 20).slice(-3), ["--character-ranges", "10:19", "-"]);
    assert.deepEqual(rangeArgs(settings, 7, 7).slice(-3), ["--character-ranges", "7", "-"]);
    assert.ok(rangeArgs({ ...settings, sortImports: false }, 0, 1).includes("--skip-sorting-imports"));
  });
});

describe("minimalEdit", () => {
  it("keeps the common prefix and suffix", () => {
    assert.deepEqual(minimalEdit("abcXdef", "abcYYdef"), [{ start: 3, end: 4, text: "YY" }]);
    assert.deepEqual(minimalEdit("same", "same"), []);
    assert.equal(apply(UNFORMATTED, minimalEdit(UNFORMATTED, FORMATTED)), FORMATTED);
  });

  it("does not split surrogate pairs", () => {
    const [edit] = minimalEdit("x\u{1D11E}y", "x\u{1D120}y");
    assert.deepEqual(edit, { start: 1, end: 3, text: "\u{1D120}" });
  });
});

describe("formatDocument", () => {
  it("formats a document", async () => {
    assert.equal(apply(UNFORMATTED, await formatDocument(binary, UNFORMATTED, settings)), FORMATTED);
  });

  it("returns no edits for a formatted document", async () => {
    assert.deepEqual(await formatDocument(binary, FORMATTED, settings), []);
  });

  it("keeps CRLF line endings", async () => {
    const crlf = UNFORMATTED.replace(/\n/g, "\r\n");
    assert.equal(apply(crlf, await formatDocument(binary, crlf, settings)), FORMATTED.replace(/\n/g, "\r\n"));
  });

  it("honours the import settings", async () => {
    const text = "import b.B;\nimport a.A;\nclass C { A a; B b; }\n";
    const sorted = apply(text, await formatDocument(binary, text, settings));
    assert.ok(sorted.startsWith("import a.A;\nimport b.B;\n"), sorted);
    const kept = apply(text, await formatDocument(binary, text, { ...settings, sortImports: false }));
    assert.ok(kept.startsWith("import b.B;\nimport a.A;\n"), kept);
  });

  it("formats javadoc only when enabled", async () => {
    const text = "/**\n * Short   text.\n */\nclass A {}\n";
    assert.deepEqual(await formatDocument(binary, text, settings), []);
    const formatted = apply(text, await formatDocument(binary, text, { ...settings, formatJavadoc: true }));
    assert.equal(formatted, "/** Short text. */\nclass A {}\n");
  });

  it("reports syntax errors with their position", async () => {
    await assert.rejects(formatDocument(binary, "class {\n", settings), (e: unknown) => {
      assert.ok(e instanceof JtkError);
      assert.equal(e.line, 1);
      assert.match(e.message, /^line 1: /);
      return true;
    });
  });

  it("explains a missing executable", async () => {
    await assert.rejects(formatDocument("/nonexistent/jtk", FORMATTED, settings), /jtk executable not found/);
  });

  it("can be cancelled", async () => {
    const abort = new AbortController();
    abort.abort();
    await assert.rejects(formatDocument(binary, UNFORMATTED, settings, abort.signal), /cancelled/);
  });
});

describe("formatRange", () => {
  it("formats only the selected statements", async () => {
    const start = UNFORMATTED.indexOf("    int y");
    const edits = await formatRange(binary, UNFORMATTED, start, start + 5, settings);
    assert.equal(apply(UNFORMATTED, edits), "class A {\n  int x=1;\n    int y = 2;\n}\n");
  });

  it("uses UTF-16 offsets", async () => {
    // Only the selected declaration is formatted, not the one before it on the same line (same as
    // palantir-java-format 2.103.0 with --character-ranges).
    const text = "class U {\n  String s=\"ä€\u{1D11E}\";   int  z=1;\n}\n";
    const start = text.indexOf("int  z");
    const edits = await formatRange(binary, text, start, start + 3, settings);
    assert.equal(apply(text, edits), "class U {\n  String s=\"ä€\u{1D11E}\";\n    int z = 1;\n}\n");
  });
});
