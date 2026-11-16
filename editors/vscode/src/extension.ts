// VS Code integration: registers jtk as a formatter for Java documents and selections.

import * as vscode from "vscode";
import { Edit, FormatSettings, JtkError, formatDocument, formatRange, resolveBinary } from "./jtk";

let output: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("jtk");
  const selector: vscode.DocumentSelector = [
    { language: "java", scheme: "file" },
    { language: "java", scheme: "untitled" },
  ];
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("jtk.showOutput", () => output.show()),
    vscode.languages.registerDocumentFormattingEditProvider(selector, {
      provideDocumentFormattingEdits: (document, _options, token) =>
        format(context, document, token, (binary, settings, signal) =>
          formatDocument(binary, document.getText(), settings, signal),
        ),
    }),
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, {
      provideDocumentRangeFormattingEdits: (document, range, _options, token) =>
        format(context, document, token, (binary, settings, signal) =>
          formatRange(
            binary,
            document.getText(),
            document.offsetAt(range.start),
            document.offsetAt(range.end),
            settings,
            signal,
          ),
        ),
    }),
  );
}

export function deactivate(): void {}

function settingsFor(document: vscode.TextDocument): { path: string; settings: FormatSettings } {
  const config = vscode.workspace.getConfiguration("jtk", document);
  return {
    path: config.get<string>("path", ""),
    settings: {
      sortImports: config.get<boolean>("sortImports", true),
      removeUnusedImports: config.get<boolean>("removeUnusedImports", true),
      reflowLongStrings: config.get<boolean>("reflowLongStrings", true),
      timeoutMs: config.get<number>("timeout", 10000),
    },
  };
}

async function format(
  context: vscode.ExtensionContext,
  document: vscode.TextDocument,
  token: vscode.CancellationToken,
  run: (binary: string, settings: FormatSettings, signal: AbortSignal) => Promise<Edit[]>,
): Promise<vscode.TextEdit[]> {
  const { path, settings } = settingsFor(document);
  const binary = resolveBinary(path, context.extensionPath);
  const abort = new AbortController();
  const cancellation = token.onCancellationRequested(() => abort.abort());
  const version = document.version;
  try {
    const edits = await run(binary, settings, abort.signal);
    if (token.isCancellationRequested || document.version !== version) {
      return [];
    }
    return edits.map(
      (e) => new vscode.TextEdit(new vscode.Range(document.positionAt(e.start), document.positionAt(e.end)), e.text),
    );
  } catch (error) {
    if (!token.isCancellationRequested) {
      report(document, binary, error);
    }
    return [];
  } finally {
    cancellation.dispose();
  }
}

/**
 * Reports a failure without a popup, since formatting also runs on every save: syntax errors in code being edited
 * are expected. A missing executable is a setup problem and is shown as an error message.
 */
function report(document: vscode.TextDocument, binary: string, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  output.appendLine(`[${new Date().toISOString()}] ${vscode.workspace.asRelativePath(document.uri)}: ${message}`);
  if (error instanceof JtkError && message.startsWith("jtk executable not found")) {
    void vscode.window.showErrorMessage(message, "Open Settings").then((choice) => {
      if (choice === "Open Settings") {
        void vscode.commands.executeCommand("workbench.action.openSettings", "jtk.path");
      }
    });
    return;
  }
  vscode.window.setStatusBarMessage(`$(error) jtk: ${message}`, 8000);
  output.appendLine(`  (executable: ${binary})`);
}
