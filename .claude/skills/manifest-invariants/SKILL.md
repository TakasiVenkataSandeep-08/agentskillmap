---
name: manifest-invariants
description: Use when editing the manifest schema (schema/manifest-v1.schema.json), docs/02-manifest-schema.md, or the canonical serialization / canonicalize() path in skillmap-core. Triggers on adding or changing a manifest field, changing an array's sort order, adding a capability taxonomy term, touching evidence or tier types, or anything that changes what the manifest looks like on disk.
---

# Manifest invariants

The manifest is `schema/manifest-v1.schema.json` plus the canonicalization rules in
`docs/02-manifest-schema.md`. An edit that looks locally reasonable — a new optional field,
a plausible-looking sort — routinely breaks byte-identical determinism (invariant 2) or
tier separation (invariant 5) without looking like it does. This skill exists because those
breaks don't show up in a normal review pass; they show up as CI flakiness on the one input
that has a tie, weeks later.

## Every array needs a DECLARED TOTAL ORDER

Reproduced from `docs/02-manifest-schema.md` — this table is the actual spec, not a
paraphrase of it:

| Array | Order |
|---|---|
| `inventory` | `path` |
| `capabilities` | `(capability, first evidence file, first evidence start_byte)` |
| `instructions` | `(signal, first evidence file, first evidence start_byte)` |
| `evidence` (strict) | `(file, start_byte)` |
| `evidence` (advisory) | `(file, start_line)` |
| `unresolved` | `(file, reason, start_byte)` — absent `start_byte` sorts **before** any present one |
| `advisory.findings` | `(kind, first evidence file, first evidence start_line, claim)` |
| `diagnostics` | `(code, file)` — absent `file` sorts **before** any present one |
| `disclosure.trigger_terms` | lexicographic, deduplicated |
| `disclosure.declared_capabilities` | lexicographic, deduplicated |
| `detail.paths`, `detail.hosts` | lexicographic, deduplicated |

If you add a new array to the manifest, it needs a row in this table (and the matching row
in `docs/02-manifest-schema.md`) before it needs a Rust type. If a sort key is optional
(like `start_byte` in `unresolved`), the absence case needs an explicit rule stated right
there — "optional sort keys need an explicit rule for absence or the order is partial, and
a partial order is a nondeterminism bug that only shows up on the one input that has a tie"
is the exact failure mode this guards against. Silently defaulting an absent key to `0`, or
to "sorts last," is a rule — but an implicit one nobody wrote down, which means the next
person to touch the sort has no way to know they broke it.

**The keys in that table are not by themselves total, and the tiebreak is part of the spec.**
Two elements can agree on every declared key and still differ — two capabilities sharing a
term and a first-evidence position but differing in `reachability`, two `unresolved` entries
sharing `(file, reason, start_byte)` but differing in `note`. `canonicalize()` breaks those
ties on the element's own canonical JSON rendering. Don't replace that with a derived
structural ordering over the Rust types (`#[derive(Ord)]` and friends): that makes the
artifact's bytes depend on the declaration order of struct fields and enum variants, so
reordering a variant for readability silently changes every manifest in every repo.

## Byte-wise sorting, never locale collation

"All sorting is byte-wise over UTF-8, never locale collation" (invariant 2). Using a
locale-aware string comparator (the default in a lot of standard-library sort-by-string
helpers) makes byte-identity false on the first non-ASCII path, and it fails on whichever
machine has a different `LANG` set — not in code review, not in the author's own CI run,
but on someone else's machine or a future glibc update. If you're adding a sort, check what
comparator it actually calls.

## Three tiers, never blended

`capabilities` (tier `proven`), `instructions` (tier `pattern`), and `advisory` are
**separate top-level arrays/objects**, not one array with a `tier` field. This is
invariant 5 enforced by shape rather than by discipline — a consumer that only trusts
static analysis drops two keys and is done, instead of having to filter correctly and
eventually filtering wrong. If a change would make it easier to add a `tier` discriminator
and merge these into one collection, that change is regressing the design on purpose,
not simplifying it.

Two consequences that are easy to violate in a change that looks like a cleanup:

- A `pattern` finding never gets promoted into `capabilities` under any condition,
  including "we're now very confident about it."
- An `advisory` finding can add, remove, or modify nothing in `capabilities`,
  `instructions`, or `unresolved`. It is read-only with respect to the deterministic
  branches, by construction — `skillmap-semantic` doesn't even depend on the crates that
  produce them (see `ARCHITECTURE.md`). A PR that has the semantic pass adjust a
  `proven`/`pattern` finding, even to fix an apparent false positive, breaks quarantine.

## Evidence completeness is tier-dependent, and it's a schema fact, not a convention

- `evidenceStrict` (used by `capabilities` and `instructions`): `file`, `start_byte`,
  `end_byte`, `start_line`, `rule_id`, `snippet_sha256` — **all required**. A rule fired,
  so all five exist; there is no legitimate reason for one to be missing.
- `evidenceAdvisory` (used by `advisory.findings`): `file`, `start_line` only — and the
  type **structurally cannot hold** `rule_id` or `snippet_sha256`. No rule fired, so there's
  no `rule_id`, and a byte span back-derived from a model's prose citation is manufactured
  precision — it looks checkable and isn't.

Don't add fields to `evidenceAdvisory` to make it "more complete." The incompleteness is
the honest state, and the schema enforces the boundary at the `$defs` level specifically so
the advisory tier can't accidentally claim deterministic provenance.

## No floats, counts, totals, scores, or grades — ever

Invariant 1: never a risk score, letter grade, traffic light, or the words "safe" /
"malicious" / "suspicious" in the manifest. Invariant 2 extends this concretely: no
`count` or `total` field anywhere, even one that looks purely descriptive (e.g.
`capabilities_count`) — they're derivable from the arrays, and they create diff churn that
swamps the signal when one finding is added. If you think a field needs to be a float,
you're building a score; see invariant 1 and stop.

## No volatile fields

No timestamps, hostnames, absolute paths, usernames, durations, or run IDs anywhere in the
manifest — those go to stderr or `run-meta.json`, never into the JSON that CI diffs. This
includes indirectly: a field that embeds a duration-derived ordering, or a path that hasn't
been normalized to forward-slash-relative-to-bundle-root, reintroduces machine-dependence
through the back door. `target.root` and `content_digest` in particular are
machine-independent by construction (see the merkle definition in
`docs/02-manifest-schema.md`) — don't add a field near them without checking it inherits
that property.

## Any shape change is a schema-version event

Adding a field, a capability taxonomy term, an `unresolved` reason code, a `diagnostics`
code, or an `instructionSignal` — all of it is a schema-version event with a migration
note, per the DoD checklist in `AGENTS.md`. "It's additive so it's backward compatible" is
not the bar here; the bar is that `schema_version` changes and the note explains what an
existing `skillmap.lock` consumer needs to know.

## `canonicalize()` is the only serialization path

`skillmap-core` exposes one canonicalization function — sorted keys, the array orders
above, two-space indent, LF, trailing newline, UTF-8, no BOM — and it is the only place
`serde_json` gets called for output. `serde_json::to_string_pretty` (or any ad hoc
`serde_json::to_string`/`to_writer` on manifest types) must never escape into the codebase
outside that one function. If you're writing a manifest to a file or stdout from anywhere
else, that's the bug, not a shortcut.
