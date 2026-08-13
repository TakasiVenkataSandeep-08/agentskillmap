# skillmap — agent and contributor context

> The name is settled: `skillmap`, available on crates.io as `skill-map` and on npm as
> `skillmap`. The repository was renamed from `skillaudit` in full; if you find the old
> name anywhere, that is a bug, not a variant spelling.

**This file is canonical.** It is the single source of truth for the invariants, the build
order, and the definition of done. `CLAUDE.md` points here and adds only Claude-Code-specific
tooling. Agent-specific files must never restate the invariants — a second copy is a second
copy that drifts.

That choice is not incidental. This tool's entire thesis is *cross-agent* auditing: `SKILL.md`
is read by Claude Code, Claude.ai, the Anthropic API, Codex, Cursor, Gemini CLI, Antigravity,
and Windsurf. A project arguing that agent tooling must span vendors, whose own contributor
instructions are legible to exactly one vendor, is arguing against itself.

## What this is

A capability differ for AI agent skills (`SKILL.md` bundles). It records **"what does this
skill make my agent capable of doing?"** — with byte-level evidence — and reports when that
answer changes.

Deliberately not an *auditor*: recall is measured, published, and well short of complete, so a
clean report is not an assurance. The README carries the numbers and says so above the fold.

It is **not** a linter, **not** a risk scorer, and **not** a malware classifier.

## Why it exists

`SKILL.md` is an open standard read by Claude Code, Claude.ai, the Anthropic API, Codex,
Cursor, Gemini CLI, Antigravity, and Windsurf. A skill is arbitrary instructions plus
optional scripts, executing with the agent's permissions, installed with a single
`npx skills add <user>/<repo>` from a blog post.

The structural hole is **progressive disclosure**: at session start the agent sees only
~100 tokens of name and description. The reviewer reads `SKILL.md`, sees something benign,
installs. The payload lives in deep-loaded reference files that only enter context on
trigger, days later, mid-task, unobserved. Human review of skills is structurally shallower
than human review of code. That asymmetry is the product thesis.

---

## The twelve invariants

These are not style preferences. A PR that violates one is rejected regardless of what it
adds. If a task appears to require violating one, stop and escalate rather than working
around it.

### 1. Manifest, not verdict

Emit capabilities with evidence. Never emit a risk score, a letter grade, a traffic light,
or the words "safe" / "malicious" / "suspicious" in the manifest.

Half the flagged capabilities are legitimate — plenty of good skills need shell exec.
A tool that moralizes gets uninstalled in a day; a tool that describes becomes
infrastructure. Policy (`policy.toml`) decides what is acceptable. The scanner does not.

### 2. Byte-identical determinism

Same bundle → byte-identical manifest, on any machine, any run, forever.

- Sort every collection before serialization. No `HashMap` iteration into output —
  use `BTreeMap`, or sort explicitly.
- No timestamps, hostnames, absolute paths, usernames, durations, or run IDs in the
  manifest. Those go to stderr or a separate `run-meta.json`.
- Normalize paths to forward-slash, relative to bundle root. Normalize line endings to LF
  before hashing text.
- Canonical JSON: sorted keys, no insignificant whitespace, UTF-8, `\n` terminated.
- There is a CI test that scans the fixture corpus twice on two platforms and byte-compares.
  Nondeterminism is a P0 bug, not a nit. CI diffing is the entire product value; noise
  destroys it.

### 3. Unknown is a first-class output

Never silently drop what you could not analyze. Unparseable file, unsupported language,
dynamic dispatch, indirect call, computed import — each emits an `unresolved` entry with a
reason code.

A scanner that reports nothing because it understood nothing must be visibly distinct from
a scanner that reports nothing because there was nothing there. Absence of findings is only
meaningful next to a complete `unresolved` list.

### 4. Every finding carries provenance

A finding you cannot point at is a finding nobody trusts, and it cannot be regression-tested.
Every finding in every tier names a file and a location. What "location" means is fixed per
tier by the schema, not by the author's judgement:

- **`proven` and `pattern`** carry the full set — `{ file, byte span, line, rule_id,
  snippet_sha256 }`, all required. A rule fired, so all five exist. **No exceptions**,
  including for instruction-plane findings.
- **`advisory`** carries `{ file, line }`, and the type structurally cannot hold anything
  more. No rule fired, so there is no `rule_id` to report, and a byte span reconstructed from
  a model's prose citation would be manufactured precision — worse than an honest line
  number, because it looks checkable and is not.

The distinction is enforced by two separate evidence types in `skillmap-core` and two
separate `$defs` in the schema, so the advisory tier cannot claim deterministic provenance
even by accident.

### 5. Three assurance tiers, never blended

