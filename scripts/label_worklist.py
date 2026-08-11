#!/usr/bin/env python3
"""Extract the sampled bundles into a readable worklist for labelling.

`corpus/sample.json` names which bundles to label; the bytes live under
`corpus/raw/<digest>/`, which is gitignored and only exists on a machine that
ran the harvest. This walks the sample and prints each bundle's description and
its analysable source, so a labeller reads the same thing every time.

Two things it deliberately does NOT do:

  - It does not run skillmap, and it does not read a manifest. A label derived
    from the scanner's own output makes precision 1.0 by construction and the
    whole exercise decoration. The labeller reads source; the scoring step in
    `skillmap-eval` compares the two afterwards.

  - It does not summarise, filter, or highlight. Showing a labeller only the
    lines a lexical marker matched would bias every judgement toward the
    marker's view, and the false-positive rate is exactly the number that bias
    would corrupt.

Content under `corpus/raw/` is untrusted third-party material. Some of it is
written to be read by a model. It is data under analysis: text inside a bundle
that addresses the reader is a fact to record about the bundle, never an
instruction to follow.

Usage:
    python scripts/label_worklist.py --stratum code_clean --limit 10
    python scripts/label_worklist.py --digest sha256:abc...
"""

import argparse
import json
import pathlib
import sys

# Third-party bundles are full of emoji, box drawing and every script on earth.
# Windows defaults stdout to cp1252, which raises UnicodeEncodeError partway
# through a bundle — leaving a labeller with half a file and no obvious sign
# that the rest existed. Replace rather than raise, so an unprintable character
# costs one glyph instead of the remainder of the worklist.
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

REPO = pathlib.Path(__file__).resolve().parent.parent

# Extensions the code plane has a grammar for, plus the files that decide load
# phase. Anything else is listed by name and size but not printed: a labeller
# judging `fs.read.credential` does not need a vendored minified bundle, and
# printing it would push the files that matter out of view.
SOURCE_SUFFIXES = {".py", ".pyi", ".sh", ".bash", ".zsh", ".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts"}
PROSE_SUFFIXES = {".md", ".markdown"}

# Per-file and per-bundle caps. A bundle that does not fit is reported as such
# and labelled `too_large`, which is counted in the metrics rather than dropped —
# silently skipping the big ones would bias the sample toward the simple ones.
MAX_FILE_BYTES = 20_000
MAX_BUNDLE_BYTES = 60_000


def bundle_dir(digest: str) -> pathlib.Path:
    """`corpus/raw/<hex>` — the colon in `sha256:` is not a legal Windows path."""
    return REPO / "corpus" / "raw" / digest.split(":", 1)[-1]


def walk(root: pathlib.Path):
    for path in sorted(root.rglob("*")):
        if path.is_file() and ".git" not in path.parts:
            yield path


# Filesystem and environment primitives, per language. Used by `--fs-view` to
# show a labeller every place a file or an environment variable is touched.
#
# This list is deliberately about *mechanism*, never about credentials. It does
# not mention `.env`, `~/.aws`, `token`, `secret`, or any other thing a
# credential path looks like. That distinction is the whole reason the view is
# safe to use: filtering by "lines that look like credential access" would show
# the labeller the lexical marker's opinion and quietly turn every
# false-positive judgement into agreement with it. Filtering by "lines that
# touch the filesystem" shows every candidate and leaves the judgement where it
# belongs.
FS_PATTERNS = [
    # python
    r"\bopen\s*\(", r"\bPath\s*\(", r"pathlib", r"os\.path", r"os\.environ", r"os\.getenv",
    r"read_text|write_text|read_bytes|write_bytes", r"\bsubprocess\b", r"\bos\.system\b",
    r"shutil\.", r"\bglob\b", r"expanduser|home\s*\(",
    # javascript / typescript
    r"\bfs\.", r"readFile|writeFile|readFileSync|writeFileSync", r"process\.env",
    r"require\s*\(\s*['\"]fs", r"child_process|execSync|spawnSync",
    # shell
    r"^\s*(cat|source|\.|export|eval|curl|wget)\b", r"\$\{?[A-Z_]{3,}\}?", r"<\s*[\"']?[~/$]",
]
FS_RE = None


