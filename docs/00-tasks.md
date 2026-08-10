# Task backlog

Ordered. Each task states its own acceptance criteria. Do not start a task whose
predecessor's criteria are unmet — the ordering exists because each stage produces the
input the next one needs (see `AGENTS.md`, build order).

Releases are not phased: v1.0 ships everything. The **build** order is fixed.

---

## T0 — project infrastructure

Not in the original backlog, because the original backlog assumed the repository already
enforced its own standards. It did not: `AGENTS.md` claimed a two-platform determinism CI test
that did not exist, invariant 7's contributor argument had no `CONTRIBUTING.md` behind it, and
`SECURITY.md` promised reproducible builds with no pinned toolchain and no dependency gate.

- Documented directory layout; `AGENTS.md` canonical, `CLAUDE.md` a pointer.
- Spec repairs so the manifest schema stops contradicting the invariants: total sort order on
  every array, tier-dependent evidence types, closed `detail` and `diagnostics`, exact
  `content_digest` and `target.root` definitions, `declared_capabilities` as raw strings.
- Reproducibility and supply-chain gates: pinned toolchain, `--remap-path-prefix`,
  `cargo-deny`, `.gitattributes` LF normalization.
- `CONTRIBUTING.md` with a rule-authoring walkthrough that requires no Rust; issue and PR
  templates.
- `scripts/verify_spec.py` and the CI jobs that can genuinely pass before any crate exists.

**Done when:** CI is green, `scripts/verify_spec.py` passes including its negative cases, and
the schema and `docs/02-manifest-schema.md` example validate against each other.

---

## T1 — `skillaudit-core`: manifest types and canonical serialization

Build the spine before anything that produces data.

- Rust types mirroring `schema/manifest-v1.schema.json`, with the three finding tiers as
  **separate types in separate arrays** (invariant 5).
- One canonicalization function, the only public serialization path. Sorted keys, declared
  array orders (see `docs/02-manifest-schema.md`), LF, two-space indent, trailing newline.
- Round-trip property test: serialize → parse → serialize is a fixed point.
- Validation against the JSON Schema in CI, so the schema and the types cannot drift.

**Done when:** a hand-built manifest with shuffled input ordering serializes byte-identically
across 1,000 randomized field-insertion orders, and validates against the schema.

---

## T2 — `skillaudit-parse` + `skillaudit-resolve`: bundles and inventory

- `Resolver` trait; `claude-code` impl first, second resolver chosen by T4 data.
- Frontmatter parsing, file walk, per-file SHA-256, merkle `content_digest`.
- **Load-phase classification** (`always` / `on_trigger` / `reference` / `unreferenced`) via
  the reference graph from `SKILL.md`. This is the core signal — get it right before
  anything else consumes it.
- Symlink escape, size limits, and binary files all produce `unresolved` entries, never
  silent skips.

**Done when:** running on `anthropics/skills` produces a valid manifest per bundle with an
empty `capabilities` array (no rules yet) and a fully populated inventory, byte-identical
across two runs on two platforms.

---

## T3 — `skillaudit-corpus`: the harvest

Depends on T2 because it reuses the parser. See `docs/01-corpus-scan.md` for sources,
sampling discipline, and the full measurement list.

- Requires `GITHUB_TOKEN`; fails fast and clearly without one.
- Content-addressed archive under `corpus/raw/<digest>/`; re-runs must not re-fetch.
- Records discovery provenance per bundle so head and tail can be reported separately.
- Emits `corpus/index.json` and `corpus/report.md`.

**Done when:** the report states base rates with denominators and sampling bias, and the
format-scope decision rule (≥5% presence) has been applied to pick v1 resolvers.

**This is the kill gate.** If the numbers are boring, publish the negative result and stop.

---

## T4 — `skillaudit-rules` + `skillaudit-code`: the engine

- Rule loader and validator (`skillaudit rules validate`), tree-sitter query execution,
  capture → finding mapping with provenance.
- Reachability: call graph from entry points; `observed` / `present` / `unresolved`.
  Intra-file plus direct cross-file imports only. Do not claim more than you established.
- Zero language-specific code in the engine. The reference rule triple in
  `rules/python/credential-read.*` is the contract — if implementing it requires an engine
  special case, fix the engine.
