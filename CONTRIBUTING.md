# Contributing to skillmap

This repository is **pre-alpha**. Nothing runs yet — there is no `skillmap` binary, no
`cargo` workspace member outside what `docs/00-tasks.md` has already unlocked, and the
commands referenced below (`skillmap rules validate`, `skillmap rules bless`) do not
exist yet. `docs/00-tasks.md` is the source of truth for what is being built, in what order,
and what "done" means for each stage. Do not start a task whose predecessor's acceptance
criteria are unmet — the ordering is load-bearing, not a suggestion (see `AGENTS.md`, build
order).

Read `AGENTS.md` before anything else. It is canonical — the twelve invariants there
constrain every PR in this repository, including this one. This file does not restate them;
it explains how to act on them.

## Project layout, briefly

```
crates/            Rust workspace members — added only when their task begins
rules/<lang>/       rule metadata (TOML) — data, not code
queries/<lang>/     tree-sitter queries (.scm) — data, not code
fixtures/<lang>/    positive.*, negative.*, expected.json per rule
docs/               design docs; 00-tasks.md is the ordered backlog
schema/             JSON Schema for the manifest
```

Full layout and data flow: `ARCHITECTURE.md`.

---

## Add a detection rule in 15 minutes — no Rust required

Invariant 7 is a bet: Rust attracts an order of magnitude fewer drive-by contributors than
Go, and rule coverage is what determines this project's long-term quality, so detection
coverage cannot be allowed to require Rust. This section is what backs that bet. If you can
read a tree-sitter query and edit a TOML file, you can add a rule — no crate ever needs to
change.

The engine itself doesn't exist yet (`skillmap-rules` and `skillmap-code` are task **T4**
in `docs/00-tasks.md`), so a rule PR today can't be run against a live scanner. What it can
do, and what reviewers will check, is match the shape of the one rule this repository
already ships as its contract: `rules/python/credential-read.toml`,
`queries/python/credential-read.scm`, and `fixtures/python/credential-read/`. Everything
below walks through that triple. Copy it.

A rule is always three things, never one file:

```
rules/<lang>/<id>.toml        # metadata: capability, tier, capture-role map, path/host data
queries/<lang>/<id>.scm       # tree-sitter query producing captures
fixtures/<lang>/<id>/
    positive.<ext>            # must fire
    negative.<ext>            # must NOT fire
    expected.json             # canonical manifest fragment
```

### 1. Pick the capability term

Every rule maps to one term from the **closed taxonomy** in `docs/02-manifest-schema.md`
(`process.exec`, `fs.read.credential`, `code.dynamic_eval`, `net.egress`, and so on). Pick an
existing term. If none of them fit what you're detecting, stop — inventing a new term is a
**schema-version event**, not a normal PR. It touches the schema, the manifest docs, and
invariant 12 ("no capability declared in the taxonomy that no rule detects"), and needs a
design discussion first, not a query file.

### 2. Write the query — `queries/<lang>/<id>.scm`

Match structure, not text. `queries/python/credential-read.scm` matches the *shape* of a
call — `(call function: (identifier) @_fn arguments: (argument_list . (string) @path)
(#eq? @_fn "open")) @site` — rather than grepping for the substring `open(`. A skill author who
writes `pathlib.Path(x).open()` or wraps the call in a helper still gets caught by a
structural query; a textual one breaks on the first reformat.

Two hard rules for the query file:

- **Capture the smallest span that identifies the site**, and tag it `@site`. Evidence spans
  are what a human reads inside a PR or a `skillmap ci` failure — a whole-function capture
  tells a reviewer nothing they can act on in ten seconds.
- **Never put a path list, a host list, or any other literal data inside the query.** Data
  goes in the TOML's `[match]` table (step 3), specifically so a contributor extending
  coverage never has to touch tree-sitter syntax at all.