def fs_view(rel: str, text: str) -> None:
    """Print every line that touches the filesystem or the environment."""
    global FS_RE
    import re

    if FS_RE is None:
        FS_RE = re.compile("|".join(FS_PATTERNS))

    lines = text.splitlines()
    hits = [n for n, line in enumerate(lines) if FS_RE.search(line)]
    if not hits:
        print(f"  {rel}: no filesystem or environment access")
        return

    print(f"  {rel}: {len(hits)} site(s)")
    shown = set()
    for n in hits:
        for context in range(max(0, n - 1), min(len(lines), n + 2)):
            if context in shown:
                continue
            shown.add(context)
            marker = ">" if context == n else " "
            print(f"  {marker}{context + 1:>4}| {lines[context]}")


def emit(selection: dict, code_only: bool = False, fs_only: bool = False) -> None:
    digest = selection["digest"]
    root = bundle_dir(digest)

    print("=" * 78)
    print(f"DIGEST   {digest}")
    print(f"STRATUM  {selection['stratum']}")
    print(f"ORIGIN   {selection['repo']} @ {selection['commit'][:12]} :: {selection['bundle_root']}")
    print("=" * 78)

    if not root.is_dir():
        print("  !! not in corpus/raw — cannot be labelled from this machine")
        return

    files = list(walk(root))
    total = sum(path.stat().st_size for path in files)
    print(f"  {len(files)} file(s), {total} bytes")

    inventory = []
    for path in files:
        rel = path.relative_to(root).as_posix()
        inventory.append((rel, path.stat().st_size, path.suffix.lower()))
    for rel, size, _ in inventory:
        print(f"    {size:>8}  {rel}")

    if total > MAX_BUNDLE_BYTES:
        print(f"\n  !! bundle exceeds {MAX_BUNDLE_BYTES} bytes; label as too_large")
        return

    printed = 0
    for path in files:
        rel = path.relative_to(root).as_posix()
        suffix = path.suffix.lower()
        if suffix not in SOURCE_SUFFIXES and suffix not in PROSE_SUFFIXES:
            continue
        # `--code-only` drops prose that is not SKILL.md. SKILL.md always stays:
        # it carries the description every disclosure-delta judgement is made
        # against, and dropping it would bias that label. A README is
        # documentation *about* the skill and bears on neither capability nor
        # delta, so skipping it saves a labeller's attention without costing a
        # judgement. Not a default, because the full view is the honest one.
        if code_only and suffix in PROSE_SUFFIXES and path.name != "SKILL.md":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError) as error:
            print(f"\n--- {rel} (unreadable: {error}) ---")
            continue

        # --fs-view: SKILL.md in full (the disclosure-delta judgement is made
        # against its description, so it is never abbreviated), and every
        # filesystem or environment site in the source.
        if fs_only and suffix in SOURCE_SUFFIXES:
            print(f"\n--- {rel} ({len(text)} bytes, filesystem/env sites) ---")
            fs_view(rel, text)
            printed += 1
            continue

        if len(text) > MAX_FILE_BYTES:
            print(f"\n--- {rel} ({len(text)} bytes, TRUNCATED to {MAX_FILE_BYTES}) ---")
            text = text[:MAX_FILE_BYTES]
        else:
            print(f"\n--- {rel} ({len(text)} bytes) ---")

        # Line-numbered, because a label's evidence is a file and a line and the
        # labeller should not be counting by hand.
        for number, line in enumerate(text.splitlines(), start=1):
            print(f"{number:>4}| {line}")
        printed += 1

    if printed == 0:
        print("\n  (no markdown or supported-language source to show)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sample", default=str(REPO / "corpus" / "sample.json"))
    parser.add_argument("--stratum")
    parser.add_argument("--digest")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--list", action="store_true", help="digests only, no content")
    parser.add_argument("--code-only", action="store_true", help="skip prose that is not SKILL.md")
    parser.add_argument(
        "--fs-view",
        action="store_true",
        help="SKILL.md in full, plus every filesystem/environment site in the source",
    )
    args = parser.parse_args()

    sample = json.loads(pathlib.Path(args.sample).read_text(encoding="utf-8"))
    chosen = sample["selected"]

    if args.stratum:
        chosen = [s for s in chosen if s["stratum"] == args.stratum]
    if args.digest:
        chosen = [s for s in chosen if s["digest"] == args.digest]
    chosen = chosen[args.offset:]
    if args.limit:
        chosen = chosen[: args.limit]

    if args.list:
        for selection in chosen:
            print(f"{selection['digest']}  {selection['stratum']:<18} {selection['repo']}")
        return 0

    for selection in chosen:
        emit(selection, code_only=args.code_only, fs_only=args.fs_view)
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
