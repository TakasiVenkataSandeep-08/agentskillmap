# Security

## Threat model

`skillaudit` analyzes hostile input by design. A skill bundle is arbitrary text and code
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

1. `--semantic` — one model API call per chunk. Nothing else.
2. `skillaudit corpus` — the research harvester, which fetches public repositories.

There is **no telemetry**. No analytics, no accounts, no phone-home, not even opt-in. A
supply-chain tool with a supply-chain problem is worthless, and this project would have no
standing to audit anyone else while shipping a beacon.

## Our own supply chain

- Reproducible builds. Two builds of the same tag from clean checkouts are byte-identical;
  an unreproducible release is a release blocker.
- Signed release artifacts, verifiable without trusting the download host.
- No `postinstall` script in the npm package. Per-platform binaries ship as
  `optionalDependencies`, so nothing executes at install time.
- Minimal dependency tree. Every dependency is one this project would have to defend in the
  threat model above.

## Reporting

Report vulnerabilities in `skillaudit` privately via GitHub Security Advisories rather than
a public issue.

## Reporting findings about third-party skills

The corpus research will surface real skills doing questionable things.

- Describe patterns, not people. Published research names no maintainer as a suspect.
- Anything that looks live and exploitable goes through coordinated disclosure to the
  maintainer and the hosting registry first, with a 90-day default timeline.
- Do not publish a working exploit chain for a live skill. The capability manifest and the
  evidence spans are sufficient to make the point.
