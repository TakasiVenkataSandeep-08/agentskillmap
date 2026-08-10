# The manifest — design notes

Build this before any parser. It is the CLI output, the CI diff format, the policy
allowlist target, and the eval ground-truth format simultaneously. Getting it wrong means
rewriting everything downstream.

Machine-readable version: `schema/manifest-v1.schema.json`.

## Shape

```jsonc
{
  "schema_version": "1.0.0",
  "tool": { "name": "skillaudit", "version": "0.1.0" },

  "target": {
    "kind": "skill",
    "name": "example-skill",
    "resolver": "claude-code",
    "root": "skills/example-skill",
    "content_digest": "sha256:…"        // merkle root over sorted inventory
  },

  "inventory": [
    {
      "path": "SKILL.md",
      "size": 1834,
      "sha256": "…",
      "load_phase": "on_trigger",        // always | on_trigger | reference | unreferenced
      "parsed_as": "markdown",
      "parse_status": "ok"               // ok | error | unsupported
    }
  ],

  "disclosure": {
    "description_bytes": 412,
    "declared_capabilities": [],         // from frontmatter, if the agent supports it
    "trigger_terms": ["…"],              // extracted, not scored
    "reference_files": 4,
    "unreferenced_files": 1
  },

  "capabilities": [                      // tier: proven — code plane only
    {
      "capability": "fs.read.credential",
      "reachability": "observed",        // observed | present | unresolved
      "detail": { "paths": ["~/.aws/credentials"] },
      "evidence": [
        {
          "file": "scripts/collect.py",
          "start_byte": 412, "end_byte": 448, "start_line": 17,
          "rule_id": "py.credential-read.dotfile",
          "snippet_sha256": "…"
        }
      ]
    }
  ],

  "instructions": [                      // tier: pattern — weak, lexical, prose
    {
      "signal": "instruction.fetch_as_instruction",
      "evidence": [ { "file": "reference/setup.md", "start_byte": 88, "end_byte": 140,
                      "start_line": 6, "rule_id": "instr.fetch-as-instruction",
                      "snippet_sha256": "…" } ]
    }
  ],

  "unresolved": [
    { "reason": "dynamic_dispatch", "file": "scripts/run.sh",
      "start_byte": 90, "end_byte": 118, "start_line": 4,
      "note": "exec target is a shell variable" }
  ],

  "advisory": {                          // tier: advisory — flag-gated, quarantined
    "enabled": true,
    "model": "…",
    "prompt_sha256": "…",
    "findings": [
      { "kind": "disclosure_delta",
        "claim": "reference/setup.md instructs credential upload; description mentions only formatting",
        "evidence": [ { "file": "reference/setup.md", "start_line": 12 } ] }
    ]
  },

  "diagnostics": [
    { "code": "unsupported_language", "file": "bin/helper.rb" }
  ]
}
```

## Decisions worth defending

**Three sibling arrays, not one array with a `tier` field.** A consumer that trusts only
static analysis drops `instructions` and `advisory` and is done. If tiers were a field,
every consumer would have to filter correctly, and eventually one wouldn't. Invariant 5 is
enforced by shape.

**`advisory.enabled: false` is present, not omitted,** when the semantic pass didn't run.
"Not checked" and "checked, found nothing" must be distinguishable in a diff.

**`unresolved` is top-level, not nested per capability.** It describes gaps in the analysis,
not properties of findings. Nesting it would make it invisible when the capability list is
empty — precisely the case where it matters most.

**No `count` or `total` fields anywhere.** They're derivable, and they create diff churn
that swamps the signal when one finding is added.

**`content_digest` is a merkle root over the sorted inventory**, so it is stable under
directory-walk order and file-mtime changes. It is the identity used by the lockfile and
by corpus deduplication.

**Reachability is per-capability, not per-evidence.** If any evidence for a capability is
`observed`, the capability is `observed`. A capability whose evidence is entirely `present`
stays `present`. Do not silently upgrade.

## Canonical serialization

Non-negotiable, per invariant 2:

- Keys sorted lexicographically at every level.
- Arrays sorted by a declared total order: `inventory` by `path`; `capabilities` by
  `(capability, first evidence file, start_byte)`; `evidence` by `(file, start_byte)`;
  `unresolved` by `(file, start_byte, reason)`; `diagnostics` by `(code, file)`.
- Two-space indent, LF, trailing newline, UTF-8, no BOM.
- Paths relative to bundle root, forward slashes.
- Floats: none. If you think you need one, you're building a score. See invariant 1.

Write the canonicalizer as one function in `skillaudit-core` and make it the only path to
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
| `fs.read.credential` | Reads a known credential path or secret-bearing env var |
| `fs.read.outside_bundle` | Reads outside the bundle root and project |
| `fs.write.outside_bundle` | Writes outside the bundle root |
| `fs.write.agent_config` | Writes `CLAUDE.md`, `settings.json`, hook or statusline config |
| `agent.hook.install` | Registers a hook that runs outside the skill's own trigger |
| `code.dynamic_eval` | `eval`, `exec`, `Function`, `source` of computed content |
| `code.obfuscation` | Encoding/decoding chain feeding a sink |
| `env.read.secret` | Reads env vars matching the secret-name set |
| `mcp.tool_reference` | References MCP servers or tools |

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

## `skillaudit.lock`

Per-project lockfile: for each installed bundle, `{ resolver, root, content_digest,
capabilities: [term…], schema_version }`. Deliberately *not* the full manifest — it is
reviewed by humans in PRs, so it holds the capability set and the digest only. The full
manifest is regenerated on demand.

The CI check is: recompute, compare to lock, fail on capability escalation, print the delta.
That check — *"this skill gained credential access in the update you're about to merge"* —
is the product. Everything above exists to make that line trustworthy.
