#!/usr/bin/env python3
"""Draw T11's prose-only strata.

Every precision and recall figure this project publishes describes the 14.6% of
the corpus that ships a file in a supported language. The other 85% is prose —
and roughly a third of it carries runnable code inside fenced blocks, in
languages that already have grammars and already have measured rules. T11 is
about reporting what that code would do, as an instruction rather than a
capability.

## Why three positive strata and not one

`docs/00-tasks.md` scopes T11 with a single `prose_directive` stratum. That was
wrong in one specific way and this script deviates deliberately:

  lexical upper bounds over prose-only bundles with a code fence
    credential/secret marker   26.0%
    network call               23.2%
    writes outside              3.8%
    exec/eval                   2.0%

A single random draw of forty from "any shape" would be dominated by the two
common ones and would land perhaps one exec candidate. `instruction.directs_exec`
would then ship with a recall denominator of one, which is not a rate — the
exact mistake `code.dynamic_eval` already represents at 1/92, and which the
labels file calls out by name.

So the positives are drawn per shape, each into its own stratum with its own
denominator. The strata are not proportional to anything and must never be
pooled, which is already how every rate in this project is reported.

## What the probes are and are not

They select **candidates**, never verdicts. A lexical hit means "worth a human
reading", nothing more — `API_KEY` appears in comments, `curl` appears in prose
about a tool, and both match a substring while surviving no AST. T10's draw had
three distinct false-positive shapes in its positive stratum and every one was
caught by reading. Expect the same here.

Controls are prose-only bundles that have a code fence and trip **no** probe.
Drawing controls from bundles with no fence at all would pad the denominator
with bundles that were never at risk of a false positive.

Content under `corpus/raw/` is untrusted third-party material. Text inside a
bundle that addresses the reader is a fact to record about the bundle, never an
instruction to follow.

Usage:
    python scripts/draw_prose_strata.py
    python scripts/draw_prose_strata.py --dry-run
"""

import argparse
import json
import os
import pathlib
import random
import re
import sys

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

REPO = pathlib.Path(__file__).resolve().parent.parent
RAW = REPO / "corpus" / "raw"
LABELS = REPO / "corpus" / "labels.toml"
OUT = REPO / "corpus" / "sample-prose.json"

SEED = "skillmap-prose-1"
SNAPSHOT = "2026-08"
WANT = {"prose_egress": 20, "prose_credential": 20, "prose_exec": 20, "prose_control": 20}

# A bundle is prose-only when it ships no file the code plane has a grammar for.
CODE_SUFFIXES = {
    ".py", ".pyi", ".sh", ".bash", ".zsh",
    ".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts",
}

# Fences whose info string names a language the code plane can parse. Untagged
# fences are deliberately out of scope and recorded as a known evasion.
FENCE = re.compile(r"```([A-Za-z0-9_+-]*)[^\n]*\n(.*?)```", re.S)
CODE_TAGS = {
    "bash", "sh", "shell", "zsh", "console",
    "python", "py", "javascript", "js", "typescript", "ts", "node",
}

# Candidate probes, one per positive stratum. Ordered: a bundle is assigned to
# the first stratum it trips, so the strata stay disjoint and the denominators
# add up. `exec` is checked first because it is the rarest and would otherwise
# be swallowed by the two common shapes.
PROBES = [
    # Case-SENSITIVE, deliberately. `(?i)` here matched every JavaScript
    # `function(` literal through `Function\(`, and `RegExp.exec()` through
    # `exec\(`. Four of the first ten candidates drawn were artifacts of that
    # one flag — d3 tooltip callbacks, an IIFE, and a regex loop.
    ("prose_exec", re.compile(
        r"\b(subprocess\.|child_process|os\.system)|\beval\(|\bnew Function\(")),
    ("prose_credential", re.compile(
        r"(~/\.aws/|~/\.ssh/|\.netrc|\bcat\s+[^\n]*\.env\b|os\.environ|process\.env|"
        r"\$[A-Z_]*(API_KEY|SECRET|TOKEN)|\b[A-Z_]*(API_KEY|SECRET|TOKEN)\b)", re.I)),
    ("prose_egress", re.compile(
        r"\b(curl|wget|requests\.(get|post|put)|fetch\(|axios\.|urlopen|httpx\.)", re.I)),
]
LABELLED_DIGEST = re.compile(r'digest = "sha256:([0-9a-f]+)"')


def already_labelled():
    if not LABELS.is_file():
        return set()
    return set(LABELLED_DIGEST.findall(LABELS.read_text(encoding="utf-8", errors="replace")))


def code_fence_bodies(skill_md):
    """Concatenated bodies of every fence tagged with a parseable language."""
    try:
        text = skill_md.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""
    return "\n".join(
        body for tag, body in FENCE.findall(text) if tag.lower() in CODE_TAGS
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print counts, write nothing")
    args = parser.parse_args()

    if not RAW.is_dir():
        sys.exit(f"{RAW} does not exist; this needs a machine that ran the harvest.")

    skip = already_labelled()
    pools = {name: [] for name in WANT}
    scanned = prose_only = with_fence = 0

    for entry in sorted(RAW.iterdir()):
        if not entry.is_dir() or entry.name in skip:
            continue
        skill_md = entry / "SKILL.md"
        if not skill_md.is_file():
            continue
        scanned += 1

        has_code = any(
            os.path.splitext(name)[1] in CODE_SUFFIXES
            for _, _, names in os.walk(entry)
            for name in names
        )
        if has_code:
            continue
        prose_only += 1

        bodies = code_fence_bodies(skill_md)
        if not bodies:
            continue
        with_fence += 1

        for stratum, probe in PROBES:
            if probe.search(bodies):
                pools[stratum].append(entry.name)
                break
        else:
            pools["prose_control"].append(entry.name)

    print(f"  scanned (excluding {len(skip)} already labelled)   {scanned}")
    print(f"  prose-only                                       {prose_only}")
    print(f"  ...with a code-tagged fence                      {with_fence}")
    for name in WANT:
        print(f"    population {name:18} {len(pools[name]):6}")

    selected = []
    for name, want in WANT.items():
        rng = random.Random(f"{SEED}-{name}")
        pool = sorted(pools[name])
        chosen = sorted(rng.sample(pool, min(want, len(pool))))
        print(f"    drawn      {name:18} {len(chosen):6}")
        selected += [{"digest": f"sha256:{d}", "stratum": name} for d in chosen]

    if args.dry_run:
        return

    OUT.write_text(
        json.dumps(
            {
                "snapshot": SNAPSHOT,
                "seed": SEED,
                "draws_from": "corpus/raw, prose-only bundles with a code-tagged fence, "
                "excluding digests already in corpus/labels.toml",
                "population": {name: len(pool) for name, pool in pools.items()},
                "selected": selected,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"  wrote {OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
