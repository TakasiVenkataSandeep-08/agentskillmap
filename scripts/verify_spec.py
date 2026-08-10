"""Spec verifier for skillaudit — the pre-Rust stand-in for T1.

This repository is pre-alpha: no Rust crate exists yet (`Cargo.toml` has
`members = []` on purpose, see AGENTS.md's build order). But the spine of the
project — the manifest schema and the invariants it must encode — already
exists as prose (`AGENTS.md`, `docs/02-manifest-schema.md`) and as a machine
schema (`schema/manifest-v1.schema.json`). This script is what proves those
two documents agree with each other and with the repository's own claims
about itself, before there is a Rust test suite to do it.

Every check here is a stand-in for a Rust test that task T1
(`skillaudit-core`: manifest types and canonical serialization, see
`docs/00-tasks.md`) is expected to port and supersede:

- Check 1 (schema legality) becomes a `#[test]` that loads and validates the
  JSON Schema at build time, or a `build.rs` assertion.
- Check 2 and 3 (doc example + negative/positive cases) become the
  round-trip and schema-conformance property tests described in T1's
  "Done when" clause.
- Check 4 (prose paths resolve) has no Rust equivalent — it stays a docs
  lint — but is included here because nothing else in the repository runs
  it yet.
- Check 5 (no CRLF) becomes the two-platform determinism CI test AGENTS.md
  invariant 2 requires once fixtures are consumed by Rust code.
- Check 6 (TOML/JSON/YAML parse) is subsumed by `cargo` itself once crates
  exist to parse their own config; until then, nothing else notices a
  malformed `rules/*.toml` or `.github/workflows/*.yml`.

Until T1 lands, this script is the only thing standing between "the spec
documents agree with each other" and "nobody checked."

Each check is independent, runs to completion regardless of earlier
failures, and reports its own pass/fail with enough detail to act on without
re-running anything. Exit code is 0 iff every check passed.
"""

from __future__ import annotations

import json
import argparse
import os
import re
import subprocess
import sys
import tomllib
from copy import deepcopy
from pathlib import Path

# jsonschema and yaml are imported inside the checks that need them, not here.
# The line-endings check is the one CI runs on two platforms, and it depends on
# nothing but the standard library and git — a module-level import would force
# every runner to install packages it never uses.

REPO_ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = REPO_ROOT / "schema" / "manifest-v1.schema.json"
DOC_PATH = REPO_ROOT / "docs" / "02-manifest-schema.md"

# Repo-relative paths that prose may legitimately reference before they exist.
# Each entry needs a named reason, because an unexplained allowlist entry is
# indistinguishable from a typo somebody gave up on:
#
#   policy.toml         docs/00-tasks.md "Known gaps" — T8 input, unspecified
#   skillaudit.lock     docs/00-tasks.md "Known gaps" — T8 input, one-line spec
#   rules/languages.toml  docs/00-tasks.md "Known gaps" — T4 input
#   run-meta.json       AGENTS.md invariant 2 — where run metadata goes instead
#                       of the manifest; nothing writes it until a CLI exists
#   crates/, npm/       ARCHITECTURE.md target layout; members are added per task
#   corpus/             output of task T3, produced at runtime, never committed
KNOWN_GAP_EXACT = {
    "policy.toml",
    "skillaudit.lock",
    "rules/languages.toml",
    "run-meta.json",
}
KNOWN_GAP_PREFIXES = ("corpus/", "npm/", "crates/")

# Top-level names that belong to this repo's own namespace, even though some
# of them (crates/, npm/, corpus/, policy.toml, skillaudit.lock,
# run-meta.json) don't exist on disk yet. A backticked token whose first path
# segment isn't in this set is a reference to something outside this repo
# (a URL, another GitHub repo, an npm scope, a generic filename like
# `SKILL.md` or `settings.json` used illustratively) and is not a path this
# check can meaningfully resolve.
EXTRA_KNOWN_TOP_LEVEL = {"crates", "npm", "corpus", "policy.toml", "skillaudit.lock", "run-meta.json"}

PATH_EXT_RE = re.compile(r"\.(md|json|toml|scm|py|rs|yml|yaml)$")
FENCE_RE = re.compile(r"```.*?```", re.DOTALL)
INLINE_CODE_RE = re.compile(r"`([^`\n]+)`")
PLACEHOLDER_CHARS = set("<>{}*")


