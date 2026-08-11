# Architecture

Read `AGENTS.md` first. The invariants there constrain everything below.

## Data flow

```
                     ┌──────────────┐
  agent config  ───► │   resolve    │  discovery paths per agent → BundleRef[]
  or explicit path   └──────┬───────┘
                            ▼
                     ┌──────────────┐
                     │    parse     │  frontmatter, file inventory, hashing,
                     └──────┬───────┘  load-phase classification, ref graph
                            ▼
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
 ┌────────────┐      ┌────────────┐      ┌────────────┐
 │ code plane │      │ instr.plane│      │  semantic  │  (flag-gated)
 │  tier:     │      │  tier:     │      │  tier:     │
 │  proven    │      │  pattern   │      │  advisory  │
 └──────┬─────┘      └──────┬─────┘      └──────┬─────┘
        └───────────────────┼───────────────────┘
                            ▼
                     ┌──────────────┐
                     │   manifest   │  canonical JSON, versioned schema
                     └──────┬───────┘
                            ▼
              ┌─────────────┴─────────────┐
              ▼                           ▼
       ┌────────────┐              ┌────────────┐
       │   policy   │              │    diff    │  manifest[n-1] vs manifest[n]
       │  allow/deny│              │ capability │
       └────────────┘              │  delta     │
                                   └────────────┘
```

The three analysis planes run independently and cannot see each other's output. This is
what makes invariant 5 enforceable at the type level rather than by discipline: each plane
returns its own finding type, and only the manifest assembler sees all three.

## Workspace layout

```
skillmap/
├── Cargo.toml                  # workspace
├── AGENTS.md                   # invariants — read first (canonical)
├── CLAUDE.md                   # pointer to AGENTS.md + Claude Code tooling
├── ARCHITECTURE.md
├── CONTRIBUTING.md             # contributor workflow; adding a rule without Rust
├── SECURITY.md                 # threat model, network behaviour, disclosure policy
├── LICENSE
├── rust-toolchain.toml         # pinned exactly — reproducible builds (invariant 9)
├── deny.toml                   # cargo-deny: our own supply chain
├── rustfmt.toml
├── .gitattributes              # LF normalization — a CRLF fixture changes its SHA-256
├── .editorconfig
├── .claude/                    # skills, subagents, commands, hooks (Claude Code only)
├── scripts/                    # verify_spec.py and friends
├── crates/                     # target layout; members are added as their task begins
│   ├── skillmap-core/        # types, manifest, canonical ser, capability taxonomy  [T1, exists]
│   ├── skillmap-resolve/     # Resolver trait + per-agent discovery conventions  [T2, exists]
│   ├── skillmap-parse/       # bundle parse, frontmatter, inventory, reference graph  [T2, exists]
│   ├── skillmap-rules/       # rule loading, validation, tree-sitter query engine  [T4, exists]
│   ├── skillmap-code/        # code plane: sinks + reachability      → tier `proven`  [T4, exists]
│   ├── skillmap-instr/       # instruction plane: lexical patterns   → tier `pattern`  [T5, exists]
│   ├── skillmap-semantic/    # quarantined model pass                → tier `advisory`
│   ├── skillmap-scan/        # assembles one manifest from all three planes  [T8, exists]
│   ├── skillmap-policy/      # policy.toml, allowlists, exit codes  [T8, exists]
│   ├── skillmap-diff/        # skillmap.lock + capability escalation  [T8, exists]
│   ├── skillmap-corpus/      # research harvester (build step 1)  [T3, exists]
│   ├── skillmap-eval/        # labeled corpus, metrics, CI gate  [T6, exists]
│   └── skillmap-cli/         # bin: `skillmap` — lock, ci  [T8, exists]
├── rules/                      # TOML rule metadata (data, not code)
├── queries/                    # tree-sitter .scm queries
├── fixtures/                   # positive/negative + expected manifests
│   ├── bundles/                # whole-bundle corpus + blessed manifests (T2)
│   ├── adversarial/            # red-team cases + declared expectations (T6)
│   └── projects/               # v1.0/v1.1 pair for the escalation check (T8)
├── skillmap.lock               # this repo's own lock — skillmap gates skillmap
├── policy.toml                 # …and its own allowlist, which is empty
├── action.yml                  # the published GitHub Action wrapping `skillmap ci`
├── npm/skillmap/               # the wrapper package; platform packages are generated (T9)
├── eval/                        # committed baseline the CI gate compares against
├── schema/                     # JSON Schema for the manifest
├── npm/                        # wrapper package + per-platform binaries
└── .github/workflows/          # CI, release, and the published action
```

Dependency direction is strictly downward: `core` depends on nothing internal; `cli` depends
on everything; no crate depends on a sibling analysis plane.

`skillmap-scan` is not in the original plan and was added at T8. Manifest assembly lived
inside `skillmap-eval` from T6, with a note that a crate whose only job is to call three
functions — written before a second caller exists — would be a stub. T8 produced the second
caller: `skillmap ci` scans before it compares, and a product binary reaching into the test
harness for the ability to scan would have the arrow backwards.

