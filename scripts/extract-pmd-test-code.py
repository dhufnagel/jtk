#!/usr/bin/env python3
"""Writes the Java code of PMD's rule tests to files, as a corpus for scripts/compare-with-pmd.sh.

Usage: scripts/extract-pmd-test-code.py <pmd-repo> <out-dir>

PMD's rule tests are XML files (pmd-java/src/test/resources/net/sourceforge/pmd/lang/java/rule/<category>/xml/
<Rule>.xml) with one <test-code> per case. Each case becomes <out-dir>/<category>/<Rule>/case<N>.java.
"""

import pathlib
import sys
import xml.etree.ElementTree as ET

NS = "{http://pmd.sourceforge.net/rule-tests}"


def main(repo, out):
    rules = pathlib.Path(repo, "pmd-java/src/test/resources/net/sourceforge/pmd/lang/java/rule")
    count = 0
    for test_file in sorted(rules.glob("*/xml/*.xml")):
        root = ET.parse(test_file).getroot()
        fragments = {f.get("id"): f.text or "" for f in root.iter(NS + "code-fragment")}
        target = pathlib.Path(out, test_file.parent.parent.name, test_file.stem)
        for number, case in enumerate(root.iter(NS + "test-code"), 1):
            code = case.find(NS + "code")
            ref = case.find(NS + "code-ref")
            text = code.text if code is not None else fragments.get(ref.get("id")) if ref is not None else None
            if not text or not text.strip():
                continue
            target.mkdir(parents=True, exist_ok=True)
            (target / f"case{number}.java").write_text(text.strip("\n") + "\n")
            count += 1
    print(f"{count} test cases written to {out}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
