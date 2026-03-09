#!/usr/bin/env python3
import re
import sys
from pathlib import Path
from difflib import get_close_matches

ROOT = Path(".").resolve()
SPEC = ROOT / "specs" / "spec_ledger.yaml"

TEST_ATTR_RE = re.compile(r"#\[(?:test|tokio::test|rstest)\b")
FN_RE = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)\s*\(")
REF_RE = re.compile(r"(crates/[A-Za-z0-9_./-]+\.rs::[A-Za-z0-9_]+)")

def collect_rust_tests():
    tests = []
    for path in ROOT.rglob("*.rs"):
        rel = path.relative_to(ROOT).as_posix()
        try:
            lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
        except Exception:
            continue

        pending = False
        for line in lines:
            if TEST_ATTR_RE.search(line):
                pending = True
                continue
            if pending:
                m = FN_RE.search(line)
                if m:
                    tests.append((rel, m.group(1)))
                    pending = False
                elif line.strip() and not line.strip().startswith("#"):
                    pending = False
    return tests

def main(write=False):
    raw = SPEC.read_text(encoding="utf-8")
    refs = sorted(set(REF_RE.findall(raw)))
    tests = collect_rust_tests()

    by_name = {}
    by_path = {}
    for path, name in tests:
        by_name.setdefault(name, []).append(path)
        by_path.setdefault(path, set()).add(name)

    replacements = {}
    unresolved = []

    for ref in refs:
        path, name = ref.split("::", 1)

        if path in by_path and name in by_path[path]:
            continue

        # exact name in same crate
        crate_prefix = "/".join(path.split("/")[:2]) + "/"
        same_crate = [p for p in by_name.get(name, []) if p.startswith(crate_prefix)]

        if len(same_crate) == 1:
            replacements[ref] = f"{same_crate[0]}::{name}"
            continue

        # close-name search in same file
        file_names = sorted(by_path.get(path, []))
        close_in_file = get_close_matches(name, file_names, n=3, cutoff=0.6)

        # close-name search in same crate
        same_crate_names = sorted(
            {n for p, n in tests if p.startswith(crate_prefix)}
        )
        close_in_crate = get_close_matches(name, same_crate_names, n=5, cutoff=0.6)

        unresolved.append({
            "ref": ref,
            "close_in_file": close_in_file,
            "close_in_crate": close_in_crate,
            "exact_name_elsewhere": by_name.get(name, []),
        })

    print("\nPlanned replacements:\n")
    for old, new in sorted(replacements.items()):
        print(f"{old}\n  -> {new}\n")

    print("\nStill unresolved:\n")
    for item in unresolved:
        print(item["ref"])
        if item["close_in_file"]:
            print(f"  close_in_file:   {item['close_in_file']}")
        if item["close_in_crate"]:
            print(f"  close_in_crate:  {item['close_in_crate']}")
        if item["exact_name_elsewhere"]:
            print(f"  exact_elsewhere: {item['exact_name_elsewhere']}")
        print()

    if write and replacements:
        updated = raw
        for old, new in sorted(replacements.items(), key=lambda kv: -len(kv[0])):
            updated = updated.replace(old, new)
        SPEC.write_text(updated, encoding="utf-8")
        print("Applied replacements to specs/spec_ledger.yaml")

if __name__ == "__main__":
    main(write="--write" in sys.argv)
