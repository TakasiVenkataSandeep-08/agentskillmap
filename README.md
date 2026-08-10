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

## Status

Pre-alpha. Nothing works yet. Start with `docs/00-tasks.md`.

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
