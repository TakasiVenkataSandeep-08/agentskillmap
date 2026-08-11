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

## T1 — `skillmap-core`: manifest types and canonical serialization

Build the spine before anything that produces data.

- Rust types mirroring `schema/manifest-v1.schema.json`, with the three finding tiers as
  **separate types in separate arrays** (invariant 5).
- One canonicalization function, the only public serialization path. Sorted keys, declared
  array orders (see `docs/02-manifest-schema.md`), LF, two-space indent, trailing newline.
- Round-trip property test: serialize → parse → serialize is a fixed point.
- Validation against the JSON Schema in CI, so the schema and the types cannot drift.

**Done when:** a hand-built manifest with shuffled input ordering serializes byte-identically
across 1,000 randomized field-insertion orders, and validates against the schema.

**Status: done.** `crates/skillmap-core` ships the types, `canonicalize()`, and
`content_digest()`. The determinism suite runs the 1,000-shuffle criterion above plus
serialize→parse→serialize fixed-point, idempotence, framing, key-sort, and no-float checks.

Two decisions worth recording, because neither is obvious from the code:

- **Ties are broken by each element's canonical JSON rendering.** The declared array orders
  are not total on their own; see `docs/02-manifest-schema.md`. Without a tiebreak, two
  findings agreeing on every declared key would serialize in whatever order the analysis
  emitted them, which is exactly the tie-dependent nondeterminism the spec warns about.
- **Schema validation lives in Python, not in a Rust dev-dependency.** The drift gate is
  split: `--test golden` proves the types still render
  `crates/skillmap-core/tests/golden/manifest-maximal.json` byte for byte, and
  `scripts/verify_spec.py` (check `golden-manifest`) proves that file still satisfies the
  schema, whose `additionalProperties: false` catches a field added on the Rust side. The
  `jsonschema` crate would have pulled tokio, hyper, and reqwest into the tree of a tool
  whose `SECURITY.md` promises a minimal dependency set and no network.

Re-bless the golden manifest after an intentional shape change with
`SKILLMAP_BLESS=1 cargo test -p skillmap-core --test golden`, and bump `schema_version` in
the same commit — a manifest shape change is a schema-version event.

---

## T2 — `skillmap-parse` + `skillmap-resolve`: bundles and inventory

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

**Status: done, against a local corpus rather than `anthropics/skills`.** The criterion is
met by `fixtures/bundles/`, whose blessed manifests are byte-compared on every run and
schema-validated by `scripts/verify_spec.py`; the two-platform half is CI's `rust` matrix.
Running against the real `anthropics/skills` corpus is deliberately deferred to **T3**, which
is the harvester and the task that owns fetching third-party bundles at all. Until T3, no
part of this repository downloads anything (invariant 9).

Decisions worth recording:

- **Frontmatter is parsed by a strict subset parser, not a YAML library.** It accepts
  `key: scalar`, quoted scalars, flow and block sequences, and block scalars, and **refuses
  everything else with a line number** — anchors, aliases, merge keys, tags, nesting,
  duplicate keys. A general engine would accept constructs no `SKILL.md` needs, some of
  which (alias expansion) are a denial-of-service shape, in the first untrusted bytes this
  tool touches; and `serde_yaml`, the obvious pick, was archived by its author in 2024.
  Refusing loudly is a first-class outcome here (invariant 3). If real bundles turn out to
  need more of YAML, T3's corpus is what will say so with a denominator.
- **No file is classified `always`.** The always-loaded content is the frontmatter
  *description*, which lives inside `SKILL.md` rather than in a file of its own, and is
  reported as `disclosure.description_bytes`. Tagging `SKILL.md` itself `always` would claim
  its body is seen at session start — the exact false comfort this tool exists to dispel.
- **`inventory[].size` is the number of bytes hashed, not what `stat` reports.** A CRLF
  checkout has more bytes on disk than an LF one; reporting the on-disk figure made the same
  bundle produce two different manifests on two platforms even though `sha256` matched. Found
  by a test, not by review.
- **The reference graph follows links out of any text file, not just markdown.** A helper a
  script imports is reachable by a documented path; reporting it as `unreferenced` would
  drown the one signal that matters.
- **The fixture corpus is stored flat, not under a real `.claude/skills/` tree.** A committed
  `.claude/skills/` is a *live* skill directory — every agent reading this repository would
  load the fixtures as installed skills, and one of them is deliberately shaped like an
  exfiltration payload. Discovery against the real convention is tested in a scratch
  directory instead.

