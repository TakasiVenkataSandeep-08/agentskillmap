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

**The language set now follows the corpus, as this task required.** T3's harvest ranked
python 5.1%, shell 3.3%, javascript 2.4%, typescript 1.0% of bundles, and those four are
exactly what is ported — each with a grammar, a reachability query, and a `credential-read`
rule triple with both fixtures.

Adding a grammar without a rule would have been a regression, not a no-op: a file whose
language has a grammar is no longer reported as `unsupported_language`, so shell scripts
would have gone from an honest "not analyzed" to silence. That is why each grammar landed
with a rule rather than ahead of one.

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

**Status: the plane is built and three of five signals ship. T5 is NOT done, and cannot be
until T3 runs.**

The "done when" above *is* a corpus measurement. There is no benign stratum without the
harvest, so the false-positive rate is unmeasured and nothing is published. That is the honest
state, not a technicality to wave through.

**Two signals are deliberately withheld.** This task requires three negative fixtures drawn
from real corpus bundles *before* the queries for `instruction.silence` and
`instruction.privilege_claim` are written — they are the signals most likely to earn this
project attention and most likely to false-positive on ordinary skills that discuss logging
verbosity or permission handling. Those fixtures do not exist, so those queries are not
written. `the_two_riskiest_signals_are_deliberately_not_shipped` fails if either appears, so
shipping them without corpus negatives has to be a deliberate act that deletes an assertion.

Shipped: `instruction.fetch_as_instruction`, `instruction.exfil`,
`instruction.config_mutation` — each a full triple with a positive and a negative fixture.

Decisions worth recording:

- **The negatives are real prose, not invented.** Each one is drawn from this repository's own
  security and architecture documentation, which describes exfiltration, indirect prompt
  injection and agent-config writes at length without instructing any of them. Prose *about* a
  behaviour is the hardest false positive a lexical rule faces. This is a stand-in for the
  corpus negatives the review checklist asks for, and it is not a substitute — it is one
  document set, written by one project, with a house style.
- **`no_instruction_rule_fires_on_this_repositorys_own_documentation`** runs every rule over
  every markdown file in the repository and requires zero hits. It is the closest thing to a
  measured false-positive rate available pre-corpus, and it is what caught the bug below.
- **Instruction findings carry `EvidenceStrict`, the same full provenance as the code plane.**
  Invariant 4 says "No exceptions, including for instruction-plane findings". A weak tier is
  not a licence for weak provenance: a lexical match still fired at an exact byte range and a
  reviewer must be able to read the sentence.
- **Prose has no reachability, so `reachability` in `rules/languages.toml` is now optional.**
  Asking whether a sentence is reachable is a category error — a paragraph nothing links to is
  still one the agent reads. A language without a reachability query is analyzed by the
  instruction plane only, and the code plane never reports `observed` for it.
- **A grammar without `proven` rules is not `unsupported_language`.** Markdown has a grammar
  now, so the code plane saying nothing about a `.md` file is correct rather than a gap;
  claiming the analysis could not read it would be false.

One bug worth naming, because it would have been invisible without the negatives: a
tree-sitter `#match?` predicate must be **grouped with the node it constrains** by an extra
pair of parentheses. Written without them the predicate binds to nothing, the pattern
degenerates to "every inline node", and all three rules fired on every sentence in the
repository — including headings like "A rule is a triple". Every positive fixture still passed.
Only the negatives caught it, which is invariant 8's entire argument in one incident.

---

## T6 — `skillmap-eval`: harness and CI gate

See `docs/05-eval.md`. Three suites: fixture, corpus, adversarial. Per-capability metrics,
`unresolved` rate tracked, held-out split fixed by seed.

**Done when:** CI fails on a seeded regression, and the README carries published numbers
with the corpus version that produced them.

**Status: the gate is done. The published numbers are not, and cannot be until T3 runs.**

Clause one holds. `cargo run -p skillmap-eval` runs on both CI platforms and exits non-zero
on three separate regressions, each seeded by its own test:

- a case that used to pass now fails;
- **coverage shrank** — a case was deleted;
- **a case fell back to pending** — it was silenced without being deleted.

The last two matter more than the first. Both make the suite *greener* while checking less,
and a gate that only counted failures would wave both through. Deleting a failing test is the
easiest way to get a green build, so the baseline records how many cases executed and the gate
treats a fall as a regression.

