# Rule authoring

Rules are **data**. Adding detection coverage must never require touching a `.rs` file
(invariant 7). This is the mitigation for choosing Rust: the contributor pool for a Rust
security tool is an order of magnitude smaller than for Go, and rule coverage is what
determines the product's long-term quality. Someone who spots a missed obfuscation pattern
must be able to fix it with a query file and two fixtures.

If you ever find yourself adding a `match` arm for a specific sink name, stop — the
architecture has failed and the fix belongs in the engine, not the arm.

## A rule is a triple

```
rules/python/credential-read.toml        # metadata
queries/python/credential-read.scm       # tree-sitter query
fixtures/python/credential-read/
    positive.py                          # must fire
    negative.py                          # must NOT fire
    expected.json                        # canonical manifest fragment
```

Validation checks that all three exist, that the query compiles against the grammar, that
captures and roles line up **in both directions** (see below), and that both fixtures produce
their expected outcome. A rule missing a negative fixture is rejected — that is an untested
false-positive generator (invariant 8).

All of that runs today, in `skillmap-rules` and in `skillmap-code`'s fixture suite: every rule
under `rules/` is discovered and exercised against its own fixtures on every `cargo test`, so
adding a rule adds coverage automatically. What does not exist yet is the **CLI wrapper** —
`skillmap rules validate` and `skillmap rules bless` become subcommands at task T9, when there
is a binary to hang them on. Until then, blessing a fixture is
`SKILLMAP_BLESS=1 cargo test -p skillmap-code`.

## Capture roles — the engine's whole vocabulary

The engine knows **roles**, never sink names. This is the mechanism that makes invariant 7
enforceable rather than aspirational: a new sink, a new language, a new obfuscation trick all
reduce to the same four roles, so none of them is a reason to touch a `.rs` file.

| Role | What the engine does with it |
|---|---|
| `site` | **Required.** Byte span reported as the evidence span. |
| `path` | Optional. Literal value filtered through `[match].path_prefixes`; survivors land in `detail.paths`. |
| `host` | Optional. Literal value filtered through `[match].host_suffixes`; survivors land in `detail.hosts`. |
| `dynamic` | Optional. A computed target. The engine **first tries to fold it** to a path (see below); only if that fails does it emit `unresolved: computed_target` — never a silent skip (invariant 3). |

### `dynamic` does not mean "give up"

It used to. The T3 labelling pass then measured recall at 38.9% and found that **every
credential read in the corpus reaches its path by computation** — not one used a string
literal. So a `dynamic` capture is now folded first, and only an unfoldable one becomes
`unresolved`.

Folding handles literals, path joins (`a / b`, `os.path.join`, `path.join`), home-directory
lookups (`Path.home()`, `os.homedir()`, `expanduser("~")`), `Path(x)`, and identifiers bound
**exactly once** in the file. Anything else is unknown, and stays unknown. It is a folder, not
an interpreter: there is no control flow and no cross-file analysis.

Three outcomes, and a rule author only needs to know which pattern list catches which:

| Fold result | Example | Matched against |
|---|---|---|
| fully resolved | `Path.home() / ".aws" / "credentials"` | `path_prefixes`, `path_suffixes` **and** `path_contains` |
| tail only | `os.path.join(root, ".env")` | `path_suffixes` and `path_contains` |
| unknown | `open(compute())` | nothing; becomes `unresolved` |

A tail-only path knows *what the file is called* and not *where it is*, so asking a prefix
question of it would be asking about a location it does not have. It can still be asked what
directory holds it, but only about the part that folded: a `credentials/` directory sitting in
the unknown head stays invisible, because nothing established it is there.

Two naming rules, both enforced by `rules validate`:

- **Captures prefixed `@_` are query-local.** They exist to drive `#eq?` / `#match?`
  predicates and are invisible to the engine. Do not declare them.
- **Every other capture must appear in `[captures]` and map to a role above.** Validation
  runs TOML→query *and* query→TOML. A capture the query produces but the TOML never declares
  is a rule silently dropping information on the floor, which is how a detection quietly
  stops firing; it is a `rule_validation_error`, not a warning.

Adding a role is an engine change and a schema-version event. If a rule seems to need a fifth
role, that is a design discussion, not a patch.