Still open, tracked below: plugin-wrapped bundles (`.claude/plugins`) and a second resolver,
chosen by T3 data per this task's own note. `unsupported_language` was the third item here and
is now emitted by T4's code plane.

---

## T3 — `skillmap-corpus`: the harvest

Depends on T2 because it reuses the parser. See `docs/01-corpus-scan.md` for sources,
sampling discipline, and the full measurement list.

- Requires `GITHUB_TOKEN`; fails fast and clearly without one.
- Content-addressed archive under `corpus/raw/<digest>/`; re-runs must not re-fetch.
- Records discovery provenance per bundle so head and tail can be reported separately.
- Emits `corpus/index.json` and `corpus/report.md`.

**Done when:** the report states base rates with denominators and sampling bias, and the
format-scope decision rule (≥5% presence) has been applied to pick v1 resolvers.

**This is the kill gate.** If the numbers are boring, publish the negative result and stop.

**Status: the harvester is built and tested; the harvest itself has not been run.**

The pipeline is complete — sources, fetch caching, content-addressed archive, measurement,
`index.json`, `report.md` — and exercised end to end by tests that use a local `Fetcher`
instead of the network. What has *not* happened is a real run: that needs a `GITHUB_TOKEN`
and fetches thousands of third-party repositories, which is the operator's call to make and
the operator's credentials to use, not something to do on their behalf.

**So the kill-gate decision is still open.** No base rate in this repository has been
measured against real bundles yet. To run it:

```bash
GITHUB_TOKEN=... cargo run -p skillmap-corpus -- --snapshot 2026-08
```

Decisions worth recording:

- **`ureq` for search, `git clone` for contents.** The corpus crate is the only one in the
  workspace that touches the network, which is why the HTTP dependency lives there alone. It
  is used exclusively for authenticated GETs returning small JSON; the bulk transfer goes
  through the git the operator already has. `reqwest` would have pulled tokio, hyper, and
  roughly 130 crates into a supply-chain auditor's tree — the same tree refused for
  `jsonschema` in T1.
- **Lexical measurements are labelled as upper bounds and never become findings.** The
  capability-surface counts are substring matches: they do not parse, establish no
  reachability, and carry no provenance, so presenting them as tier-`proven` would blend an
  assurance tier (invariant 5) and overstate what was established (invariant 4). They exist
  to size the problem and to tell T4 which grammars to write first. `report.md` says all of
  this above the table, not in a footnote.
- **Rates are integer arithmetic; there are no floats anywhere.** Percentages are computed
  in tenths and byte shares in parts-per-million, so `index.json` diffs cleanly between
  snapshots and prints identically on every platform.
- **The fetch cache is keyed on the pinned commit,** not on a branch name. Keyed on a branch
  it would serve last month's contents as this month's, and the corpus would not be
  reproducible — which is the one property that makes a published base rate checkable.
- **Bundles are deduplicated by content digest.** The same bundle vendored into five
  repositories is one row. Without that the base rates over-count whatever is most copied,
  which is exactly the popular material.

The toolchain pin moved to a current stable in this task: `ureq` pulls `url` → `idna` →
`icu_*`, which require rustc 1.86 or newer.

---

## T4 — `skillmap-rules` + `skillmap-code`: the engine

- Rule loader and validator (`skillmap rules validate`), tree-sitter query execution,
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

**Status: the engine is done. Language breadth is still gated on T3.**

All three clauses hold, each with a test: the reference triple's fixtures pass, an unported
language produces `unsupported_language` rather than silence, and the credential read inside
`collect()` — which nothing calls — reports `present`. A companion test asserts the same sink
reports `observed` once something calls it, so the dead-code result cannot be passing because
`present` is returned unconditionally.

**Only Python is ported, and that is the part T3 decides.** This task says to port rules "for
the languages T3 showed actually matter, in that order", and T3 has not been run. Python was
built because the reference rule triple already contracts for it; picking the next language
without the corpus data would be exactly the ambition-over-evidence move the build order
exists to prevent.

Decisions worth recording:

- **Reachability is data-driven too.** Invariant 7 would be half-honoured if sinks were data
  but the engine still hardcoded what a function definition looks like. `queries/<lang>/_reachability.scm`
  supplies four roles — `def.name`, `def.span`, `call.name`, `call.dynamic` — and the engine
  knows nothing else. Adding a language is a grammar dependency, a registry line, and two
  query files.
