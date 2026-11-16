// Running jtk and turning its output into edits. Independent of the VS Code API so that it can be tested with Node.

import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

export interface FormatSettings {
  sortImports: boolean;
  removeUnusedImports: boolean;
  reflowLongStrings: boolean;
  formatJavadoc: boolean;
  timeoutMs: number;
}

/** A replacement of the text between two offsets, in UTF-16 code units (like VS Code and jtk's offsets). */
export interface Edit {
  start: number;
  end: number;
  text: string;
}

/** A failure to format, with a message suitable for the user. */
export class JtkError extends Error {
  /** 1-based position of a syntax error, if jtk reported one. */
  readonly line?: number;
  readonly column?: number;

  constructor(message: string, line?: number, column?: number) {
    super(message);
    this.line = line;
    this.column = column;
  }
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

function settingArgs(settings: FormatSettings): string[] {
  const args: string[] = [];
  if (!settings.sortImports) {
    args.push("--skip-sorting-imports");
  }
  if (!settings.removeUnusedImports) {
    args.push("--skip-removing-unused-imports");
  }
  if (!settings.reflowLongStrings) {
    args.push("--skip-reflowing-long-strings");
  }
  if (settings.formatJavadoc) {
    args.push("--format-javadoc");
  }
  return args;
}

/** Arguments for formatting a whole document read from stdin. */
export function documentArgs(settings: FormatSettings): string[] {
  return [...settingArgs(settings), "-"];
}

/**
 * Arguments for formatting the range [start, end) (UTF-16 offsets) of a document read from stdin. jtk's character
 * ranges are closed, and an empty selection formats the code at the cursor.
 */
export function rangeArgs(settings: FormatSettings, start: number, end: number): string[] {
  const range = end > start ? `${start}:${end - 1}` : `${start}`;
  return [...settingArgs(settings), "--output-replacements", "--character-ranges", range, "-"];
}

export interface RunResult {
  code: number | null;
  stdout: string;
  stderr: string;
}

/** Runs jtk with `input` on stdin. */
export function run(binary: string, args: string[], input: string, timeoutMs: number, signal?: AbortSignal): Promise<RunResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ["pipe", "pipe", "pipe"], signal, timeout: timeoutMs });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", (error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") {
        reject(
          new JtkError(
            `jtk executable not found: ${binary}. Install jtk or set "jtk.path" in the settings.`,
          ),
        );
      } else if (error.name === "AbortError") {
        reject(new JtkError("formatting was cancelled"));
      } else {
        reject(new JtkError(`could not run ${binary}: ${error.message}`));
      }
    });
    child.on("close", (code, killSignal) => {
      if (killSignal !== null && code === null) {
        reject(new JtkError(`jtk did not finish within ${timeoutMs} ms`));
        return;
      }
      resolve({
        code,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      });
    });
    child.stdin.on("error", () => {
      // The process may exit before reading all input (e.g. on a usage error); the close handler reports it.
    });
    child.stdin.end(input, "utf8");
  });
}

/**
 * Checks jtk's result. jtk prints nothing on stderr when formatting succeeds; errors look like
 * `<stdin>:12:5: error: message`. (Like palantir-java-format, usage errors exit with 0, so stderr is what counts.)
 */
function checkResult(result: RunResult): string {
  const stderr = result.stderr.trim();
  if (stderr === "" && result.code === 0) {
    return result.stdout;
  }
  const first = stderr.split(/\r?\n/)[0] ?? "";
  const match = /^<stdin>:(\d+):(\d+): error: (.*)$/.exec(first);
  if (match) {
    return fail(new JtkError(`line ${match[1]}: ${match[3]}`, Number(match[1]), Number(match[2])));
  }
  const plain = /^<stdin>:\s*error: (.*)$/.exec(first);
  return fail(new JtkError(plain ? plain[1] : first || `jtk exited with code ${result.code}`));
}

function fail(error: JtkError): never {
  throw error;
}

/**
 * The smallest single edit turning `before` into `after` (common prefix and suffix are kept), so that the cursor and
 * folding are disturbed as little as possible. Returns no edit if the texts are equal.
 */
export function minimalEdit(before: string, after: string): Edit[] {
  if (before === after) {
    return [];
  }
  let prefix = 0;
  const max = Math.min(before.length, after.length);
  while (prefix < max && before.charCodeAt(prefix) === after.charCodeAt(prefix)) {
    prefix++;
  }
  let suffix = 0;
  while (
    suffix < max - prefix &&
    before.charCodeAt(before.length - 1 - suffix) === after.charCodeAt(after.length - 1 - suffix)
  ) {
    suffix++;
  }
  // Do not split a surrogate pair.
  if (prefix > 0 && isHighSurrogate(before.charCodeAt(prefix - 1))) {
    prefix--;
  }
  if (suffix > 0 && isLowSurrogate(before.charCodeAt(before.length - suffix))) {
    suffix--;
  }
  return [{ start: prefix, end: before.length - suffix, text: after.slice(prefix, after.length - suffix) }];
}

function isHighSurrogate(code: number): boolean {
  return code >= 0xd800 && code <= 0xdbff;
}

function isLowSurrogate(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}

interface JsonReplacement {
  replaceRange: { lowerEndpoint: number; upperEndpoint: number };
  replacementString: string;
}

/** Parses the output of `--output-replacements`. */
export function parseReplacements(json: string): Edit[] {
  const parsed = JSON.parse(json) as JsonReplacement[];
  return parsed.map((r) => ({
    start: r.replaceRange.lowerEndpoint,
    end: r.replaceRange.upperEndpoint,
    text: r.replacementString,
  }));
}

/** Formats a whole document (including import fixing and string reflow, as configured). */
export async function formatDocument(
  binary: string,
  text: string,
  settings: FormatSettings,
  signal?: AbortSignal,
): Promise<Edit[]> {
  const result = await run(binary, documentArgs(settings), text, settings.timeoutMs, signal);
  return minimalEdit(text, checkResult(result));
}

/** Formats the statements and declarations touching [start, end) (UTF-16 offsets). Imports are left alone. */
export async function formatRange(
  binary: string,
  text: string,
  start: number,
  end: number,
  settings: FormatSettings,
  signal?: AbortSignal,
): Promise<Edit[]> {
  const result = await run(binary, rangeArgs(settings, start, end), text, settings.timeoutMs, signal);
  const replacements = parseReplacements(checkResult(result));
  // Each replacement may span much more than what changes; shrink it to the changed part.
  return replacements.flatMap((r) =>
    minimalEdit(text.slice(r.start, r.end), r.text).map((e) => ({
      start: r.start + e.start,
      end: r.start + e.end,
      text: e.text,
    })),
  );
}