When the target can't be resolved statically — a variable, a concatenation, a computed call
— don't quietly skip it. Capture it too, tagged `@dynamic`, so the engine can emit an
`unresolved: computed_target` entry instead of silence (invariant 3). The reference query
does exactly this for `open(target)` where `target` is a variable.

### 3. Write the metadata — `rules/<lang>/<id>.toml`

```toml
id         = "py.credential-read.dotfile"
language   = "python"
capability = "fs.read.credential"     # from the closed taxonomy, step 1
tier       = "proven"                 # proven (code plane) | pattern (instruction plane)
query      = "queries/python/credential-read.scm"

[captures]
site    = "@site"
path    = "@path"
dynamic = "@dynamic"

[match]
path_prefixes = ["~/.aws/", "~/.ssh/", ".env", ".netrc"]

[docs]
summary = "..."
rationale = "..."
false_positive_notes = "..."
```

The `[captures]` table is the whole point of the split: it maps the engine's fixed **roles**
— `site` (required; the evidence span), `path`, `host`, and `dynamic` — to whatever capture
names your query happens to use. The engine only ever knows roles, never sink names, which is
the mechanism that keeps invariant 7 true instead of aspirational: a new sink, a new
language, a new obfuscation trick all reduce to the same four roles, and none of them is a
reason to open a `.rs` file. Wanting a fifth role is a real design discussion, not something
a rule PR can decide unilaterally.

Two naming rules, both checked mechanically (once `rules validate` exists, see step 5) in
both directions:

- Any capture in the query prefixed `@_` (like `@_fn` above) is **query-local** — it only
  exists to drive an `#eq?`/`#match?` predicate and is invisible to the engine. Do not
  declare it in `[captures]`.
- **Every other capture the query produces must appear in `[captures]`.** A capture the
  query emits that the TOML never declares is a rule silently dropping information on the
  floor — exactly how a detection quietly stops firing — and validation rejects it as a
  `rule_validation_error`, not a warning.

Path and host lists belong in `[match]`, not the query, so someone can add `~/.gnupg/` to
credential coverage by editing one TOML array.

### 4. Write the fixtures — `fixtures/<lang>/<id>/`

`positive.<ext>` must contain a case the query fires on. `negative.<ext>` must contain a
case that looks similar but must not fire — see the reference pair,
`fixtures/python/credential-read/positive.py` (an actual `open("~/.aws/credentials")`) versus
`negative.py` (a docstring and a string list that *mention* `~/.aws/credentials` without ever
opening it). The negative fixture is what proves the query is structural rather than a
grep in disguise.

### 5. Bless and validate

`skillmap rules bless` will fill in the byte offsets in `expected.json` once the engine
exists — those offsets are not something you hand-write, and `expected.json` in the
reference triple says so explicitly rather than pretending to be complete. `skillmap rules
validate` will then check that the query compiles, that captures and roles line up in both
directions, and that both fixtures produce their expected outcome.

**Neither command exists yet.** The engine that runs them is task **T4** in
`docs/00-tasks.md`. Until T4 lands, a rule PR is reviewed by hand against the same checklist
those commands will eventually automate (see below) — write the triple as if the tooling
existed, because it will, and your rule will be validated against exactly this contract the
day it does.

---

## Why the negative fixture is not optional

Invariant 8: a rule with no negative fixture is an untested false-positive generator. A
query that only has a positive example to prove itself against has, in practice, been tested
against nothing — you have no evidence it isn't just matching the string `open(` with extra
steps. The negative fixture is the test that would catch that.

And it has to be a **real** negative — drawn from an actual bundle in the corpus (once one
exists; today, from a real skill you can point to), not one you invent to make the rule look
good. An invented negative tests your imagination of what a false positive looks like. A real
one tests whether the ecosystem actually produces the shape you're worried about. Those are
different questions, and only the second one is useful to the project. This is spelled out
hardest for `instruction.silence` and `instruction.privilege_claim` in
`docs/03-rules-authoring.md` — they're the two signals most likely to get this project
attention and most likely to false-positive on ordinary skills that talk about logging or
permissions, so their negative fixtures are explicitly load-bearing.

