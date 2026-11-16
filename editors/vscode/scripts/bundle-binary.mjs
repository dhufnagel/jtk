// Builds jtk for the current platform and copies it (and the license) into the extension, for local packaging.
// Release builds for all platforms are left to CI.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const extensionDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoDir = join(extensionDir, "..", "..");
const exe = process.platform === "win32" ? "jtk.exe" : "jtk";

execFileSync("cargo", ["build", "--release", "--bin", "jtk"], { cwd: repoDir, stdio: "inherit" });

const target = join(extensionDir, "bin", `${process.platform}-${process.arch}`);
mkdirSync(target, { recursive: true });
copyFileSync(join(repoDir, "target", "release", exe), join(target, exe));
copyFileSync(join(repoDir, "LICENSE"), join(extensionDir, "LICENSE"));
console.log(`bundled ${join(target, exe)}`);
