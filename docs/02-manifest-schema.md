# The manifest — design notes

Build this before any parser. It is the CLI output, the CI diff format, the policy
allowlist target, and the eval ground-truth format simultaneously. Getting it wrong means
rewriting everything downstream.

Machine-readable version: `schema/manifest-v1.schema.json`. The example below is validated
against that schema in CI — if the two ever disagree, the build fails.

## Shape

```json
{
  "schema_version": "1.3.0",
  "tool": { "name": "skillmap", "version": "0.7.1" },

  "target": {
    "kind": "skill",
    "name": "example-skill",
    "resolver": "claude-code",
    "root": "example-skill",
    "content_digest": "sha256:db0823d8a7fb5fe544398af6db72238d80e28b80f5715eb6f560e89dd1d8585c"
  },

  "inventory": [
    {
      "path": "SKILL.md",
      "size": 1834,
      "sha256": "sha256:0e95f5a3666031d6fac42bc6ca698b40b61648f4974c8ec355181ed4888c5c26",
      "load_phase": "on_trigger",
      "parsed_as": "markdown",
      "parse_status": "ok"
    },
    {
      "path": "scripts/collect.py",
      "size": 902,
      "sha256": "sha256:aa9ecfd639fb52aeb69483ac7654fc4a6662d22f698d6a096f7c9f818914ec99",
      "load_phase": "reference",
      "parsed_as": "python",
      "parse_status": "ok"
    }
  ],

  "disclosure": {
    "description_bytes": 412,
    "declared_capabilities": [],
    "trigger_terms": ["aws", "credentials", "format"],
    "reference_files": 4,
    "unreferenced_files": 1
  },

  "capabilities": [
    {
      "capability": "fs.read.credential",
      "reachability": "observed",
      "detail": { "paths": ["~/.aws/credentials"] },
      "evidence": [
        {
          "file": "scripts/collect.py",
          "start_byte": 412,
          "end_byte": 448,
          "start_line": 17,
          "rule_id": "py.credential-read.dotfile",
          "snippet_sha256": "sha256:bf69c1d222a7bedcb6a18bd2ada770bc79b87d5a82f61f0be14e02ea981b6302"
        }
      ]
    }
  ],

  "instructions": [
    {
      "signal": "instruction.fetch_as_instruction",
      "evidence": [
        {
          "file": "reference/setup.md",
          "start_byte": 88,
          "end_byte": 140,
          "start_line": 6,
          "rule_id": "instr.fetch-as-instruction",
          "snippet_sha256": "sha256:4b652647dbbcea8c050dc169fb386d64efbe122e0777ba13f228c364b049689c"
        }
      ]
    }
  ],

  "unresolved": [
    {
      "reason": "dynamic_dispatch",
      "file": "scripts/run.sh",
      "start_byte": 90,
      "end_byte": 118,
      "start_line": 4,
      "note": "exec target is a shell variable"
    }
  ],

  "advisory": {
    "enabled": true,
    "model": "claude-sonnet-5",
    "prompt_sha256": "sha256:22103d3836026a9e38910b068af4615458e410ba89d41bcc0d97fc737b9b85b3",
    "findings": [
      {
        "kind": "disclosure_delta",
        "claim": "reference/setup.md instructs credential upload; description mentions only formatting",
        "evidence": [{ "file": "reference/setup.md", "start_line": 12 }]
      }
    ]
  },

  "diagnostics": [
    { "code": "rule_load_error", "file": "rules/ruby/exec.toml", "note": "query references capture @target, not declared in [captures]" }
  ]
}
```

## Decisions worth defending

**Three sibling arrays, not one array with a `tier` field.** A consumer that trusts only
static analysis drops `instructions` and `advisory` and is done. If tiers were a field,
every consumer would have to filter correctly, and eventually one wouldn't. Invariant 5 is
enforced by shape.

**`advisory.enabled: false` is present, not omitted,** when the semantic pass didn't run.
"Not checked" and "checked, found nothing" must be distinguishable in a diff. The schema
enforces both halves: `enabled: false` requires an empty `findings` and forbids `model` and
`prompt_sha256`; `enabled: true` requires both, because an unpinned advisory branch is not
reproducible and turns every CI diff into noise (invariant 6).

**`unresolved` is top-level, not nested per capability.** It describes gaps in the analysis,
not properties of findings. Nesting it would make it invisible when the capability list is
empty — precisely the case where it matters most.

