#!/usr/bin/env python3
"""Writes THIRD-PARTY-LICENSES.md for the crates compiled into the jtk binary.

Usage: scripts/third-party-licenses.py [--target TRIPLE] > THIRD-PARTY-LICENSES.md

Only normal (runtime) dependencies are listed; build scripts and dev-dependencies are not part of the binary.
License texts are taken from the license files shipped in each crate's source.
"""

import argparse
import json
import os
import subprocess
import sys

LICENSE_PREFIXES = ("license", "licence", "copying", "unlicense", "notice")


def runtime_packages(target):
    cmd = ["cargo", "metadata", "--format-version", "1", "--locked"]
    if target:
        cmd += ["--filter-platform", target]
    meta = json.loads(subprocess.check_output(cmd))
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    seen, stack = set(), [meta["resolve"]["root"]]
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    root = packages[meta["resolve"]["root"]]["name"]
    return sorted((packages[p] for p in seen if packages[p]["name"] != root), key=lambda p: p["name"])


def license_files(package):
    directory = os.path.dirname(package["manifest_path"])
    files = []
    for name in sorted(os.listdir(directory)):
        path = os.path.join(directory, name)
        if os.path.isfile(path) and name.lower().startswith(LICENSE_PREFIXES):
            files.append((name, open(path, encoding="utf-8", errors="replace").read().strip()))
    return files


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target")
    args = parser.parse_args()
    out = sys.stdout
    out.write("# Third-party licenses\n\n")
    out.write("The jtk binary includes the following Rust crates. Their source code is available from crates.io\n")
    out.write("(https://crates.io/crates/<name>/<version>) and the repositories listed below.\n\n")
    out.write("| Crate | Version | License | Source |\n|---|---|---|---|\n")
    packages = runtime_packages(args.target)
    for p in packages:
        source = p.get("repository") or f"https://crates.io/crates/{p['name']}/{p['version']}"
        out.write(f"| {p['name']} | {p['version']} | {p['license'] or p.get('license_file') or '?'} | {source} |\n")
    out.write("\n## License texts\n")
    for p in packages:
        out.write(f"\n### {p['name']} {p['version']}\n")
        files = license_files(p)
        if not files:
            out.write(f"\nLicensed under {p['license']}; the crate ships no license file.\n")
        for name, text in files:
            out.write(f"\n`{name}`:\n\n```text\n{text}\n```\n")


if __name__ == "__main__":
    main()