## Metadata format

```toml
id          = "py.credential-read.dotfile"
language    = "python"
capability  = "fs.read.credential"     # must be in the closed taxonomy
tier        = "proven"                 # proven | pattern
query       = "queries/python/credential-read.scm"

# Captures the engine reads out of the query. Keys are roles from the table above;
# values are the capture names in the .scm. Every non-`@_` capture must appear here.
[captures]
site        = "@site"                  # byte span reported as evidence
path        = "@path"                  # optional; lands in detail.paths
dynamic     = "@dynamic"               # optional; becomes unresolved: computed_target

# Which paths make this a credential read. Data, not code.
[match]
# "is this the file at this location" — matched with starts_with.
path_prefixes = [
  "~/.aws/", "~/.ssh/", "~/.config/", "~/.kube/config",
  ".env", ".netrc", "~/.docker/config.json"
]
# "is this a file with this name, wherever it lives" — matched at a component
# boundary, so ".env" matches "a/b/.env" and never "production.env".
#
# This is what makes a partially folded path usable. Keep the list short and keep
# every entry a filename whose only conventional purpose is holding a secret:
# "config.json" here would fire on most of the benign stratum.
path_suffixes = [".env", ".netrc", "credentials.json", ".token"]
# "is this file inside a directory called this" — matched at component boundaries
# on both sides, so "credentials" matches "~/x/credentials/y" and never
# "~/my-credentials/y". Multi-component patterns bind as a unit.
#
# The question the other two cannot ask. Reach for it when the *directory* is the
# convention and the filename is not: ~/.clawdbot/credentials/homebridge.json is
# named per integration, so no filename list reaches it, and it sits under a home
# directory too broad for any prefix list to claim.
path_contains = ["credentials"]

[docs]
summary = "Reads a path conventionally holding credentials."
rationale = """
Legitimate for skills that manage cloud config. Reported, not judged — policy decides.
"""
false_positive_notes = """
Fires on skills that document credential paths in example strings. The negative fixture
covers the documentation case; extend it rather than narrowing the query.
"""
```

## Query conventions

- Capture the **smallest span that identifies the site**. Evidence spans are what humans
  read in a PR; a whole-function capture is useless to a reviewer.
- Prefer structural matching over string matching. `(call function: (attribute ...))` beats
  matching the text `open(`.
- Never encode a path list in the query. Paths go in `[match]` so a contributor can extend
  coverage without understanding tree-sitter.
- When the query cannot determine a target statically, do **not** skip it — the engine emits
  `unresolved` with reason `computed_target` (invariant 3). Design the query to capture the
  dynamic site too, and let the engine classify.

## Instruction-plane rules

Same triple, `tier = "pattern"`, language `markdown`. These are lexical and deliberately
weak; they land in `instructions`, never `capabilities`, and can never be promoted.

**Negative fixtures decide whether one of these is shippable, and T13 is the evidence.** It
labelled 156 bundles and withdrew `instruction.exfil` at 2/36 precision. Its false positives
were a security policy made entirely of prohibitions, a finance skill stating it *cannot* move
funds, a bundle disclosing that its examples may transmit prompt context, and six wallet
skills where `send` means a token transfer. An invented negative predicts none of that.

`instruction.silence` and `instruction.privilege_claim` were also removed — they had sat in
the vocabulary since T5 with no rule, and the pass that would have supplied their fixtures
read 156 bundles and found no candidate prose for either. `crates/skillmap-instr/tests/signals.rs`
now fails if any vocabulary term has no rule that can produce it.

## Adding a language

1. Add the tree-sitter grammar dependency.
2. Register the extension → grammar mapping in `rules/languages.toml`.
3. Port the sink rules; anything not ported leaves that language emitting `unresolved`
   with reason `unsupported_language` — which is correct and honest, not a gap.

The engine gains no language-specific code. If it needs some, that is a bug in the engine.

## Review checklist for a rule PR

- [ ] Negative fixture drawn from a real bundle, not invented
- [ ] Span is minimal and points at something a reviewer can act on
- [ ] Capability term already exists in the taxonomy (adding one is a schema-version event)
- [ ] `false_positive_notes` filled in honestly
- [ ] Eval metrics did not regress beyond tolerance
