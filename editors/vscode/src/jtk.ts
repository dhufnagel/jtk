// Finding jtk and its settings. Independent of the VS Code API so that it can be tested with Node.

import * as fs from "node:fs";
import * as path from "node:path";

/** The settings the language server takes as initialization options, with their defaults. */
const SERVER_SETTINGS = {
  sortImports: true,
  removeUnusedImports: true,
  reflowLongStrings: true,
  formatJavadoc: false,
  unsafeFixes: false,
};

export type ServerSettings = typeof SERVER_SETTINGS;

/** The initialization options for `jtk lsp`, read from the `jtk.*` settings with `get`. */
export function initializationOptions(get: <T>(key: string, fallback: T) => T): ServerSettings {
  const options = { ...SERVER_SETTINGS };
  for (const key of Object.keys(options) as (keyof ServerSettings)[]) {
    options[key] = get(key, options[key]);
  }
  return options;
}

/**
 * Chooses the jtk executable: the configured path, else the binary bundled with the extension for this platform,
 * else `jtk` on the PATH.
 */
export function resolveBinary(
  configuredPath: string,
  extensionDir: string,
  platform: string = process.platform,
  arch: string = process.arch,
  exists: (file: string) => boolean = fs.existsSync,
): string {
  if (configuredPath.trim() !== "") {
    return configuredPath.trim();
  }
  const exe = platform === "win32" ? "jtk.exe" : "jtk";
  const bundled = path.join(extensionDir, "bin", `${platform}-${arch}`, exe);
  return exists(bundled) ? bundled : exe;
}