**`unresolved` is about the bundle; `diagnostics` is about the run.** Anything the analysis
could not cover *in the skill being scanned* — an unsupported language, a computed target, a
file too large, a symlink leaving the root — is an `unresolved` entry, because that is a fact
a reviewer needs about this bundle. Anything wrong with *the tool's own execution* — a rule
file that would not load, a semantic response that failed schema validation, an unreadable
`policy.toml` — is a `diagnostic`. Both vocabularies are closed enums, so the boundary cannot
blur by accretion. When in doubt: would this entry still be true if a different tool scanned
the same bundle? If yes, it is `unresolved`.

**Evidence completeness is tier-dependent, and the schema enforces it.** `capabilities` and
`instructions` carry `evidenceStrict`: file, byte span, line, `rule_id`, and
`snippet_sha256`, all required. A rule fired, so all five exist, and a finding that cannot be
pointed at cannot be regression-tested. `advisory` carries `evidenceAdvisory`: file and line
only, and the type structurally *cannot* hold a `rule_id` or a snippet hash. No rule fired
there, and a byte span back-derived from a model's prose citation is manufactured precision —
worse than an honest line number, because it looks checkable and isn't.

**`detail` is a closed object.** `paths` and `hosts`, both sorted and deduplicated, nothing
else. It sits inside an artifact required to be byte-identical, so every key needs a declared
type and a declared order. Adding a key is a schema-version event, same rule as adding a
taxonomy term.

**`declared_capabilities` holds raw strings, not taxonomy terms.** It is read from
third-party frontmatter written by authors who have never seen our vocabulary, so
constraining it to `capabilityTerm` would hard-fail on the first real bundle. We record
verbatim what the author declared. Mapping those strings into our taxonomy is a separate,
explicitly lossy step, and it never happens silently.

**No `count` or `total` fields anywhere.** They're derivable, and they create diff churn
that swamps the signal when one finding is added.

**Reachability is per-capability, not per-evidence.** If any evidence for a capability is
`observed`, the capability is `observed`. A capability whose evidence is entirely `present`
stays `present`. Do not silently upgrade.

## Identity: `content_digest` and `target.root`

Both are machine-independent by construction, or invariant 2 is false.

**`content_digest`** is a merkle root over the sorted inventory, covering **file bytes only**:

```
leaf_i = sha256( path_i_utf8 || 0x00 || raw_sha256_bytes_i )     # 32 raw bytes, not hex
root   = sha256( leaf_0 || leaf_1 || … || leaf_n )               # leaves in sorted path order
```

- Paths are forward-slash and relative to the bundle root, sorted **byte-wise over UTF-8**.
- Text files are LF-normalized before their `sha256` is computed, so a CRLF checkout on
  Windows cannot change the digest.
- **`load_phase` and `parse_status` are excluded.** The digest means *"these bytes"*, nothing
  more. Including classification would mean every improvement to the load-phase classifier
  invalidates every `skillmap.lock` in every repo that uses the tool — churn with no
  corresponding change in what the skill can do.

**`inventory[].size`** is the number of bytes that were hashed, not the number `stat`
reports. For text those differ: a CRLF checkout of the same commit has more bytes on disk
than an LF one, and reporting the on-disk figure would make the same bundle produce two
different manifests on two platforms even though `sha256` matched on both. Reporting a size
that does not describe the hashed bytes would also just be confusing.

**`target.root`** is a forward-slash path relative to the **resolver's discovery root** — for
`claude-code`, the path under `.claude/skills/`, so `example-skill`, not
`/home/ana/work/proj/.claude/skills/example-skill` and not `../../.claude/skills/example-skill`.
Anchoring to cwd or the project directory would leak machine layout into the manifest and
break byte-identity between two developers with different checkout paths.

## Canonical serialization

Non-negotiable, per invariant 2:

- Keys sorted lexicographically at every level.
- **All sorting is byte-wise over UTF-8**, never locale collation. Locale-sensitive
  comparison makes "byte-identical on any machine" false on the first non-ASCII path, and it
  fails on the machine that has a different `LANG` rather than in CI.
- Arrays sorted by a declared total order — every array, no exceptions:

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

  Optional sort keys need an explicit rule for absence or the order is partial, and a partial
  order is a nondeterminism bug that only shows up on the one input that has a tie.

- **Ties are broken by the element's own canonical JSON rendering.** The keys above are the
  declared order; they are not by themselves total. Two capabilities can share a term and a
  first-evidence position and still differ in `reachability`, and two `unresolved` entries can
  share `(file, reason, start_byte)` and differ in `note`. Left there, their relative order
  would be whatever order the analysis happened to emit them in — a different filesystem walk
  or a different rule evaluation order would produce different bytes. Comparing the rendered
  elements makes the order total; elements whose renderings are also equal are byte-for-byte
  identical, so their relative order is unobservable in the output.

  This is deliberately *not* implemented by deriving a structural ordering over the Rust
  types. That would make the artifact's bytes depend on the declaration order of struct
  fields and enum variants, so a cosmetic reordering in a later PR would silently change
  every manifest in every repo that uses the tool.

