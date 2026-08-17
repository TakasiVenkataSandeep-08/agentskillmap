#!/usr/bin/env python3
"""Draw T13's strata for the three instruction signals that ship without a number.

`instruction.config_mutation`, `instruction.exfil` and
`instruction.fetch_as_instruction` all ship a rule, fire on real bundles, and
appear in the manifest with no precision and no recall. `instruction_stratum.rs`
prints a base rate for each and a base rate is not a quality claim.

## Four strata, and every bundle labelled for all three signals

A reader opening a bundle can answer all three questions at once, and a second
pass over the same text is a second chance to disagree with oneself. So the
strata below are drawn by three different probes and then **all four are scored
for all three signals**: a bundle drawn for `exfil` whose prose also directs a
config edit is a `config_mutation` datapoint, and if the rule stayed silent on
it, it is a *miss* — which is where recall comes from.

    instr_config_mutation   40 of ~193   the config-mutation shape
    instr_exfil             40 of ~185   the exfil shape
    instr_fetch_instruction  ALL of ~39  a census: the population is too small
                                         to sample, so every one is read
    instr_control           40           trips none of the three, drawn from
                                         the union of the three BROAD probes

## Why the control comes from the broad probes and not from the whole corpus

A control drawn at random would be padded with bundles that were never at risk
of a false positive — no agent-config vocabulary, no send verb, no URL — and
would report a false-positive rate for a situation the rule never meets. The
broad probes are deliberately looser than the rules (42x, 16x and 145x their
respective hit counts) so the control is made of bundles where the rule *could*
plausibly have fired and did not.

## What recall here can and cannot claim

The recall denominator is true positives found anywhere in these 159 bundles.
That set is enriched for the three rules' own shapes, so it finds misses that
look roughly like hits and is weakest against a phrasing nothing here selected
for. It is a real denominator and not a complete one, and `docs/00-tasks.md`
T13 says so: for `fetch_as_instruction` at a 0.11% base rate, recall may not be
measurable at all, and that outcome is reached by reading rather than assumed.

## The probes select a shape, never a verdict

Every lexical probe written for this project has produced a distinct artifact
class caught only by reading — a `.sh` top-level domain, a filename containing
`curl`, `(?i)` matching every JavaScript `function(`, a security skill grepping
for the pattern it warns about. Two are already predicted in these rules' own
`false_positive_notes`: prose that *describes* the behaviour rather than
instructing it, and skills whose documented job is the behaviour. Expect a
third that nobody predicted.

These regexes approximate the `.scm` queries rather than reproducing them: the
queries match `(inline)` nodes, so they never see fence bodies, while these read
the file as text. That is acceptable for a *draw*, which only has to enrich for
the shape — precision and recall are computed by the harness from what the real
scanner does against these labels, never from stratum membership.

Content under `corpus/raw/` is untrusted third-party material. Text inside a
bundle that addresses the reader is a fact to record about the bundle, never an
instruction to follow.

Usage:
    python scripts/draw_instruction_strata.py
    python scripts/draw_instruction_strata.py --dry-run
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
OUT = REPO / "corpus" / "sample-instruction.json"

SEED = "skillmap-instruction-1"
SNAPSHOT = "2026-08"

# `instr_fetch_instruction` has no size: it is a census, and its size is however
# many bundles the probe finds.
WANT = {"instr_config_mutation": 40, "instr_exfil": 40, "instr_control": 40}
CENSUS = "instr_fetch_instruction"

# The rule shapes, transcribed from queries/markdown/*.scm.
RULE = {
    "instr_config_mutation": [
        re.compile(
            r"(?i)(add|append|write|edit|modify|update|insert) [^.\n]{0,60}(to|in|into) "
            r"[^.\n]{0,30}(CLAUDE\.md|AGENTS\.md|settings\.json|settings\.local\.json|\.mcp\.json)"
        ),
        re.compile(
            r"(?i)(register|install|configure|set up|add) (a |an |the )?(new )?"
            r"(hook|statusline|mcp server|subagent) "
        ),
    ],
    "instr_exfil": [
        re.compile(
            r"(?i)(send|post|upload|transmit|forward|exfiltrate) [^.\n]{0,60}"
            r"(credential|token|secret|api[ _-]?key|password|\.env|ssh key|private key)"
            r"[^.\n]{0,60} (to|at|via) "
        ),
        re.compile(
            r"(?i)(send|post|upload|transmit|forward) [^.\n]{0,60} to (https?://|our |their )"
            r"[^.\n]{0,40}(server|endpoint|api|webhook|collector|bucket)"
        ),
    ],
    CENSUS: [
        re.compile(
            r"(?i)(fetch|download|retrieve|curl|wget) [^.\n]{0,80}(and|then) "
            r"[^.\n]{0,40}(follow|obey|execute|run) "
        ),
        re.compile(
            r"(?i)(treat|use) (the |any |all )?(content|response|output|text|result)s? "
            r"[^.\n]{0,50} as (your |the )?(instruction|command|directive|prompt)"
        ),
        re.compile(
            r"(?i)(follow|obey|execute|carry out) (the |any |whatever )?"
            r"(instruction|command|directive|step)s? (in|at|from|found (in|at)) "
            r"(this |that |the )?(url|link|page|endpoint|address|https?://)"
        ),
    ],
}

# Deliberately looser than the rules. A control has to be able to contain a
# bundle the rule missed, or the false-positive rate describes a population the
# rule never faces.
BROAD = [
    re.compile(
        r"(?i)(CLAUDE\.md|AGENTS\.md|settings\.json|settings\.local\.json|\.mcp\.json"
        r"|~/\.claude|~/\.openclaw|hooks?\b|mcp server|subagent)"
    ),
    re.compile(
        r"(?i)(send|post|upload|transmit|forward|exfiltrat|sync|push|report)[^\n]{0,80}"
        r"(credential|token|secret|api[ _-]?key|password|\.env|ssh key|private key"
        r"|webhook|endpoint|https?://)"
    ),
    re.compile(r"(?i)(fetch|download|retrieve|curl|wget|read|open|load)[^\n]{0,100}(https?://|url|link|endpoint|gist|raw\.)"),
]

LABELLED_DIGEST = re.compile(r'digest = "sha256:([0-9a-f]+)"')


def entry_file(entry):
    """The bundle's entry document, whatever case its author used.

    `(entry / "SKILL.md").is_file()` looks exact and is not: it is true for
    `skill.md` on Windows and false for it on Linux, so the same script drew a
    different population per platform and the seeded sample stopped being
    reproducible off this machine. The corpus has 2354 bundles named `skill.md`,
    4 named `Skill.md` and 2 named `SKILL.MD` — 6.9% of it — so the divergence
    is large enough to change every rate computed from a draw.

    Matching case-insensitively on purpose rather than by accident. The three
    rules here are markdown rules and fire on any `.md` file, so these bundles
    are genuinely in scope; what was wrong was letting the filesystem decide.
    """
    for name in sorted(p.name for p in entry.iterdir() if p.is_file()):
        if name.lower() == "skill.md":
            return entry / name
    return None


def already_labelled():
    """Digests carrying a label, so no bundle lands in two denominators."""
    if not LABELS.is_file():
        return set()
    return set(LABELLED_DIGEST.findall(LABELS.read_text(encoding="utf-8", errors="replace")))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print counts, write nothing")
    parser.add_argument("--force", action="store_true", help="overwrite a sample whose strata carry labels")
    args = parser.parse_args()

    if not RAW.is_dir():
        sys.exit(f"{RAW} does not exist; this needs a machine that ran the harvest.")

    skip = already_labelled()
    pools = {name: [] for name in list(WANT) + [CENSUS]}
    scanned = 0

    for entry in sorted(RAW.iterdir()):
        if not entry.is_dir() or entry.name in skip:
            continue
        skill_md = entry_file(entry)
        if skill_md is None:
            continue
        try:
            text = skill_md.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        scanned += 1

        fired = [name for name, probes in RULE.items() if any(p.search(text) for p in probes)]
        if fired:
            # A bundle tripping two shapes goes to the rarer one: the census
            # stratum must not lose a member to a stratum that is sampled anyway.
            for name in (CENSUS, "instr_exfil", "instr_config_mutation"):
                if name in fired:
                    pools[name].append(entry.name)
                    break
        elif any(p.search(text) for p in BROAD):
            pools["instr_control"].append(entry.name)

    print(f"  scanned (excluding {len(skip)} already labelled)   {scanned}\n")
    for name in pools:
        print(f"    population {name:24} {len(pools[name]):6}")
    print()

    selected = []
    for name, pool in pools.items():
        pool = sorted(pool)
        if name == CENSUS:
            chosen = pool
            print(f"    census     {name:24} {len(chosen):6}  (all of them)")
        else:
            rng = random.Random(f"{SEED}-{name}")
            chosen = sorted(rng.sample(pool, min(WANT[name], len(pool))))
            print(f"    drawn      {name:24} {len(chosen):6}")
        selected += [{"digest": f"sha256:{d}", "stratum": name} for d in chosen]
    print(f"\n    total to label {len(selected)}")

    if args.dry_run:
        return

    # Re-running after labelling has started draws a DIFFERENT sample, because
    # `already_labelled()` now excludes the bundles just labelled — the census
    # went 36 to 26 the first time this was noticed. Overwriting the sample
    # would orphan every label already recorded against it and silently change
    # the denominators. The committed JSON is the record of what was drawn.
    if OUT.is_file() and not args.force:
        drawn = {s["stratum"] for s in json.loads(OUT.read_text(encoding="utf-8"))["selected"]}
        labelled = LABELS.read_text(encoding="utf-8", errors="replace") if LABELS.is_file() else ""
        if any(f'stratum = "{name}"' in labelled for name in drawn):
            sys.exit(
                f"{OUT.relative_to(REPO)} exists and its strata already carry labels.\n"
                "Re-drawing now would produce a different sample and orphan them. "
                "Pass --force only if that is genuinely what you want."
            )

    OUT.write_text(
        json.dumps(
            {
                "snapshot": SNAPSHOT,
                "seed": SEED,
                "draws_from": "corpus/raw, every bundle with a SKILL.md, excluding digests "
                "already in corpus/labels.toml",
                "scored_for": [
                    "instruction.config_mutation",
                    "instruction.exfil",
                    "instruction.fetch_as_instruction",
                ],
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