class CheckResult:
    def __init__(self, name: str) -> None:
        self.name = name
        self.failures: list[str] = []

    def fail(self, message: str) -> None:
        self.failures.append(message)

    @property
    def ok(self) -> bool:
        return not self.failures


def load_schema() -> dict:
    with SCHEMA_PATH.open(encoding="utf-8") as fh:
        return json.load(fh)


def extract_doc_example() -> dict:
    text = DOC_PATH.read_text(encoding="utf-8")
    blocks = re.findall(r"```json\n(.*?)```", text, re.DOTALL)
    if len(blocks) != 1:
        raise AssertionError(
            f"expected exactly one fenced ```json block in {DOC_PATH.relative_to(REPO_ROOT)}, "
            f"found {len(blocks)}"
        )
    return json.loads(blocks[0])


# ---------------------------------------------------------------------------
# Check 1 — schema legality
# ---------------------------------------------------------------------------
def check_schema_legality(result: CheckResult) -> None:
    import jsonschema

    schema = load_schema()
    try:
        jsonschema.Draft202012Validator.check_schema(schema)
    except jsonschema.exceptions.SchemaError as exc:
        result.fail(f"schema/manifest-v1.schema.json is not a legal 2020-12 schema: {exc.message}")


# ---------------------------------------------------------------------------
# Check 2 — the doc example validates
# ---------------------------------------------------------------------------
def check_doc_example_validates(result: CheckResult) -> None:
    import jsonschema

    schema = load_schema()
    validator = jsonschema.Draft202012Validator(schema)
    try:
        example = extract_doc_example()
    except (AssertionError, json.JSONDecodeError) as exc:
        result.fail(str(exc))
        return
    errors = sorted(validator.iter_errors(example), key=lambda e: list(e.path))
    for err in errors:
        path = "/".join(str(p) for p in err.path) or "<root>"
        result.fail(f"doc example manifest fails schema at {path}: {err.message}")


# ---------------------------------------------------------------------------
# Check 3 — negative and positive mutation cases
# ---------------------------------------------------------------------------
def check_negative_positive_cases(result: CheckResult) -> None:
    import jsonschema

    schema = load_schema()
    validator = jsonschema.Draft202012Validator(schema)

    try:
        base = extract_doc_example()
    except (AssertionError, json.JSONDecodeError) as exc:
        result.fail(f"cannot build mutation cases: {exc}")
        return

    must_fail: dict[str, dict] = {}
    must_pass: dict[str, dict] = {}

    # -- MUST FAIL --------------------------------------------------------

    m = deepcopy(base)
    finding = deepcopy(m["advisory"]["findings"][0])
    m["advisory"] = {"enabled": False, "findings": [finding]}
    must_fail["advisory enabled:false but findings non-empty"] = m

    m = deepcopy(base)
    m["advisory"] = {
        "enabled": False,
        "findings": [],
        "model": base["advisory"]["model"],
        "prompt_sha256": base["advisory"]["prompt_sha256"],
    }
    must_fail["advisory enabled:false but carrying model/prompt_sha256"] = m

    m = deepcopy(base)
    m["advisory"] = {"enabled": True, "findings": []}
    must_fail["advisory enabled:true but missing model and prompt_sha256"] = m

    m = deepcopy(base)
    del m["capabilities"][0]["evidence"][0]["rule_id"]
    must_fail["capabilities evidence entry with rule_id deleted"] = m

    m = deepcopy(base)
    del m["instructions"][0]["evidence"][0]["snippet_sha256"]
    must_fail["instructions evidence entry with snippet_sha256 deleted"] = m

    m = deepcopy(base)
    m["advisory"]["findings"][0]["evidence"][0]["rule_id"] = "should.not.exist"
    must_fail["advisory.findings evidence entry carrying a rule_id"] = m

    m = deepcopy(base)
    m["capabilities"][0]["detail"]["urls"] = ["https://example.com"]
    must_fail["capabilities[0].detail with an undeclared key (urls)"] = m

    m = deepcopy(base)
    m["diagnostics"].append({"code": "unsupported_language", "file": "scripts/run.sh"})
    must_fail["diagnostics carrying a bundle-scoped code (unsupported_language)"] = m

    # -- MUST PASS ----------------------------------------------------------

    m = deepcopy(base)
    m["disclosure"]["declared_capabilities"] = ["pdf-generation"]
    must_pass["disclosure.declared_capabilities with a free-form string"] = m

    m = deepcopy(base)
    m["advisory"] = {"enabled": False, "findings": []}
    must_pass["advisory enabled:false with empty findings and no pinning"] = m

    for label, doc in must_fail.items():
        if validator.is_valid(doc):
            result.fail(f"expected schema to REJECT case but it ACCEPTED: {label}")

    for label, doc in must_pass.items():
        errors = list(validator.iter_errors(doc))
        if errors:
            detail = "; ".join(e.message for e in errors)
            result.fail(f"expected schema to ACCEPT case but it REJECTED: {label} ({detail})")