**Clause two is now partly met.** The harvest ran, and the README carries published numbers
naming corpus snapshot `2026-08` and the commit that produced it. What it publishes are the
*corpus base rates* — exact, mechanical, with denominators and the head/tail split — not
quality metrics.

What is still missing is the labelling pass. `docs/01-corpus-scan.md` calls for ~150
hand-labelled bundles as ground truth; without them there is no held-out split, no precision
or recall per capability, and no false-positive rate on a benign stratum — which
`docs/05-eval.md` names as *the headline metric*. `eval/baseline.json` therefore still carries
`corpus_snapshot: null`, and the test enforcing that was rewritten: the field stays empty not
because no corpus exists, but because the eval has never been run against a labelled one.

The original blocking statement, kept for the record: published numbers must name the corpus
version and commit that produced them, and there was no corpus.

**All eight adversarial cases from `docs/05-eval.md` are declared; five run.** The three that
cannot are `obfuscated-exec` (needs a `code.obfuscation` rule), `injection-in-reference`
(needs T7), and `capability-added-in-update` (needs T8). They are present as real bundles with
declared expectations and marked pending with a reason, not omitted — a suite that silently
covered five of eight would be the same false comfort invariant 3 rejects, one level up.

Decisions worth recording:

- **Adversarial cases are data.** Each is `fixtures/adversarial/<id>/` — a real bundle plus an
  `expect.toml`. Adding a red-team case is a directory; nobody should have to edit Rust to
  attack the scanner, for the same reason nobody should have to edit Rust to add a rule.
- **The quiet cases are asserted directly.** `docs/05-eval.md` says the last two matter as much
  as the rest, so `documented-credential-path` and `legitimate-deploy` have their own test
  rather than being folded into an aggregate that could hide them.
- **Invariant 1 is checked mechanically.** The `no_verdict` expectation scans the serialized
  manifest for verdict language and for floats. It found one bug immediately: a naive textual
  float scan flagged `"version": "0.1.0"`, so the check parses the JSON and asks whether a
  *number* is fractional. Only the parser can answer that question.
- **The tolerance is zero, deliberately.** `docs/05-eval.md` allows a declared tolerance, but a
  tolerance is only meaningful over a statistical corpus. On a deterministic fixture suite it
  would just be permission to break one case.
- **`skillmap-eval` is where the three planes are first assembled** into one manifest, because
  eval is the first thing that needs a whole one. T9's CLI lifts that function; a crate whose
  only job is to call three others, written before there is a second caller, would be a stub.

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

Built. **Two of the three "done when" clauses are met and the third is not**, and the split
is worth stating precisely rather than rounding up:

- *"provably does not alter any deterministic branch"* — **done**, and proved rather than
  argued. `crates/skillmap-scan/tests/quarantine.rs` scans the same bundle with no semantic
  pass, with one that finds nothing, and with one returning output written to suppress a
  deterministic finding, then compares the deterministic half of the manifest byte for byte.
- *"the red-team injection fixture produces an `injection_attempt` finding"* — **not done**,
  because it needs a live model. What is proved is that a *relayed* injection is
  reclassified as `injection_attempt` and never acted on, and that the fixture's
  deterministic branches are unmoved by a hostile response.
- *"variance across n runs is reported"* — **the harness exists and has never been run.**
  `skillmap-semantic::variance` reports per kind, omits kinds that never fired, and counts
  failed runs. Numbers require credentials this repository does not have and must not
  fabricate.

Decisions worth recording:

- **The cut criterion could not be evaluated, and was not quietly skipped.** It reads "if T3
  *labels* show…", and there are no labels. The nearest proxy is the corpus's lexical
  disclosure-delta column: markers appearing **only** in files no documented path reaches —
  credential paths 1.6%, secret env vars 1.6%, `eval`/`exec` 2.9%, encoding chains 1.3%,
  agent-config writes 0.4%, network 9.6%. Every high-signal marker is under the 3% line and
  the union across markers is unmeasured, so the proxy points at "cut" without being able to
  say so. The layer therefore ships built, off by default, and unmeasured, and the README
  says exactly that. **This is the decision the labelling pass exists to make**, and it is
  the strongest remaining argument for doing it.
