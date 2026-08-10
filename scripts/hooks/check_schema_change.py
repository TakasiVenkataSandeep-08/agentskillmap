#!/usr/bin/env python3
"""PostToolUse hook: warn when a manifest-shape file changed.

Warn-only, per .claude/settings.json's rationale: fast local feedback, CI is the actual
gate, and a hook that fights the author gets disabled. This script never blocks a write —
it always exits 0, whether or not it has something to say.

Checked: Write/Edit targets under schema/ or exactly docs/02-manifest-schema.md. A manifest
shape change is a schema-version event (AGENTS.md definition of done) and every array in the
manifest needs a declared total order (docs/02-manifest-schema.md) — both are easy to miss
in an edit that otherwise looks like a small, locally reasonable change.
"""
import json
import sys


def normalize(path: str) -> str:
    return (path or "").replace("\\", "/")


def touches_schema(file_path: str) -> bool:
    parts = [p for p in file_path.split("/") if p]
    if "schema" in parts:
        return True
    if file_path.endswith("docs/02-manifest-schema.md"):
        return True
    return False


def check(payload: object) -> int:
    tool_input = payload.get("tool_input") if isinstance(payload, dict) else None
    if not isinstance(tool_input, dict):
        return 0

    raw_path = tool_input.get("file_path")
    file_path = normalize(raw_path) if isinstance(raw_path, str) else ""
    if not file_path or not touches_schema(file_path):
        return 0

    print(f"[check_schema_change] {file_path} changed.")
    print(
        "  A manifest shape change is a schema-version event (AGENTS.md, definition of "
        "done): bump schema_version and add a migration note in the same commit - not a "
        "follow-up."
    )
    print(
        "  Every array in the manifest needs a DECLARED TOTAL ORDER "
        "(docs/02-manifest-schema.md's sort-order table). An optional sort key with no "
        "explicit absence rule is a partial order, and a partial order is a nondeterminism "
        "bug that only shows up on the one input with a tie - it won't fail locally."
    )
    print("  (warn-only: this hook never blocks the write; fix before the DoD/CI gate.)")
    return 0


def main() -> int:
    # Deliberately broad, and wrapping everything rather than just the parse: this
    # hook is advisory, and a traceback on every Write and Edit because a payload
    # had an unexpected shape would make it worse than absent. See the sibling
    # note in check_library_lints.py.
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
        return check(payload)
    except Exception:
        return 0


if __name__ == "__main__":
    sys.exit(main())