# ---------------------------------------------------------------------------
# Check 4 — prose paths resolve
# ---------------------------------------------------------------------------
def _looks_like_path(token: str) -> bool:
    if any(c in token for c in PLACEHOLDER_CHARS):
        return False
    return ("/" in token) or bool(PATH_EXT_RE.search(token))


def _is_known_gap(candidate: str) -> bool:
    if candidate in KNOWN_GAP_EXACT:
        return True
    # `candidate` has had any trailing slash stripped, so a bare directory
    # reference like `crates/` arrives as "crates" and would never match the
    # prefix "crates/". Match the bare directory name as well as anything under it.
    return any(
        candidate == prefix.rstrip("/") or candidate.startswith(prefix)
        for prefix in KNOWN_GAP_PREFIXES
    )


def check_prose_paths_resolve(result: CheckResult) -> None:
    known_top_level = {p.name for p in REPO_ROOT.iterdir()} | EXTRA_KNOWN_TOP_LEVEL

    # A known-gap entry that now exists is no longer a gap, and leaving it on the
    # allowlist permanently exempts a real path from checking. Without this, the
    # check silently weakens exactly as the repo grows: the moment T1 lands
    # crates/skillaudit-core/, every `crates/...` reference in every document
    # stops being verified.
    for gap in sorted(KNOWN_GAP_EXACT):
        if (REPO_ROOT / gap).exists():
            result.fail(f"known-gap allowlist entry `{gap}` now exists on disk; remove it from KNOWN_GAP_EXACT so references to it are checked normally")
    for prefix in KNOWN_GAP_PREFIXES:
        if (REPO_ROOT / prefix.rstrip("/")).exists():
            result.fail(f"known-gap allowlist prefix `{prefix}` now exists on disk; remove it from KNOWN_GAP_PREFIXES so references under it are checked normally")

    # Every markdown file that carries prose, not just root and docs/. The
    # .claude/ and .github/ files are among the densest path-referencing in the
    # repo, and omitting them means a dangling reference in the rule-author skill
    # passes silently.
    md_files = sorted(REPO_ROOT.glob("*.md"))
    for subdir in ("docs", ".claude", ".github"):
        md_files += sorted((REPO_ROOT / subdir).rglob("*.md"))

    for md_file in md_files:
        text = md_file.read_text(encoding="utf-8")
        text_no_fences = FENCE_RE.sub("", text)
        rel_source = md_file.relative_to(REPO_ROOT).as_posix()

        for match in INLINE_CODE_RE.finditer(text_no_fences):
            token = match.group(1).strip()
            if not token or not _looks_like_path(token):
                continue

            top_level = token.split("/", 1)[0]
            if top_level not in known_top_level:
                # Not a reference into this repo (URL, other repo, npm scope,
                # or a generic filename like SKILL.md used illustratively).
                #
                # KNOWN LIMITATION, accepted deliberately: gating on the first
                # segment means a typo IN the first segment is invisible —
                # `doc/02-manifest-schema.md` and `nonexistent-dir/thing.md` both
                # pass. A typo'd filename under a real directory is still caught,
                # which is the common case. The gate exists because without it the
                # check drowns in false positives on illustrative tokens
                # (`SKILL.md`, `settings.json`, `expected.json`) and on genuine
                # external references (`anthropics/skills`, `@skillaudit/linux-x64`),
                # and a check that cries wolf gets deleted.
                continue

            candidate = token.rstrip("/")
            if _is_known_gap(candidate):
                continue

            if not (REPO_ROOT / candidate).exists():
                result.fail(f"{rel_source} references `{token}`, which does not exist on disk and is not on the known-gap allowlist")