- **The quarantine is enforced three ways, and only one of them is the dependency graph.**
  The `Cargo.toml` has no `skillmap-code` and no `skillmap-instr`, as required — but the
  input type carries the weight. `BundleView` holds a description and file text and has no
  field for a capability, so the pass cannot read a deterministic finding, and
  `bundle_view()` in `skillmap-scan` does not take the `Manifest` at all.
- **The prompt hash covers two files, not one.** `prompts/auditor-directed.toml` decides
  whether a finding is reclassified, so hashing only the template would leave a hole where
  the advisory branch's output changes and `prompt_sha256` says nothing did.
- **A fenced response is rejected.** Models fence JSON habitually and the prompt asks them
  not to; unwrapping one is small, reasonable, and the first step of the lenient path
  `docs/04-semantic-layer.md` names as how injection wins. The cost is a diagnostic somebody
  reads. Listed in Known gaps because it is a real usability risk that no measurement has
  been taken of.
- **Hallucinated citations are discarded.** A model naming a file the bundle does not contain
  loses the finding. An advisory finding's whole value is that a human can check it in
  seconds, and one that leads nowhere has none.
- **`docs/04-semantic-layer.md` contradicts itself about `unresolved`**, and the resolution is
  recorded there rather than picked silently: the pass appends its own `size_limit` coverage
  gaps and can never modify an entry a deterministic tier wrote.
- **The eval case stayed pending, and its reason changed.** `injection-in-reference` now says
  `needs a live model` rather than `needs T7`. T7 landed and the case still cannot run: the
  eval gate is offline (invariant 9) and deterministic (invariant 2), and a case pointed at a
  replay provider would assert what a fixture author typed. Adversarial coverage did not
  grow, and inflating it would have been the easy lie here.

---

## T8 — `skillmap-policy` + `skillmap-diff` + CI action

- `policy.toml`: per-repo capability allowlist, exit codes.
- `skillmap.lock`: digest + capability set only, human-reviewable in a PR.
- Diff: capability escalation detection between lock and recompute.
- GitHub Action wrapping the CI subcommand.

**Done when:** a fixture skill that gains `fs.read.credential` in v1.1 causes a failing check
whose output a reviewer can act on in under ten seconds. **This is the product** — everything
above exists to make this line trustworthy.

Delivered. `fixtures/projects/v1.0` and `v1.1` are the pair; `crates/skillmap-cli/tests/escalation.rs`
runs the real binary against them and asserts the exit code, every field of the report, and
that the whole thing fits in eight lines. Format spec: `docs/06-policy-and-lock.md`.

Decisions worth recording:

- **Two questions, two exit codes.** *Is this capability new?* is the lock's question;
  *is it allowed here at all?* is the policy's. A skill can hold a permitted capability it
  did not have yesterday, and can hold a forbidden one it has held all along. Collapsing them
  into one failure makes both unreadable, so escalation exits `1`, policy exits `2`, both
  exits `3` — and `4` means the run did not happen.
- **`4` is invariant 3 at the process boundary.** An empty ruleset produces a clean scan of
  everything, which is the worst output this tool could emit. `skillmap` refuses to scan with
  zero rules loaded rather than reporting a confident silence, and a missing lock is an error
  rather than an empty baseline — treating absence as "no capabilities" would fail the first
  run in every repository and teach people the check cries wolf.
- **Absent `policy.toml` ≠ empty `policy.toml`.** Absent is no opinion, and the policy half
  simply does not run (loudly, on stderr). Present-and-empty is the opinion "nothing is
  permitted". Collapsing them costs dearly either way: permissive-by-default silently
  approves everything, restrictive-by-default fails every repository's first run.
- **The lock stores capability wire names, not the enum.** A lock outlives the binary that
  wrote it. An older build that dropped unknown terms would silently rewrite the lock, and the
  next run of a newer build would report the losses as fresh escalations.
- **`skillmap-scan` was extracted, on the condition its own comment set.** Manifest assembly
  lived in `skillmap-eval` from T6 with a note saying it would move once a second caller
  existed. `skillmap ci` is that caller, and a product binary depending on the test harness
  to scan would have the dependency arrow backwards.
- **`skillmap-cli` exists a task early.** T9 owns distribution, not the binary: a check nobody
  can run does not satisfy a "done when" measured in a reviewer's seconds. It has two
  subcommands and hand-rolled flag parsing — clap's tree is not worth four flags in a project
  whose SECURITY.md promises a small one.
