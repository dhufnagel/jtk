// VS Code integration: runs `jtk lsp` as the language server for Java files, for lint diagnostics, fixes and
// formatting.

import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions } from "vscode-languageclient/node";
import { initializationOptions, resolveBinary } from "./jtk";

let output: vscode.OutputChannel;
let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  output = vscode.window.createOutputChannel("jtk");
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("jtk.showOutput", () => output.show()),
    vscode.commands.registerCommand("jtk.restart", () => restart(context)),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("jtk")) {
        void restart(context);
      }
    }),
  );
  await start(context);
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("jtk");
  const binary = resolveBinary(config.get<string>("path", ""), context.extensionPath);
  const serverOptions: ServerOptions = { command: binary, args: ["lsp"] };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { language: "java", scheme: "file" },
      { language: "java", scheme: "untitled" },
    ],
    initializationOptions: initializationOptions((key, fallback) => config.get(key, fallback)),
    // The server reloads the configurations when one changes.
    synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher("**/jtk.toml") },
    outputChannel: output,
  };
  client = new LanguageClient("jtk", "jtk", serverOptions, clientOptions);
  try {
    await client.start();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    output.appendLine(`could not start ${binary} lsp: ${message}`);
    const choice = await vscode.window.showErrorMessage(
      `jtk could not be started (${binary}). Install jtk or set "jtk.path" in the settings.`,
      "Open Settings",
    );
    if (choice === "Open Settings") {
      void vscode.commands.executeCommand("workbench.action.openSettings", "jtk.path");
    }
  }
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await client?.stop();
  client = undefined;
  await start(context);
}
