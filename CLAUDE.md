# Claude Code — project context

**Read [`AGENTS.md`](AGENTS.md) first.** It is canonical: the twelve invariants, the build
order, the repository layout, and the definition of done all live there, and they constrain
everything in this repository. A PR that violates an invariant is rejected regardless of what
it adds.

This file carries only what is specific to Claude Code. It deliberately does not restate the
invariants — a second copy is a second copy that drifts, and this project audits skills across
eight different agents. Its own instructions should be legible to all of them.

## Where things are

| Path | What |
|---|---|
| `AGENTS.md` | Invariants, build order, definition of done — start here |
| `ARCHITECTURE.md` | Crate layout, data flow, key traits |
| `docs/00-tasks.md` | Ordered backlog with per-task acceptance criteria |
| `docs/02-manifest-schema.md` | The manifest spine and its canonical serialization |
| `docs/03-rules-authoring.md` | How to add detection coverage |
| `CONTRIBUTING.md` | Contributor workflow, including adding a rule without writing Rust |
| `SECURITY.md` | Threat model, network behaviour, disclosure policy |

## Working here

Do not start a task in `docs/00-tasks.md` whose predecessor's acceptance criteria are unmet.
The ordering exists because each stage produces the input the next one needs.

Detection rules are **data** (invariant 7). If a change appears to require adding a sink name
to a `.rs` file, that is a bug in the engine, not a reason to add the `match` arm.