- Two-space indent, LF, trailing newline, UTF-8, no BOM.
- Paths relative to bundle root, forward slashes.
- Floats: none. If you think you need one, you're building a score. See invariant 1.

Write the canonicalizer as one function in `skillmap-core` and make it the only path to
serialization. Do not let `serde_json::to_string_pretty` escape into the codebase.

## Capability taxonomy (v1, closed vocabulary)

Adding a term is a schema-version event. Every term must have at least one rule with
fixtures, or it does not exist (invariant 12).

| Term | Meaning |
|---|---|
| `process.exec` | Spawns a subprocess with a statically-known target |
| `process.exec.dynamic` | Spawns a subprocess with a computed target |
| `net.egress` | Outbound network; `detail.hosts` when statically resolvable |
| `net.fetch_then_execute` | Fetched content reaches an exec or eval sink |
| `fs.read.credential` | Reads a **file** at a path conventionally holding credentials |
| `fs.read.outside_bundle` | Reads outside the bundle root and project |
| `fs.write.outside_bundle` | Writes outside the bundle root |
| `fs.write.agent_config` | Writes `CLAUDE.md`, `settings.json`, hook or statusline config |
| `code.dynamic_eval` | `eval`, `exec`, `Function`, `source` of computed content |
| `code.obfuscation` | Encoding/decoding chain feeding a sink |
| `env.read.secret` | Reads an **environment variable** whose name matches the secret-name set |

Instruction-plane signals use a separate `instruction.*` namespace and never appear in
`capabilities`:

| Signal | Meaning |
|---|---|
| `instruction.fetch_as_instruction` | Prose telling the agent to treat fetched content as instructions |
| `instruction.exfil` | Prose directing data to an external destination |
| `instruction.config_mutation` | Prose directing edits to agent config |
| `instruction.silence` | Prose directing the agent not to report or surface something |
| `instruction.privilege_claim` | Prose asserting pre-authorization or elevated permission |

`instruction.silence` and `instruction.privilege_claim` are the two that will earn this
project attention. They are also the two most likely to false-positive on legitimate skills
about logging verbosity and permissions — negative fixtures for those are load-bearing.

## Migration: 1.2.0 → 1.3.0

One change: **`instruction.directs_outside_write` is added to `instructionSignal`.**
Nothing about the manifest's *shape* moved.

**What it means.** Prose directing the agent to run a command that writes to, copies into,
or makes executable a path outside the bundle — a shell profile, a config directory, a bin
directory on `PATH`, the agent's own skills directory. `tier = "pattern"`, in `instructions`,
never in `capabilities`: the bundles it targets ship no code at all, so the claim is about
what the prose tells the agent to do.

**Why this shape.** 89.8% of harvested bundles ship no parseable file, and a third of those
carry runnable code in fenced blocks that nothing looked at. Three earlier candidates —
directing egress, credential access, subprocess spawning — were defined, drawn for, and
withdrawn before a label was written: in a prose-only bundle the dominant genre is reference
material, so a network call in a code sample is documentation, and at 23–26% base rates with
no contextual separator they were noise generators. Requiring an operative heading was
measured as a rescue and failed at 30% of the *control* stratum.

What survives carries its own intent. Reference material demonstrates logic and never
mutates the reader's machine as an illustration.

**Measured** against 80 prose-only bundles labelled before the rule existed: **precision
37/38 (97.4%), recall 37/37 (100%)**. The single false positive is a copy annotated `WRONG`
in a section showing a common mistake — prose about the shape, matched as the shape, which a
`pattern`-tier rule cannot tell apart and which is why these findings are quarantined.

**Compatibility.** Same direction as 1.2.0: adding a variant to a closed vocabulary means a
1.2.0 consumer rejects a manifest carrying this signal, and the breakage is real rather than
hypothetical because a rule ships that emits it. Lock files store wire names, so this
degrades to a visible error rather than a silent rewrite. Minor because the shape is
unchanged and every existing term keeps its meaning.

## Migration: 1.1.0 → 1.2.0

One change: **`instruction.exec_directive` is added to `instructionSignal`.** Nothing
about the manifest's *shape* moved — no field added, removed, or renamed, no array
gained a sort key, and the canonical serialization is unchanged.

**What it means.** Prose directing the agent to run a command that fetches remote
content and executes it — `curl … | sh`, or fetching a `.sh`/`.py` and then running it.
It is `tier = "pattern"` and lands in `instructions`, never in `capabilities`: the claim
is about what the prose tells the agent to do, not about what the bundle's own code does.