The tree above is the **target**. `Cargo.toml`'s `members` list holds only crates that exist
today; each is added when its task in `docs/00-tasks.md` begins. Twelve empty crates would be
twelve stubs (invariant 12), and a workspace that cannot resolve is worse than a short one.

## Key abstractions

### `Resolver`

`SKILL.md` is one standard across agents. The **parser is ~90% shared** — what differs is
*discovery*: `.claude/skills/`, `.agents/skills/`, project vs. user scope, plugin bundles
that wrap several skills, per-agent frontmatter extras.

So: one parser, many resolvers.

```rust
pub trait Resolver {
    /// Stable identifier, e.g. "claude-code". Appears in manifest provenance.
    fn id(&self) -> &'static str;

    /// Candidate roots for this agent, relative to a project or home dir.
    fn search_paths(&self, scope: Scope) -> Vec<PathBuf>;

    /// Recognise a directory as a bundle root, and classify plugin wrappers.
    fn classify(&self, dir: &Path) -> Option<BundleKind>;

    /// Agent-specific frontmatter keys that carry declared capabilities, if any.
    fn declared_capability_keys(&self) -> &'static [&'static str] { &[] }
}
```

Adding an agent is a config file plus a `Resolver` impl of ~30 lines. Never fork the parser.

### Load-phase classification

Central to the thesis. Every file in the inventory is tagged with when it enters the agent's
context:

- `always` — the frontmatter description (the ~100 tokens seen at session start)
- `on_trigger` — the `SKILL.md` body
- `reference` — deep files reachable from the body by link or explicit instruction
- `unreferenced` — present in the bundle, reachable by no documented path

The **disclosure delta** — capabilities present in `reference` files but not implied by
`always` content — is the core signal. `unreferenced` files are independently interesting:
a script nothing links to is either dead weight or a payload waiting for a later commit to
wire it up.

### Rule engine

A rule is a directory-free triple:

- `rules/<lang>/<id>.toml` — metadata, capability mapping, assurance tier, fixtures
- `queries/<lang>/<id>.scm` — tree-sitter query producing captures
- `fixtures/<lang>/<id>/{positive,negative}.*` + `expected.json`

The engine loads and validates all rules at startup (`skillmap rules validate` in CI),
runs queries per parsed file, maps captures to capability findings, and attaches provenance
from the capture's byte range. See `docs/03-rules-authoring.md`.

### Reachability

A sink hit inside a function nothing calls is different from a sink hit on the entry path.
The code plane builds a call graph from bundle entry points (scripts named in `SKILL.md`,
executable files, conventional entry names) and marks findings:

- `observed` — reachable from an entry point
- `present` — the sink exists, reachability unproven
- `unresolved` — dynamic dispatch or computed target blocked the analysis

Intra-file reachability is v1. Cross-file is v1 for direct imports only; anything else is
`present`. Do not claim reachability the analysis did not establish.

## Language support

Ordered by what the corpus scan finds, not by preference. Expected: bash, python,
javascript/typescript, then the long tail. Every unsupported language yields an
`unresolved` inventory entry with reason `unsupported_language` — never silence.

## Distribution

Rust binary, npm wrapper, esbuild-style: `skillmap` is a thin package whose
`optionalDependencies` are per-platform packages (`@skillmap/linux-x64`,
`@skillmap/darwin-arm64`, …) each containing one prebuilt binary. The wrapper resolves and
execs. No `postinstall` download script — that is itself a supply-chain smell and would be
indefensible in this project specifically.

Also ship: `cargo install`, Homebrew tap, and a GitHub Action that wraps the CI subcommand.

Built at T9. Two things the original sketch did not anticipate:

**The binary carries its own rules.** Rules are data at the workspace root, so a shipped
binary had nothing to load. `crates/skillmap-rules/build.rs` walks `rules/` and `queries/`
and emits them as literals; `skillmap_rules::Source` gives the disk and embedded trees one
code path so they cannot drift, and a test compares them byte for byte. Adding a rule is
still a `.toml` and a `.scm` — invariant 7 is intact.

**`cargo install` is `--git`, not crates.io.** Cargo packages only files beneath a package's
own directory, so `skillmap-rules` cannot carry rule trees that live at the workspace root.
Moving them into the crate would bury the contributor-facing surface; a synchronized copy is
a second copy that drifts. `docs/07-distribution.md` states the gap rather than hiding it.

## Where this dies, and the hedge

Registries absorb scanning — publisher signing, marketplace-side checks. Assume ~12 months.

The hedge is deliberate and shapes the architecture: be the **policy and CI layer inside
orgs**, and be **cross-agent**. No single registry will write policy that spans Claude Code,
Cursor, Codex, and Windsurf. That is why `Resolver` is an abstraction from commit one rather
than a Claude-Code-shaped hardcode, and why `policy` and `diff` are first-class crates rather
than CLI flags.

The durable asset is not the scanner — it is the corpus and the growing labeled set.
Treat `crates/skillmap-corpus` and `fixtures/` as the crown jewels.