- Port rules for the languages T3 showed actually matter, in that order.

**Done when:** every rule's fixtures pass, `unsupported_language` is emitted for everything
unported, and the adversarial "sink in dead code" case reports `present` rather than
`observed`.

---

## T5 — `skillaudit-instr`: instruction plane

- `tier = "pattern"`, markdown grammar, `instruction.*` namespace only.
- Write three negative fixtures **drawn from real corpus bundles** before writing the query
  for `instruction.silence` and `instruction.privilege_claim`.

**Done when:** false-positive rate on the benign stratum is measured and published, per
signal.

---

## T6 — `skillaudit-eval`: harness and CI gate

See `docs/05-eval.md`. Three suites: fixture, corpus, adversarial. Per-capability metrics,
`unresolved` rate tracked, held-out split fixed by seed.

**Done when:** CI fails on a seeded regression, and the README carries published numbers
with the corpus version that produced them.

---

## T7 — `skillaudit-semantic`: the quarantined pass

See `docs/04-semantic-layer.md`. Built now, not earlier, because it is measured against the
labels from T3.

- Crate does not depend on `skillaudit-code` or `skillaudit-instr` — quarantine enforced by
  the dependency graph, not by review.
- Deleting the `advisory` key from output must lose nothing else.

**Done when:** the red-team injection fixture produces an `injection_attempt` finding and
provably does not alter any deterministic branch; variance across n runs is reported.

**Cut criterion:** if T3 labels show disclosure delta in under ~3% of bundles, ship v1.0
without this and say so in the README.

---

## T8 — `skillaudit-policy` + `skillaudit-diff` + CI action

- `policy.toml`: per-repo capability allowlist, exit codes.
- `skillaudit.lock`: digest + capability set only, human-reviewable in a PR.
- Diff: capability escalation detection between lock and recompute.
- GitHub Action wrapping the CI subcommand.

**Done when:** a fixture skill that gains `fs.read.credential` in v1.1 causes a failing check
whose output a reviewer can act on in under ten seconds. **This is the product** — everything
above exists to make this line trustworthy.

---

## T9 — distribution

Rust binary; npm wrapper with per-platform `optionalDependencies` holding prebuilt binaries,
esbuild-style. **No `postinstall` download script** — that is itself a supply-chain smell and
indefensible in this project specifically. Plus `cargo install`, Homebrew tap, reproducible
and signed releases.

**Path sanitization belongs here, not in a checked-in `.cargo/config.toml`.** Absolute build
paths and usernames must not reach the binary, but neither `--remap-path-prefix` nor Cargo's
`trim-paths` can express that portably in a committed file: `--remap-path-prefix` needs a
literal `FROM=TO` where `FROM` is the machine's own workspace path, and `trim-paths` is still
nightly-only. So it is set via `RUSTFLAGS` in the release workflow, where the actual paths are
known, and verified by the byte-identity check below rather than assumed.

**Done when:** two builds of the same tag from clean checkouts are byte-identical.

---

## Cross-cutting, every task

The definition-of-done checklist at the bottom of `AGENTS.md` applies to all of the above.

---

## Known gaps

Tracked here rather than left to be rediscovered. None is a blocker for the task it sits in
front of; each is a thing this repository currently claims or implies but does not yet have.

- **`policy.toml` has no format spec.** Referenced in `AGENTS.md` (invariant 1),
  `docs/02-manifest-schema.md`, and T8. Writing it before T8 would be speculation, since the
  exit-code semantics depend on what the diff turns out to need.
- **`skillaudit.lock` is specified in one sentence.** Enough to build against at T8, not
  enough for a third party to write a compatible reader. Expand when the diff exists.
- **`rules/languages.toml` does not exist.** The extension → grammar mapping described in
  `docs/03-rules-authoring.md`. A T4 input.
- **The taxonomy has thirteen terms and the repository has one rule.** Invariant 12 forbids
  shipping a term no rule detects, so v1.0 either grows rules to cover the taxonomy or the
  taxonomy shrinks to match the rules. T4 decides which, from T3's data — not from ambition.
- **`OWNER` is a placeholder** in `Cargo.toml`'s `repository` and the schema `$id`, and
  `skillaudit` is a placeholder name. `AGENTS.md` says do not defer past v0.2.