- **Rules are not embedded in the binary yet**, so `--rules` must point at a checkout. That is
  invariant 7 working as designed everywhere except distribution; T9 packages them. Named in
  Known gaps rather than papered over with a fallback that might find the wrong tree.
- **skillmap gates skillmap.** CI runs `skillmap ci` against this repository's own two skills
  with a committed `skillmap.lock` and `policy.toml`. The allowlist is empty, which is the
  honest answer for two prose-only skills and a claim CI now enforces.
- **Two fixture-discovery blocklists became rule-driven.** Adding `fixtures/projects/` made
  both `skillmap-code`'s fixture test and the eval fixture suite read the version directories
  as languages. Both now ask the ruleset what a language is, which cannot fall out of date the
  way "skip these three directory names" did.

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

Delivered, and verified rather than asserted: two copies of the tree at different absolute
paths produce the same SHA-256, checked locally on Windows and gated in
`.github/workflows/release.yml` on Windows and Linux before anything is published. Details and
install paths: `docs/07-distribution.md`.

Decisions worth recording:

- **The binary embeds its own rules.** This was the gap that made T8's check undistributable,
  and it is not in the bullets above because nobody noticed it until there was a binary to
  ship. `crates/skillmap-rules/build.rs` walks `rules/` and `queries/` and emits them as
  literals; `Source` gives the disk and embedded trees one code path, and
  `tests/embedded.rs` compares them byte for byte. Adding a rule is still a `.toml` and a
  `.scm` — the build script walks and has no list to update, so invariant 7 holds.
- **A build that embeds no rules is refused by the build script.** Not warned about. A
  scanner with no rules reports every project clean, silently, in the direction that looks
  like good news.
- **Two bugs stood between the claim and the truth, and only one was visible.** The first:
  MSVC writes the wall clock into the PE header, which is exactly the 24 bytes by which the
  first two builds differed. The second was worse — ninety-one registry paths carrying a
  username sat in `.rodata` as panic locations, where `strip` does not reach, because the
  remap flag was spelled `/c/Users/...` while rustc emits `C:\Users\...`. **Byte-identity
  never caught it**, since both builds ran as the same user. `scripts/build-release.sh` now
  greps the finished binary for the workspace, `CARGO_HOME` and `$HOME` in both spellings and
  refuses to publish on a hit. The general lesson is the project's own: a check that can only
  confirm what you expected is not a check.
- **`scripts/homebrew-formula.sh` shipped a silent-failure bug in its first draft**, found by
  running it: `exit 1` inside `$(…)` kills only the subshell, so a missing checksum produced
  a complete, publishable formula with four empty `sha256` fields and exit code 0. Checksums
  are now resolved before the template. A formula with a wrong hash fails at install with a
  mismatch, which reads to a user exactly like a compromised download.
- **Signing is a keyless attestation, not a key.** `actions/attest-build-provenance`,
  verifiable with `gh attestation verify`. A signing key this project would have to store,
  rotate and eventually mishandle is a worse story, and the attestation answers the question
  people actually have — did this come from that repository's release workflow.
- **`cargo install` is `--git`, not crates.io**, because Cargo packages only files beneath a
  package's own directory and the rule trees live at the workspace root. Both fixes are worse
  than the gap; the reasoning is in `docs/07-distribution.md` and the gap is listed below.
- **The CLI grew `scan`, `rules` and `version`.** `rules` is the one that matters: the rules
  are no longer visible on disk beside the tool, and "which rule produced this, and what does
  it claim" is the first question anyone asks about a finding they disagree with.

---

## Cross-cutting, every task

The definition-of-done checklist at the bottom of `AGENTS.md` applies to all of the above.

---

## Known gaps

Tracked here rather than left to be rediscovered. None is a blocker for the task it sits in
front of; each is a thing this repository currently claims or implies but does not yet have.

- **~~`policy.toml` has no format spec.~~** Closed by T8: `docs/06-policy-and-lock.md`.
- **~~`skillmap.lock` is specified in one sentence.~~** Closed by the same document — fields,
  framing, escalation semantics, and why unknown capability terms round-trip.
- **~~The rules tree is not embedded in the binary.~~** Closed by T9. `--rules` survives as an
  override for developing against an edited tree.
- **The Homebrew tap repository does not exist.** `scripts/homebrew-formula.sh` generates a
  correct formula from the published checksums and the release workflow attaches it, but
  `brew install skillmap/skillmap/skillmap` resolves through `skillmap/homebrew-skillmap`,
  which has to be created before that command works.