# ---------------------------------------------------------------------------
# Check 5 — no CRLF in tracked text files
# ---------------------------------------------------------------------------
def _git_tracked_files() -> list[str]:
    proc = subprocess.run(
        ["git", "ls-files"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return [line for line in proc.stdout.splitlines() if line]


def _git_binary_marked(paths: list[str]) -> set[str]:
    if not paths:
        return set()
    proc = subprocess.run(
        ["git", "check-attr", "--stdin", "binary"],
        cwd=REPO_ROOT,
        input="\n".join(paths),
        capture_output=True,
        text=True,
        check=True,
    )
    binary: set[str] = set()
    for line in proc.stdout.splitlines():
        # Format: "<path>: binary: <value>", where <path> itself may
        # contain ": " so only the last two fields are structural.
        parts = line.rsplit(": ", 2)
        if len(parts) != 3:
            continue
        path, _attr, value = parts
        if value.strip() == "set":
            binary.add(path)
    return binary


def check_no_crlf(result: CheckResult) -> None:
    try:
        tracked = _git_tracked_files()
        binary = _git_binary_marked(tracked)
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        result.fail(f"could not enumerate tracked files via git: {exc}")
        return

    for rel_path in tracked:
        if rel_path in binary:
            continue
        full_path = REPO_ROOT / rel_path
        try:
            data = full_path.read_bytes()
        except OSError as exc:
            result.fail(f"could not read tracked file {rel_path}: {exc}")
            continue
        if b"\r\n" in data:
            result.fail(f"{rel_path} contains CRLF line endings (breaks byte-identical hashing, invariant 2)")


# ---------------------------------------------------------------------------
# Check 6 — TOML, JSON, and YAML parse
# ---------------------------------------------------------------------------
def check_configs_parse(result: CheckResult) -> None:
    for path in sorted(REPO_ROOT.rglob("*.toml")):
        if ".git" in path.parts:
            continue
        rel = path.relative_to(REPO_ROOT).as_posix()
        try:
            with path.open("rb") as fh:
                tomllib.load(fh)
        except tomllib.TOMLDecodeError as exc:
            result.fail(f"{rel} is not valid TOML: {exc}")

    for path in sorted(REPO_ROOT.rglob("*.json")):
        if ".git" in path.parts:
            continue
        rel = path.relative_to(REPO_ROOT).as_posix()
        try:
            json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            result.fail(f"{rel} is not valid JSON: {exc}")

    import yaml

    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(REPO_ROOT.rglob(pattern)):
            if ".git" in path.parts:
                continue
            rel = path.relative_to(REPO_ROOT).as_posix()
            try:
                yaml.safe_load(path.read_text(encoding="utf-8"))
            except yaml.YAMLError as exc:
                result.fail(f"{rel} is not valid YAML: {exc}")


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------
# (short name for --only, human-readable name, function)
CHECKS = [
    ("schema", "schema legality (Draft 2020-12)", check_schema_legality),
    ("doc-example", "doc example validates against schema", check_doc_example_validates),
    ("mutations", "negative and positive mutation cases", check_negative_positive_cases),
    ("prose-paths", "prose paths resolve", check_prose_paths_resolve),
    ("line-endings", "no CRLF in tracked text files", check_no_crlf),
    ("configs-parse", "TOML/JSON/YAML files parse", check_configs_parse),
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Verify the manifest spec is internally consistent. "
        "Stands in for the Rust test suite until crates exist (task T1)."
    )
    parser.add_argument(
        "--only",
        metavar="CHECK",
        help="run a single named check instead of all of them. A CI job that "
        "reports one specific failure should run only that check, or a "
        "failure elsewhere gets reported under a job name that did not cause it.",
    )
    parser.add_argument("--list", action="store_true", help="print valid --only names and exit")
    args = parser.parse_args()

    if args.list:
        for short, name, _ in CHECKS:
            print(f"{short:15} {name}")
        return 0

    selected = CHECKS
    if args.only:
        selected = [c for c in CHECKS if c[0] == args.only]
        if not selected:
            valid = ", ".join(c[0] for c in CHECKS)
            print(f"unknown check {args.only!r}; valid names: {valid}", file=sys.stderr)
            return 2

    results: list[CheckResult] = []
    for _short, name, fn in selected:
        result = CheckResult(name)
        try:
            fn(result)
        except Exception as exc:  # a check crashing is still a failure, not a script crash
            result.fail(f"check raised an unexpected exception: {exc!r}")
        results.append(result)

    print("skillaudit spec verification")
    print("=" * 60)
    for result in results:
        status = "PASS" if result.ok else "FAIL"
        print(f"[{status}] {result.name}")
        for failure in result.failures:
            print(f"    - {failure}")
    print("=" * 60)

    passed = sum(1 for r in results if r.ok)
    total = len(results)
    print(f"{passed}/{total} checks passed")

    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main())
