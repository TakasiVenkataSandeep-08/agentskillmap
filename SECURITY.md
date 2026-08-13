# Security

## Threat model

`skillmap` analyzes hostile input by design. A skill bundle is arbitrary text and code
authored by an unknown party, and some fraction of what this tool reads will be actively
trying to defeat it.

Assumed adversary goals, in rough order of likelihood:

1. Hide a capability from the manifest (obfuscation, computed targets, dead-code staging).
2. Defeat the semantic layer via prompt injection in a deep-loaded reference file.
3. Crash or hang the scanner to get a CI check skipped.
4. Escape the bundle root during the file walk (symlinks, path traversal).

Consequences that shape the design:

- **Panics are security bugs.** A parser crash on a malformed bundle is a denial of service
  on someone's CI. `unwrap`, `expect`, `panic!`, and unchecked indexing are lint-denied in
  every library crate.
- **Silence is a security bug.** Anything the analysis could not cover is reported as
  `unresolved`. A scanner that reports nothing because it understood nothing must be
  visibly distinct from one that found nothing.
- **The semantic layer is an attack surface**, not just an analysis feature. Its quarantine
  rules are in `docs/04-semantic-layer.md` and are not negotiable.

## Network behaviour

Scanning is **offline**. The binary makes network calls in exactly two places, both of which
require explicit opt-in:

1. `--advisory <model>` — **one** model API call per bundle. Nothing else. The key comes from
   `ANTHROPIC_API_KEY` and never from a flag, a file skillmap writes, or a prompt.
2. `skillmap corpus` — the research harvester, which fetches public repositories.

**Where the network code actually lives.** Only `crates/skillmap-corpus` and
`crates/skillmap-semantic` link an HTTP client, and the second does so **only under a
non-default Cargo feature**. A released binary contains no client at all: `--advisory` on one
of those builds is an error telling you to rebuild, not a silently disabled pass. So the
offline guarantee is a property of the dependency graph rather than of discipline — no crate
on the default scanning path can make a request, because none of them can reach a socket.

**The model call itself is one round trip with no tools.** `skillmap_semantic::Provider` takes
a string and returns a string; there is no tool list to populate and no second turn, so
nothing the model emits can cause an action. Its output is parsed as schema-validated JSON or
discarded — never mined out of prose, because a fallback path is how an injection wins — and
citations that do not resolve to a real file and line in the bundle are dropped. Findings land
in tier `advisory` and provably cannot alter any deterministic branch; see
`crates/skillmap-scan/tests/quarantine.rs`. The harvester contacts exactly one host, `api.github.com`, and does so only
for authenticated GETs that return JSON. Bundle contents come down through `git clone`
rather than an in-process transfer, which keeps the HTTP surface to a single verb and
reuses a tool the operator already trusts.

The client is `ureq`: blocking, pure-Rust over rustls, no async runtime. `reqwest` was
rejected for the obvious reason — tokio, hyper, and roughly 130 transitive crates is not a
tree this project can defend while auditing anyone else's.

There is **no telemetry**. No analytics, no accounts, no phone-home, not even opt-in. A
supply-chain tool with a supply-chain problem is worthless, and this project would have no
standing to audit anyone else while shipping a beacon.

## Our own supply chain

All four of these are enforced rather than promised. `docs/07-distribution.md` has the detail,
including the two bugs that stood between the first three and being true.

- **Reproducible builds.** Two builds of the same tag from clean checkouts are byte-identical.
  `.github/workflows/release.yml` builds every tag twice, from two different directories, and
  publishes nothing if they differ — an unreproducible release is a release blocker, and this
  is where the block happens.
- **No build path or username reaches a published binary.** `scripts/build-release.sh` remaps
  the workspace and the Cargo registry, then **greps the finished binary** for both and
  refuses to publish on a hit. The check is not redundant with the one above: the first
  version of the remapping matched nothing, and byte-identity did not notice, because both
  builds ran as the same user.
- **Signed release artifacts, verifiable without trusting the download host.** Keyless
  sigstore attestations bound to the release workflow:
  `gh attestation verify skillmap-linux-x64.tar.gz --repo agentskillmap/agentskillmap`. Deliberately not
  a long-lived key this project would have to store, rotate, and eventually mishandle.
- **No `postinstall` script in the npm package.** Per-platform binaries ship as
  `optionalDependencies`, so nothing executes at install time and nothing is downloaded
  outside npm's own integrity checking.
- **Minimal dependency tree.** Every dependency is one this project would have to defend in the
  threat model above. `cargo deny` gates it on every push.

## Reporting

Report vulnerabilities in `skillmap` privately via GitHub Security Advisories rather than
a public issue.

## Reporting findings about third-party skills

The corpus research will surface real skills doing questionable things.

- Describe patterns, not people. Published research names no maintainer as a suspect.
- Anything that looks live and exploitable goes through coordinated disclosure to the
  maintainer and the hosting registry first, with a 90-day default timeline.
- Do not publish a working exploit chain for a live skill. The capability manifest and the
  evidence spans are sufficient to make the point.