- **`cargo install skillmap-cli` from crates.io does not work**, and the build fails loudly
  rather than producing a ruleless binary. Cargo packages only files beneath a package's own
  directory, and `skillmap-rules` embeds `rules/` and `queries/` from the workspace root.
  `cargo install --git` works. See `docs/07-distribution.md` for why moving the trees into the
  crate, or committing a synchronized copy, are both worse than the gap.
- **CI syntax-checks the release scripts but does not lint them.** `bash -n` catches parse
  errors; shellcheck would catch more, including some of the class that produced T9's
  silent-failure bug. It is preinstalled on the runners and was left out only because it could
  not be run locally first, and a gate whose first execution is on somebody else's push is how
  a repository acquires a red build it did not author.
- **Reproducibility is verified within a runner, not across machines.** The release gate builds
  the same commit twice from two directories and compares. Two independent machines agreeing
  is the stronger claim and needs a second builder this project does not have.
- **The semantic layer is unmeasured, and that is now the largest gap in the repository.**
  T7 built it; `docs/04-semantic-layer.md` requires precision, recall, a benign-stratum
  false-positive rate and variance across n runs, and none exist because the corpus is not
  labelled. It ships off by default and the README says so. **The cut criterion cannot be
  evaluated until the labelling pass runs**, and the lexical proxy currently points at
  cutting it.
- **The semantic pass rejects a fenced JSON response.** Models fence habitually; the prompt
  asks them not to, and tolerating it is the first step of the lenient path
  `docs/04-semantic-layer.md` warns about. Whether real responses fence often enough to make
  this unusable is a measurement nobody has taken, and the right time to take it is the same
  run that produces the variance numbers.
- **`skillmap ci` scans `Scope::Project` only.** Skills installed under the user's home
  directory apply to every project and are not checked. Discovery already supports the scope;
  what is missing is an answer to which lockfile they belong in, and guessing would produce a
  lock that differs per machine — invariant 2's most obvious failure mode.
- **The taxonomy has thirteen terms and the repository has one rule.** T4 built the engine
  that runs them; it deliberately did not grow coverage, because which capabilities matter is
  what T3 is for. Invariant 12 forbids shipping a term no rule detects, so v1.0 either grows
  rules to cover the taxonomy or the taxonomy shrinks to match the rules — decided from the
  corpus, not from ambition.
- **Four languages have grammars: python, shell, javascript, typescript.** That set is the
  corpus ordering (5.1%, 3.3%, 2.4%, 1.0% of bundles), not a preference. Ruby, Go, Rust and
  the rest still report `unsupported_language`, which is honest. `.tsx` is deliberately absent:
  it needs the separate tsx grammar handle, and listing it under `typescript` would parse JSX
  with a grammar that cannot read it.
- **Plugin-wrapped bundles are not discovered.** `BundleKind::Plugin` exists in the schema
  because the manifest format has to be able to describe them, but the `claude-code` resolver
  does not walk `.claude/plugins` and never returns that kind. Returning it from a code path
  that cannot produce one would be a stub (invariant 12). A T3 input: the corpus will say how
  common plugin wrappers actually are before a walker is written for them.
- **Only one resolver exists.** T2's own note defers the second to whatever T3's data shows
  matters, rather than guessing between Cursor, Codex, and Windsurf.
- **~~`cargo deny` is configured but never runs.~~** Closed. The `supply-chain` job runs all
  four checks on every push, and the first real run found two things: `webpki-roots` ships
  under CDLA-Permissive-2.0 (a permissive *data* licence covering the Mozilla CA root store —
  now allowed, with the reasoning recorded in `deny.toml`), and the workspace was **quietly
  unpublishable**, because intra-workspace `path` dependencies carried no `version` and
  crates.io rejects those. That second one would have surfaced at T9's first release attempt.
  One duplicate remains and is skipped with a stated retirement condition: `syn@2.0`, reached
  through `ureq -> url -> idna -> icu_*`, and a proc-macro that never ships in the binary.
- **The frontmatter subset is unvalidated against real bundles.** The parser refuses anything
  outside the documented `SKILL.md` shape (see T2). Whether real skills stay inside it is a
  T3 measurement, not a guess — widening it before there is a denominator would be the wrong
  order.
