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

`skillmap rules validate` checks that all three exist, that the query compiles against the
grammar, that captures and roles line up **in both directions** (see below), and that both
fixtures produce their expected outcome. It will run in CI once the engine exists (task T4);
neither `rules validate` nor `rules bless` is implemented yet. A rule missing a negative fixture is
rejected — that is an untested false-positive generator (invariant 8).

## Capture roles — the engine's whole vocabulary

The engine knows **roles**, never sink names. This is the mechanism that makes invariant 7
enforceable rather than aspirational: a new sink, a new language, a new obfuscation trick all
reduce to the same four roles, so none of them is a reason to touch a `.rs` file.

| Role | What the engine does with it |
|---|---|
| `site` | **Required.** Byte span reported as the evidence span. |
| `path` | Optional. Literal value filtered through `[match].path_prefixes`; survivors land in `detail.paths`. |
| `host` | Optional. Literal value filtered through `[match].host_suffixes`; survivors land in `detail.hosts`. |
| `dynamic` | Optional. The target could not be resolved statically. Emits an `unresolved` entry with reason `computed_target` **instead of** a capability — never a silent skip (invariant 3). |

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

# Literal path prefixes that make this a credential read. Data, not code.
[match]
path_prefixes = [
  "~/.aws/", "~/.ssh/", "~/.config/gh/", "~/.kube/config",
  ".env", ".netrc", "~/.docker/config.json"
]

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

Two of them — `instruction.silence` and `instruction.privilege_claim` — are the signals most
likely to earn this project attention and most likely to false-positive on legitimate skills
about logging verbosity and permission handling. Their negative fixtures are load-bearing.
Write three negatives each, drawn from real bundles in the corpus, before writing the query.

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