| Tier | Source | Assurance |
|---|---|---|
| `proven` | Code plane: tree-sitter AST + reachability | Provably present in the bundle |
| `pattern` | Instruction plane: lexical patterns over prose | Weak, explicitly labelled |
| `advisory` | Semantic layer: model judgement | Non-reproducible without pinning |

These live in **separate branches of the manifest**. A `pattern` hit never promotes itself
to `proven`. An `advisory` finding can never add to, remove from, or modify the
deterministic branches. Consumers that trust only `proven` must be able to ignore the rest
by dropping two keys.

### 6. The semantic layer is quarantined

You are feeding suspected prompt-injection payloads to a model. Therefore:

- Skill content enters the prompt as delimited **data**, never as instruction.
- The pass runs with **no tools and no network access** beyond the single model call.
- Output is schema-validated JSON; violation → discard the finding and emit a diagnostic.
  Never fall back to free-text parsing.
- Model ID and SHA-256 of the exact prompt template are pinned into the manifest.
- Runs only under an explicit flag. Never on by default.
- If the model output contains anything resembling an instruction to the auditor, that is
  logged as a finding about the skill — never acted on.

### 7. Rules are data, not Rust

Detection lives in tree-sitter `.scm` query files plus TOML metadata plus fixtures.
No sink patterns hardcoded in `match` arms, ever.

This is a contributor-pool decision. Rust gets an order of magnitude fewer drive-by PRs
than Go, and rule coverage is the product's long-term quality. Someone who spots a missed
obfuscation pattern must be able to fix it with a query file and two fixtures and zero Rust.
If adding a rule requires touching a `.rs` file, the architecture has failed.

### 8. No rule without fixtures

Every rule ships a positive fixture (must fire), a negative fixture (must not fire), and an
expected-manifest snapshot. CI runs the full fixture corpus. A rule with no negative fixture
is an untested false-positive generator.

### 9. Offline by default, zero telemetry, forever

A supply-chain tool with a supply-chain problem is worthless. No network at scan time.
No analytics, no accounts, no phone-home, not even opt-in. The only network calls in the
entire binary are: (a) the semantic pass under an explicit flag, (b) the `corpus` subcommand,
which is a research tool and says so. Both are documented in `SECURITY.md`.

Releases are reproducible and signed. Publishing an unreproducible release is a release blocker.

### 10. Errors are typed; no panics in library crates

`unwrap`, `expect`, `panic!`, and array indexing that can go out of bounds are denied by lint
in every crate except the CLI binary and tests. Hostile input is the normal case here.
A parser crash on a malformed bundle is a denial-of-service on someone's CI.

### 11. Eval is CI-gated

Precision and recall against the labeled corpus are computed per release and published in
the README. A regression beyond the declared tolerance fails the build. Shipping the scanner
without this makes it a stub no matter how good the parser is.

### 12. No stub commits

A feature lands with tests and fixtures or it does not land. No `todo!()` on a shipped code
path, no "wire this up later", no capability declared in the taxonomy that no rule detects.
Prefer a smaller manifest that is entirely true over a wider one that is partly aspirational.

---

## Build order (non-negotiable, even though v1.0 ships complete)

Releases are not phased — v1.0 ships the deterministic core, the semantic layer, the CI
action and the npm wrapper together. The **build order** is still fixed, because each stage
produces the input the next one needs:

1. **`corpus`** — harvest and measure real skills. Gates the format-support decision and
   produces the launch report. See `docs/01-corpus-scan.md`.
2. **Manifest schema + canonical serialization** — the spine. Everything serializes to it.
3. **Resolver + parser + inventory** — discovery and hashing across agents.
4. **Rule engine + code plane** — tree-sitter, sinks, reachability.
5. **Eval harness** — labeled corpus, metrics, CI gate.
6. **Semantic layer** — built and measured against real labels from step 1, not against
   adversarial examples we invented ourselves.
7. **Policy + diff + CI action + npm wrapper.**

Step 6 is last on purpose. Its quality is unbounded and unmeasurable without a labeled
corpus of real undisclosed-capability cases, and step 1 is what produces those labels.

## Repository layout

See `ARCHITECTURE.md`. Do not add a crate without updating it.

Workspace members in `Cargo.toml` list only crates that exist. A crate is added to the list
when its task begins, not in advance — twelve empty crates would be twelve stubs (invariant 12).

## Definition of done for any task here

- [ ] Fixtures added (positive + negative where applicable)
- [ ] Determinism test still byte-identical
- [ ] `unresolved` emitted for anything not fully analyzed
- [ ] Provenance on every new finding type
- [ ] No new `unwrap` in a library crate
- [ ] Schema version bumped if the manifest shape changed, with a migration note
- [ ] Docs updated in the same commit
