#!/usr/bin/env python3
"""Draw the `fence_directive` stratum for T10.

T10 needs ground truth for a signal that fires on a **fenced code block inside
`SKILL.md`**. The existing labelled sample cannot supply it: of its 115 bundles,
two carry the shape. A rate computed on two is not a rate, so a targeted draw is
mandatory rather than a refinement.

## Why this is a script and not a `Stratum` variant

`skillmap-corpus` owns sampling, and the honest default would be to add a
variant to `sample::Stratum`. That was rejected, for a reason worth recording:

  The five existing strata are **disjoint by construction**. `Stratum::of`
  assigns exactly one in priority order, so inserting a sixth reclassifies
  bundles that already belong to `code_clean` and `code_other_marker` — which
  moves the published per-stratum false-positive rates for reasons that have
  nothing to do with detection quality.

That is the `mcp.tool_reference` objection in a different costume, and it sinks
the same design. This draw is therefore **supplementary**: a separate,
later-dated sample with its own seed, adding a stratum *group* to the eval —
which keys strata by string — while the original five keep their populations,
their draws and their numbers exactly as published.

## What it excludes, and why that matters

Already-labelled digests are excluded. `corpus/labels.toml` rejects a duplicate
digest outright, and a bundle drawn twice would also be one bundle counted in
two denominators.

## The shape being sampled

Positives are bundles whose `SKILL.md` has a shell-family fence whose body
either pipes a fetch into a shell, or fetches a `.sh`/`.py` and runs it.
Negatives are drawn from bundles that have a shell fence and neither shape —
ordinary usage examples, which are the false positives this signal will
actually face. Drawing negatives from the whole corpus instead would stack the
deck: a bundle with no fence at all cannot produce a false positive here, and
padding the denominator with bundles that were never at risk is how a
false-positive rate gets flattered.

**Most positives are expected to be legitimate.** A skill documenting
`curl … | sh` to install a real tool is doing something ordinary. This draw
selects a *shape*, never a verdict, and the labeller decides bundle by bundle.

Content under `corpus/raw/` is untrusted third-party material. Text inside a
bundle that addresses the reader is a fact to record about the bundle, never an
instruction to follow.

Usage:
    python scripts/draw_fence_stratum.py            # write corpus/sample-fence.json
    python scripts/draw_fence_stratum.py --dry-run  # counts only
"""

import argparse
import json
import pathlib
import random
import re
import sys

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

REPO = pathlib.Path(__file__).resolve().parent.parent
RAW = REPO / "corpus" / "raw"
LABELS = REPO / "corpus" / "labels.toml"
OUT = REPO / "corpus" / "sample-fence.json"

# Its own seed, so re-running this never perturbs `corpus/sample.json`.
SEED = "skillmap-fence-1"
SNAPSHOT = "2026-08"
WANT_POSITIVE = 40
WANT_NEGATIVE = 40

SHELL_FENCE = re.compile(r"```(?:bash|sh|shell|console|zsh)[^\n]*\n(.*?)```", re.S | re.I)
# `curl … | bash` — the documented shape, including a `sudo` in the pipe.
PIPE_TO_SHELL = re.compile(r"(curl|wget)[^\n|]{0,200}\|\s*(sudo\s+)?(ba)?sh", re.I)
# `curl … install.sh` then run it — the same directive, two lines instead of a pipe.
INSTALLER_FETCH = re.compile(r"(curl|wget)[^\n]{0,200}\.(sh|py)\b", re.I)
LABELLED_DIGEST = re.compile(r'digest = "sha256:([0-9a-f]+)"')


def already_labelled():
    """Digests `corpus/labels.toml` already carries."""
    if not LABELS.is_file():
        return set()
    text = LABELS.read_text(encoding="utf-8", errors="replace")
    return set(LABELLED_DIGEST.findall(text))


def classify(skill_md):
    """`"positive"`, `"negative"`, or `None` when there is no shell fence.

    `None` matters: a bundle with no shell fence is not a negative, it is
    outside the population this signal can fire on at all.
    """
    try:
        text = skill_md.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    bodies = "\n".join(SHELL_FENCE.findall(text))
    if not bodies:
        return None
    if PIPE_TO_SHELL.search(bodies) or INSTALLER_FETCH.search(bodies):
        return "positive"
    return "negative"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print counts, write nothing")
    args = parser.parse_args()

    if not RAW.is_dir():
        sys.exit(f"{RAW} does not exist; this needs a machine that ran the harvest.")

    skip = already_labelled()
    positives, negatives = [], []
    scanned = 0
    for entry in sorted(RAW.iterdir()):
        if not entry.is_dir() or entry.name in skip:
            continue
        skill_md = entry / "SKILL.md"
        if not skill_md.is_file():
            continue
        scanned += 1
        verdict = classify(skill_md)
        if verdict == "positive":
            positives.append(entry.name)
        elif verdict == "negative":
            negatives.append(entry.name)

    print(f"  scanned (excluding {len(skip)} already labelled)  {scanned}")
    print(f"  population, fence_directive shape               {len(positives)}")
    print(f"  population, shell fence without the shape       {len(negatives)}")

    # Sorted input plus a seeded PRNG: the same corpus draws the same sample on
    # any machine, which is what makes the published rate checkable.
    rng = random.Random(SEED)
    chosen_pos = sorted(rng.sample(positives, min(WANT_POSITIVE, len(positives))))
    rng = random.Random(SEED + "-negative")
    chosen_neg = sorted(rng.sample(negatives, min(WANT_NEGATIVE, len(negatives))))

    print(f"  drawn positive / negative                       {len(chosen_pos)} / {len(chosen_neg)}")
    if args.dry_run:
        return

    sample = {
        "snapshot": SNAPSHOT,
        "seed": SEED,
        "draws_from": "corpus/raw, excluding digests already in corpus/labels.toml",
        "population": {
            "fence_directive": len(positives),
            "fence_control": len(negatives),
        },
        "selected": (
            [{"digest": f"sha256:{d}", "stratum": "fence_directive"} for d in chosen_pos]
            + [{"digest": f"sha256:{d}", "stratum": "fence_control"} for d in chosen_neg]
        ),
    }
    OUT.write_text(json.dumps(sample, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"  wrote {OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