## Why the tool never says "safe" or "malicious"

Invariant 1. This will come up — someone will propose a severity field, a risk score, or a
`suspicious: true` flag on a finding, usually with good intentions. The answer is no, and the
reason isn't stylistic:

Half of everything this scanner flags is legitimate. A build skill needs `process.exec`. A
deploy skill needs `net.egress`. A skill that manages cloud credentials needs
`fs.read.credential` by design. If the manifest renders a verdict, it is wrong about roughly
half of what it flags, on every scan, forever — and a tool that moralizes at its users gets
uninstalled in a day. A tool that only describes, with evidence a human can check in seconds,
becomes infrastructure instead.

Judgement belongs in `policy.toml` — a per-repo allowlist that says which capabilities are
acceptable *for that repo*, which is a decision only the repo's owners can make and that
varies completely from one project to the next. The scanner's job stops at "here is what this
skill can do, and here is the evidence." Don't propose a PR that blurs that line; it will be
rejected regardless of how good the detection underneath it is.

## What gets a PR rejected regardless of merit

- Any invariant violation in `AGENTS.md`. If a task seems to require breaking one, that's a
  signal to stop and raise it, not to work around it quietly.
- A sink name, path list, or host list hardcoded in a `match` arm or anywhere else in a
  `.rs` file. That's invariant 7's failure mode exactly — it belongs in a `.toml`.
- A rule shipped without both a positive and a negative fixture.
- A capability term added to the taxonomy with no rule behind it (invariant 12) — or a rule
  referencing a capability term that isn't in the taxonomy yet.
- A finding — in any tier — without full provenance for that tier. `proven` and `pattern`
  findings require `{ file, byte span, line, rule_id, snippet_sha256 }` in full; a finding
  nobody can point at cannot be regression-tested.
- A new `unwrap`, `expect`, or `panic!` in a library crate. Hostile, malformed input is the
  normal case here, not the edge case; a parser crash is a denial-of-service on someone's CI
  (invariant 10).

## Rule PR review checklist

This is what a reviewer will actually check on a rule PR — the same list at the bottom of
`docs/03-rules-authoring.md`:

- [ ] Negative fixture drawn from a real bundle, not invented
- [ ] Span is minimal and points at something a reviewer can act on
- [ ] Capability term already exists in the taxonomy (adding one is a schema-version event)
- [ ] `false_positive_notes` filled in honestly
- [ ] Eval metrics did not regress beyond tolerance

## Non-rule contributions

For everything that isn't a detection rule — engine code, docs, the manifest schema, CI —
`docs/00-tasks.md` is the ordered backlog. Each task lists its own acceptance criteria and
its predecessor. Please don't start T4 work before T3's acceptance criteria are met, or T2
before T1's: the ordering exists because each stage produces the input the next one actually
needs, not as process for its own sake.

Whatever you touch, the definition of done at the bottom of `AGENTS.md` applies:

- Fixtures added (positive + negative where applicable)
- Determinism test still byte-identical
- `unresolved` emitted for anything not fully analyzed
- Provenance on every new finding type
- No new `unwrap` in a library crate
- Schema version bumped if the manifest shape changed, with a migration note
- Docs updated in the same commit

## Opening a PR

Use the pull request template — it mirrors the checklist above. For rule PRs specifically,
fill in the rule-triple section: which fixture is positive, which bundle the negative fixture
was drawn from, and confirmation that `expected.json` was blessed (once that command exists)
rather than hand-written.

If you're not sure a change is worth making yet, or you found a missed detection or a false
positive while using (or reading about) the tool, open an issue first — see the issue
templates under `.github/ISSUE_TEMPLATE/`. A missed detection in particular is the highest-
value thing you can report; it doesn't need a fix attached.