- **`present` and `unresolved` are different claims.** `present` says the analysis looked and
  found no caller. `unresolved` says a computed callee blocked it from being able to say.
  Collapsing them would be the silent-drop failure invariant 3 exists to prevent, so a file
  containing `globals()[name]()` reports `unresolved` for anything not statically reached
  rather than confidently reporting `present`.
- **A file nothing documents never reports `observed`.** Reachability reuses T2's load phase:
  `unreferenced` means nothing established that those bytes run, so the strongest claim
  available is `present` even for a module-level sink.
- **The `[match]` filter is what makes the negative fixture pass**, not the query. `negative.py`
  contains a real `open()` call; it is rejected because `templates/default.toml` matches no
  credential prefix. That is the designed division of labour — extend the TOML list, never
  narrow the query.
- **`expected.json` is now generated, not hand-written.** Re-bless with
  `SKILLMAP_BLESS=1 cargo test -p skillmap-code`; this becomes `skillmap rules bless` at T9.
  A second test asserts the specific claims the docs make about that fixture, so blessing
  cannot quietly record a wrong answer.

---

## T5 — `skillmap-instr`: instruction plane

- `tier = "pattern"`, markdown grammar, `instruction.*` namespace only.
- Write three negative fixtures **drawn from real corpus bundles** before writing the query
  for `instruction.silence` and `instruction.privilege_claim`.

**Done when:** false-positive rate on the benign stratum is measured and published, per
signal.

---

## T6 — `skillmap-eval`: harness and CI gate

See `docs/05-eval.md`. Three suites: fixture, corpus, adversarial. Per-capability metrics,
`unresolved` rate tracked, held-out split fixed by seed.

**Done when:** CI fails on a seeded regression, and the README carries published numbers
with the corpus version that produced them.

---

## T7 — `skillmap-semantic`: the quarantined pass

See `docs/04-semantic-layer.md`. Built now, not earlier, because it is measured against the
labels from T3.

- Crate does not depend on `skillmap-code` or `skillmap-instr` — quarantine enforced by
  the dependency graph, not by review.
- Deleting the `advisory` key from output must lose nothing else.

**Done when:** the red-team injection fixture produces an `injection_attempt` finding and
provably does not alter any deterministic branch; variance across n runs is reported.

**Cut criterion:** if T3 labels show disclosure delta in under ~3% of bundles, ship v1.0
without this and say so in the README.

---

## T8 — `skillmap-policy` + `skillmap-diff` + CI action

- `policy.toml`: per-repo capability allowlist, exit codes.
- `skillmap.lock`: digest + capability set only, human-reviewable in a PR.
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
- **`skillmap.lock` is specified in one sentence.** Enough to build against at T8, not
  enough for a third party to write a compatible reader. Expand when the diff exists.
- **The taxonomy has thirteen terms and the repository has one rule.** T4 built the engine
  that runs them; it deliberately did not grow coverage, because which capabilities matter is
  what T3 is for. Invariant 12 forbids shipping a term no rule detects, so v1.0 either grows
  rules to cover the taxonomy or the taxonomy shrinks to match the rules — decided from the
  corpus, not from ambition.
- **Only Python has a grammar.** Every other language reports `unsupported_language`. That is
  honest, and thin. T3's corpus decides which grammar comes next, in what order.
- **Plugin-wrapped bundles are not discovered.** `BundleKind::Plugin` exists in the schema
  because the manifest format has to be able to describe them, but the `claude-code` resolver
  does not walk `.claude/plugins` and never returns that kind. Returning it from a code path
  that cannot produce one would be a stub (invariant 12). A T3 input: the corpus will say how
  common plugin wrappers actually are before a walker is written for them.
- **Only one resolver exists.** T2's own note defers the second to whatever T3's data shows
  matters, rather than guessing between Cursor, Codex, and Windsurf.
- **`cargo deny` is configured but never runs.** `deny.toml` has existed since T0 and no CI
  job invokes it, so the supply-chain gate `SECURITY.md` promises is currently aspirational.
  This matters more since T3: `ureq` brought rustls and `ring` into the tree, whose licences
  (ISC, and `ring`'s OpenSSL-derived terms) may not satisfy the current allowlist. Adding the
  job is small; it is listed here rather than done silently because it will probably fail
  first time and that failure needs a decision, not a rubber stamp.
- **The frontmatter subset is unvalidated against real bundles.** The parser refuses anything
  outside the documented `SKILL.md` shape (see T2). Whether real skills stay inside it is a
  T3 measurement, not a guess — widening it before there is a denominator would be the wrong
  order.
