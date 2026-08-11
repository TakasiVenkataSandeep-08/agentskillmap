# skillmap

> Name is a placeholder — check crates.io and npm availability before first publish.

A supply-chain auditor for AI agent skills. It answers **"what does this skill make my agent
capable of doing?"** with byte-level evidence, and diffs that answer across versions.

It is not a linter, not a risk scorer, and not a malware classifier. It emits a capability
manifest; your `policy.toml` decides what is acceptable.

## Why

`SKILL.md` is an open standard read by Claude Code, Claude.ai, the Anthropic API, Codex,
Cursor, Gemini CLI, Antigravity, and Windsurf. A skill is arbitrary instructions plus
optional scripts, running with your agent's permissions, installed with one command from a
blog post.

The structural gap is progressive disclosure: the agent sees ~100 tokens of description at
session start. The reviewer reads `SKILL.md`, sees something benign, installs. The payload
lives in the deep files that only load on trigger, days later, mid-task, unobserved.

Human review of skills is structurally shallower than human review of code. This tool
closes that gap mechanically.

## The check that matters

```
$ skillmap ci
✗ example-skill  capability escalation vs skillmap.lock
    + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
      reads ~/.aws/credentials — added in this update
```

Everything else in this repository exists to make that line trustworthy.

## Measured

Corpus snapshot `2026-08`, produced by skillmap at commit `242ecac`.
Full report: [`corpus/report.md`](corpus/report.md).

**34,284 distinct bundles** from 202 public repositories, deduplicated by content digest.
881 came from curated sources — the `anthropics/skills` baseline and an awesome-list — and
33,403 from GitHub code search, which is the only source that reaches the ecosystem's tail.

The two populations are not alike, and that gap is the finding:

| Measured, exactly | Curated head | Tail | Ratio |
|---|---|---|---|
| Ships executable scripts | 2.2% | **10.4%** | 4.7× |
| Has files nothing references | 2.2% | **30.7%** | 14× |
| Mentions a credential path¹ | 0.4% | **9.1%** | 23× |
| Mentions a secret-bearing env var¹ | 0.9% | **17.5%** | 19× |
| Mentions `eval` / `exec` / subprocess¹ | 4.8% | **25.1%** | 5× |

Denominators are 881 (head) and 33,403 (tail). Anyone sampling only curated lists — which is
what most writing about this ecosystem does — would conclude the risk was theoretical.

**The progressive-disclosure gap.** The median bundle shows an agent **2.09%** of its bytes
at session start; 32.4% show under 1%. Across the corpus, **1.17 GB of 1.63 GB (72%) sits in
files nothing points at.** That asymmetry is the whole reason this project exists.

**The lead worth chasing.** 1.6% of bundles mention a credential path *only* in files no
documented path reaches — the disclosure-delta shape, and the starting list for labelling.

¹ Lexical: substring matches, not analysis. They do not parse, establish no reachability, and
carry no provenance, so they are **upper bounds** and never appear in a manifest. `corpus/report.md`
labels every one.

### What is not measured yet

`docs/05-eval.md` names the false-positive rate on a benign stratum as the headline metric,
and requires precision and recall **per capability term**. None of those exist here, because
the corpus is measured but **not labelled** — there is no ground truth to score against, so
there is no held-out split and no precision, recall, or false-positive rate. Publishing the
base rates above as though they were quality metrics would be exactly that overstatement.

What is gated in CI today is the eval suite: 4 rule-fixture cases and 5 adversarial cases
pass on every commit, and 3 further adversarial cases from `docs/05-eval.md` are declared and
reported as pending rather than omitted — they need the semantic pass (T7), the diff (T8), and
a `code.obfuscation` rule. The gate fails on a failing case, on coverage shrinking, and on a
case regressing to pending. See `eval/baseline.json`.

## Status

Pre-alpha. The scanner runs and the corpus is harvested; the CLI, policy, diff, and semantic
layer are not built. Start with `docs/00-tasks.md`, which records what each task actually
delivered and what it deliberately did not.

## For contributors

Detection rules are **data**, not Rust. Adding coverage means a tree-sitter query, a TOML
file, and two fixtures — no Rust required. See `docs/03-rules-authoring.md`.

## Reading order

| File | What it is |
|---|---|
| `AGENTS.md` | The twelve invariants. Read first; they constrain everything. |
| `ARCHITECTURE.md` | Crate layout, data flow, key traits |
| `docs/00-tasks.md` | Ordered backlog with acceptance criteria |
| `docs/01-corpus-scan.md` | Step one, and the kill gate |
| `docs/02-manifest-schema.md` | The spine |
| `docs/03-rules-authoring.md` | How to add detection |
| `docs/04-semantic-layer.md` | The quarantined model pass |
| `docs/05-eval.md` | The falsifiable quality bar |
| `SECURITY.md` | Threat model and disclosure policy |
| `CONTRIBUTING.md` | Contributor workflow — including adding a rule without writing Rust |

## License

Apache-2.0.
