---
description: Scan the fixture corpus twice and byte-compare the resulting manifests locally, so nondeterminism is caught before CI. Currently cannot run — the scanner does not exist yet.
---

This command is a placeholder for a real local determinism check, and it is not runnable
yet. Say so plainly and stop — do not simulate a scan or fabricate a "pass" result.

**Why it can't run:** the check this command names — "scan the fixture corpus twice, byte-
compare the two manifests" — requires a working `skillmap` binary that can actually
produce a manifest from a bundle. That's `skillmap-core` (manifest types + `canonicalize()`,
task T1) plus `skillmap-parse`/`skillmap-resolve` (task T2) at minimum, and realistically
`skillmap-rules`/`skillmap-code` (task T4) too, since the fixture corpus is only
interesting to scan once rules produce findings. None of these crates exist yet — see
`docs/00-tasks.md` and `Cargo.toml`'s (empty) `members` list.

**What to do when invoked:**

1. Check whether `crates/skillmap-cli` exists and builds
   (`cargo build -p skillmap-cli` if the crate is present). If it doesn't exist, report
   exactly that — "the scanner binary does not exist yet (pre-T1/T4 per docs/00-tasks.md);
   this command has nothing to run" — and stop. Do not proceed to steps below.

2. If (once T1/T2/T4 land) a binary does exist, the real procedure this command should run
   is:
   - Enumerate every fixture bundle under `fixtures/` (each `rules/<lang>/<id>/` triple's
     positive and negative fixtures, treated as minimal single-file bundles, plus any full
     bundle fixtures added later for parser/resolver testing).
   - Run the scanner against each bundle twice, in two separate process invocations, writing
     each run's canonical manifest to a temp file.
   - Byte-compare (not structurally-compare — literal byte compare) the two output files per
     bundle.
   - Report any bundle where the two runs differ, with a diff of the two manifests, since
     that's a P0 per invariant 2 — not a nit to triage later.
   - This is the *local* version of the CI test described in invariant 2
     ("scans the fixture corpus twice on two platforms and byte-compares"); it only proves
     same-machine determinism, not cross-platform, but catches the common case
     (`HashMap` iteration, unsorted `Vec`, a stray timestamp) before a CI round-trip is
     needed.

3. Until step 2 is possible, this command's only correct behavior is the honest "not yet
   runnable" report from step 1.
