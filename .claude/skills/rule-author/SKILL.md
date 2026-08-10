---
name: rule-author
description: Use when authoring, adding, or extending a skillmap detection rule — writing tree-sitter .scm queries, rule TOML metadata, or fixtures under rules/, queries/, fixtures/. Triggers on requests like "add a rule for X", "write a tree-sitter query", "extend capability coverage for language Y", "add a sink detector", or anything touching capture roles or the rule triple.
---

# Authoring a rule triple

A rule is three files, never fewer, per `docs/03-rules-authoring.md`:

```
rules/<language>/<rule-id>.toml        # metadata
queries/<language>/<rule-id>.scm       # tree-sitter query
fixtures/<language>/<rule-id>/
    positive.<ext>                     # must fire
    negative.<ext>                     # must NOT fire
    expected.json                      # canonical manifest fragment
```

Start from the reference triple and copy its shape:
`rules/python/credential-read.toml`, `queries/python/credential-read.scm`,
`fixtures/python/credential-read/{positive.py,negative.py,expected.json}`.

Everything below is a way authors get one of those three files subtly wrong. Each item
states the failure it prevents, not just the rule.

## The capability term is closed vocabulary

`capability` in the TOML must already exist in `docs/02-manifest-schema.md`'s taxonomy
table (`fs.read.credential`, `process.exec`, `net.egress`, …). If the capability you're
detecting isn't in that table, you are not writing a rule — you are proposing a
schema-version event. Stop, don't invent a string that merely looks like it fits; a new
taxonomy term needs the schema bumped and `docs/02-manifest-schema.md` updated first, in
its own change.

## Structural matching, not string matching

`(call function: (attribute ...))` beats matching the text `open(`. A query that matches
source text is defeated by whitespace, aliasing (`import os as o`), or trivial
reformatting, and it is exactly the kind of check an obfuscated payload is built to slip
past. Tree-sitter gives you the AST — use it. Look at
`queries/python/credential-read.scm`: it matches the *call shape* (`function:` +
`arguments:` with a predicate on the callee identifier), never the literal `"open("`.

## Capture the smallest span, not the whole function

Evidence spans are what a human reads in a PR diff. `@site` should point at the call
expression that is the sink, not the function it lives in. A whole-function capture forces
every reviewer to re-derive which line actually matters — it technically satisfies
"provenance" while defeating the reason provenance exists. If you're tempted to capture a
block to "be safe," capture the call/expression inside it instead.

## Path lists live in `[match]`, never in the query

Do not write `(#match? @path "\\.aws|\\.ssh|\\.netrc")` inside the `.scm` file. Literal
prefixes belong in the TOML's `[match].path_prefixes`, exactly like
`rules/python/credential-read.toml` does it. This is invariant 7 made concrete: a
contributor extending coverage for a new credential path should be able to add one string
to a TOML array and never open a `.scm` file, let alone Rust. If extending your rule's
coverage requires touching the query, the query encoded something that should have been
data.

## `[captures]` maps roles, and validation runs both directions

The engine has exactly four roles — see the table in `docs/03-rules-authoring.md`:

| Role | Required? | Effect |
|---|---|---|
| `site` | required | byte span reported as the evidence span |
| `path` | optional | filtered through `path_prefixes`, lands in `detail.paths` |
| `host` | optional | filtered through `host_suffixes`, lands in `detail.hosts` |
| `dynamic` | optional | target unresolved → `unresolved: computed_target`, not a capability |

Two rules, both enforced by `rules validate` (not yet built — see below):

- Any capture prefixed `@_` is query-local (drives `#eq?`/`#match?` predicates) and must
  **not** appear in `[captures]`.
- Every other capture the query produces **must** appear in `[captures]`. Validation checks
  TOML→query and query→TOML. A capture your query emits that the TOML never declares isn't
  a warning — it's a rule silently dropping a detection on the floor while looking like it
  works, which is worse than not having the rule.

## Dynamic targets are captured, never skipped

When the query hits a call whose target can't be resolved statically (a variable, a
concatenation, a function result), do not filter it out and move on. Capture it with the
`dynamic` role, same as the third pattern in `queries/python/credential-read.scm` does for
`open(computed_expr)`. The engine turns that into an `unresolved` entry with reason
`computed_target`. Skipping it is silence, and invariant 3 makes silence a bug, not a
missing feature — a scanner that says nothing because it understood nothing must be
indistinguishable-in-symptom-but-not-in-record from one that found nothing because there
was nothing there.

## Both fixtures are mandatory, and the negative is not invented

`positive.<ext>` must trigger the rule; `negative.<ext>` must not, and per invariant 8 and
the review checklist in `docs/03-rules-authoring.md`, the negative fixture should be drawn
from a real bundle's code shape (e.g. the credential-read reference fixture's negative is a
docstring that *mentions* `~/.aws/credentials` as an example string — a real and common
pattern — rather than a synthetic near-miss). A rule with no negative fixture is an
untested false-positive generator; a rule whose negative fixture is contrived to be easy is
close to the same thing.

`expected.json` records the manifest fragment each fixture should produce. Byte offsets are
filled in by `skillmap rules bless` once it exists — don't hand-compute them; leave the
fields that require a running engine out, matching the shape in
`fixtures/python/credential-read/expected.json`.

## What does not exist yet

`skillmap rules validate` and `skillmap rules bless` are specified in
`docs/03-rules-authoring.md` and referenced throughout, but neither command exists —
there is no `skillmap-rules` crate yet (it lands in task T4, per `docs/00-tasks.md`).
Write the triple to the shape those commands will eventually check by hand: query compiles
conceptually against the grammar, captures line up with `[captures]` in both directions,
fixtures match `expected.json`. Say so plainly if asked to "run validation" — do not
fabricate a passing check.