**Why the definition is this narrow.** A first draft read "directs execution of a command
it supplies", which is satisfied by `python scripts/build.py` in any usage section — a
signal that fires on nearly every skill describes nothing. The property worth reporting is
that the executed code **is not in the bundle and is not reviewable**: it arrives from a
URL at run time, so reading the bundle cannot tell you what runs.

**It is not a verdict, and the corpus is emphatic about that.** Of 40 bundles drawn for
this shape, 34 carry it, and nearly all are ordinary installer instructions for real
tools — container runtimes, language toolchains, vendor CLIs. The shape also spans a care
gradient no pattern can see: most pipe straight into a shell, three fetch to a file and
page it for review first, one verifies a SHA-256 before executing, one runs the fetched
script under `sudo`. All of them execute remote code, so all carry the term. Which of
those a repository tolerates is `policy.toml`'s question. Invariant 1 applies here more
sharply than anywhere else in the taxonomy.

**Compatibility.** Adding a variant to a closed vocabulary is breaking in the same
direction the 1.1.0 removal was: a 1.1.0 consumer rejects a manifest carrying a signal it
does not recognise. Unlike that removal, the breakage here is **real rather than
hypothetical** — a rule ships with this term and will emit it, on roughly a third of
bundles that carry a shell fence. A `skillmap.lock` written by a 1.2.0 binary can name
`instruction.exec_directive`; an older binary reading that lock will not recognise it.
Lock files store capability wire names precisely so this degrades to a visible error
rather than a silent rewrite. Minor rather than major because the shape is unchanged and
every existing term keeps its meaning.

## Migration: 1.0.0 → 1.1.0

Three changes, all to the capability taxonomy. Nothing about the manifest's *shape*
moved: no field was added, removed, or renamed, and the canonical serialization is
unchanged.

**Two terms were removed.** `agent.hook.install` and `mcp.tool_reference`.

Invariant 12 forbids a term no rule detects, so v1.0 had to either grow rules to cover
the taxonomy or shrink the taxonomy to match the rules — decided from the corpus rather
than from ambition. Nine terms grew rules. These two could not:

- `agent.hook.install` appears in 3 of 92 labelled bundles, and its real form is a JSON
  edit rather than a code sink. Its practical instances are covered by
  `fs.write.agent_config`, so this is a term merge rather than a capability lost.
- `mcp.tool_reference` lives in `.mcp.json`, and **no JSON grammar is registered**.
  Adding one would make every `.json` file in every bundle stop reporting
  `unsupported_language`, moving the published unresolved rate for all 92 labelled
  bundles for reasons unrelated to detection quality. That is a large, separately
  measurable change, and it must not ride along inside a taxonomy commit.

**Compatibility.** Removing a variant from a closed vocabulary is breaking in principle:
there is no `Unknown` term, so a 1.0.0 consumer rejects a manifest it does not
recognise, and a 1.1.0 consumer would reject one of these terms if it met it. In
practice the breakage is provably hypothetical — **no manifest has ever contained
either term, because no rule has ever emitted one.** That is why this is a minor bump
and not a major one, and the reasoning is recorded here rather than assumed.

**One term narrowed without changing its wire form.** `fs.read.credential` was
documented as "reads a known credential path **or secret-bearing env var**", which is
exactly what `env.read.secret` describes. Both terms claimed the same act, so which one
a manifest carried would have depended on which rule happened to fire. It now covers
**files only**. No enum variant changed and the JSON Schema is byte-identical, so a
1.0.0 manifest still validates — but the *meaning* is narrower, which matters when
reading an older manifest that was produced before either rule existed.

## Run-scoped diagnostic codes (closed)

| Code | Means |
|---|---|
| `rule_load_error` | A rule file could not be read or parsed |
| `rule_validation_error` | A rule loaded but failed validation (undeclared capture, unknown capability term, missing fixture) |
| `semantic_schema_violation` | The semantic pass returned output that failed schema validation; the finding was discarded |
| `semantic_call_failed` | The semantic model call did not complete |
| `policy_load_error` | `policy.toml` could not be read or parsed |

## `skillmap.lock`

Per-project lockfile: for each installed bundle, `{ resolver, root, content_digest,
capabilities: [term…], schema_version }`. Deliberately *not* the full manifest — it is
reviewed by humans in PRs, so it holds the capability set and the digest only. The full
manifest is regenerated on demand.

The CI check is: recompute, compare to lock, fail on capability escalation, print the delta.
That check — *"this skill gained credential access in the update you're about to merge"* —
is the product. Everything above exists to make that line trustworthy.

**Full format, `policy.toml`, and the exit codes: [`06-policy-and-lock.md`](06-policy-and-lock.md).**
