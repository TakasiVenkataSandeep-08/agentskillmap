#!/usr/bin/env python3
"""PostToolUse hook: warn about invariant-10/invariant-2 violations in library crates.

Warn-only, per .claude/settings.json's rationale: fast local feedback, CI is the actual
gate, and a hook that fights the author gets disabled. This script never blocks a write —
it always exits 0, whether or not it has something to say.

Checked: Write/Edit targets under crates/ that are .rs files, excluding the CLI binary
crate (skillmap-cli, which is explicitly exempt per AGENTS.md invariant 10) and test
code. Flags unwrap(, expect(, panic!, and serde_json::to_string_pretty appearing in the
new content.

  - unwrap/expect/panic!/unchecked indexing are denied by lint in every library crate
    (invariant 10) because hostile input is the normal case here: a parser crash on a
    malformed skill bundle is a denial-of-service on someone's CI, not just an ugly stack
    trace.
  - serde_json::to_string_pretty must never escape into the codebase (invariant 2):
    canonicalize() in skillmap-core is the ONLY serialization path, because it's the one
    place sorted keys, declared array orders, LF, and no-BOM are enforced. Any other
    serialization call is a byte-identity leak waiting to happen.
"""
import json
import re
import sys


PATTERNS = [
    (re.compile(r"\bunwrap\("), "unwrap("),
    (re.compile(r"\bexpect\("), "expect("),
    (re.compile(r"panic!"), "panic!"),
    (re.compile(r"serde_json::to_string_pretty"), "serde_json::to_string_pretty"),
]

CRATE_PATH_RE = re.compile(r"(^|/)crates/([^/]+)/")

EXEMPT_CRATES = {"skillmap-cli"}


def normalize(path: str) -> str:
    return (path or "").replace("\\", "/")


def is_test_path(path_lower: str) -> bool:
    if "/tests/" in path_lower:
        return True
    if path_lower.endswith("_test.rs") or path_lower.endswith("tests.rs"):
        return True
    if "/test_" in path_lower:
        return True
    return False


def check(payload: object) -> int:
    tool_input = payload.get("tool_input") if isinstance(payload, dict) else None
    if not isinstance(tool_input, dict):
        return 0

    raw_path = tool_input.get("file_path")
    file_path = normalize(raw_path) if isinstance(raw_path, str) else ""
    if not file_path or not file_path.endswith(".rs"):
        return 0

    match = CRATE_PATH_RE.search(file_path)
    if not match:
        return 0

    crate_name = match.group(2)
    if crate_name in EXEMPT_CRATES:
        return 0

    if is_test_path(file_path.lower()):
        return 0

    # Write gives full new content in `content`; Edit gives the replacement in
    # `new_string`. Either way, this is the text that just landed on disk.
    content = tool_input.get("content")
    if not isinstance(content, str):
        content = tool_input.get("new_string")
    if not isinstance(content, str):
        return 0

    hits = [label for pattern, label in PATTERNS if pattern.search(content)]
    if not hits:
        return 0

    print(f"[check_library_lints] {file_path} (crate: {crate_name}): found {', '.join(hits)}")
    print(
        "  invariant 10: unwrap/expect/panic!/unchecked indexing are denied by lint in "
        "library crates - hostile input is the normal case here, and a parser panic on a "
        "malformed skill bundle is a denial-of-service on someone's CI, not this crate's "
        "call to make alone."
    )
    if "serde_json::to_string_pretty" in hits:
        print(
            "  invariant 2: canonicalize() in skillmap-core is the only serialization "
            "path (sorted keys, declared array orders, LF, no BOM). "
            "serde_json::to_string_pretty must never escape into the codebase outside it."
        )
    print("  (warn-only: this hook never blocks the write; fix before the DoD/CI gate.)")
    return 0


def main() -> int:
    # The catch is deliberately broad and wraps everything, not just the parse.
    # This hook is advisory: its whole value is fast feedback, and the moment a
    # payload shape it did not anticipate produces a traceback in the author's
    # session on every single Write and Edit, it stops being feedback and becomes
    # the author's problem. Failing silent is the correct failure mode here —
    # invariant 3's "never be silent" governs the SCANNER's findings about a
    # bundle, not a convenience hook's opinion about source it could not read.
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
        return check(payload)
    except Exception:
        return 0


if __name__ == "__main__":
    sys.exit(main())
