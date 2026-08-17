# Distribution

How `skillmap` gets onto a machine, and why each choice is the one a
supply-chain auditor can defend.

The uncomfortable part first: this tool tells people to be careful about
installing arbitrary code from the internet. Its own installation path is the
first thing a reasonable reader will check, and "do as I say" is not an
available answer.

---

## The binary carries its own rules

Detection is data (invariant 7) — `rules/*.toml` and `queries/*.scm` at the
repository root. Through T8 that meant `skillmap ci` needed `--rules` pointing at
a checkout, which is fine for this repository's own CI and useless to anyone
else.

`crates/skillmap-rules/build.rs` now walks both trees at build time and emits
them as string literals. Consequences worth being explicit about:

- **Adding a rule is still a `.toml` and a `.scm`.** The build script walks; it
  has no list to update and knows nothing about what a rule means.
- **`--rules <dir>` still works**, for developing against an edited tree.
- **`skillmap rules` prints what the running binary carries.** The rules are no
  longer visible on disk beside the tool, and the first question anyone asks
  about a finding they disagree with is which rule produced it.
- **A build that embedded nothing is refused**, by the build script, before a
  binary exists. A scanner with no rules reports every project clean, which is
  the single worst output this tool can produce — and it would produce it
  silently, in the direction that looks like good news.
- **The contents are written as escaped literals, not `include_str!`.** An
  `include_str!` leaves absolute paths in the generated source, and the promise
  below is that no build path reaches a published binary.

`crates/skillmap-rules/tests/embedded.rs` compares the embedded tree against the
on-disk one byte for byte, on every build. A stale or partial embed would ship a
quieter scanner than the one this repository tests, and quieter is exactly the
failure that does not announce itself.

---

## Reproducible builds

**The claim:** two builds of the same commit, from different directories, produce
byte-identical binaries.

**Why it matters:** it is what makes an independent rebuild meaningful. Without
it, a signature proves only that a particular workflow emitted particular bytes;
with it, anyone can build the tag themselves and compare. That is the difference
between "trust this release" and "check this release".

`scripts/build-release.sh` is the single source of the flags, used by both the
release job and the job that verifies reproducibility. Two copies of a flag list
is one copy too many for a promise this specific.

### What broke it, in the order it was found

| Cause | Fix |
|---|---|
| MSVC writes the wall clock into the PE header | `-Clink-arg=/Brepro` |
| Registry dependency paths are absolute, and contain a username | `--remap-path-prefix` for `CARGO_HOME` |
| C sources bake `__FILE__` into assertions | `-ffile-prefix-map` via `CFLAGS` |
| Parallel codegen lets the optimizer choose differently | `codegen-units = 1` (already in `[profile.release]`) |

Two of those are worth expanding, because both looked fine before they were
measured:

**The timestamp.** The first two builds differed in exactly 24 bytes. Every one
was the PE `TimeDateStamp`, repeated through the COFF header and the debug
directory entries. Nothing about the source, nothing about the paths — a clock.

**The registry paths.** The workspace's own sources never leaked, because Cargo
compiles workspace members with *relative* paths. Dependencies are compiled by
absolute path, so ninety-one `C:\Users\<name>\.cargo\registry\...` strings sat in
the binary as panic locations — in `.rodata`, where `strip` does not reach. The
remap flag for them was present and matched nothing, because under Git Bash the
script saw `/c/Users/...` while rustc emits `C:\Users\...`, and
`--remap-path-prefix` is an exact prefix match.

Both builds were byte-identical *to each other* the whole time, because both ran
as the same user on the same machine. **Byte-identity did not catch this.** The
script therefore greps the finished binary for the workspace path, `CARGO_HOME`
and `$HOME`, in both path spellings, and refuses to publish if it finds any.
Verifying beats asserting; that is the entire lesson.

### Verifying a release yourself

```bash
git clone https://github.com/TakasiVenkataSandeep-08/agentskillmap && cd agentskillmap
git checkout v0.5.0
bash scripts/build-release.sh
sha256sum dist/skillmap
```

Compare against `SHA256SUMS` on the release. Same OS and architecture, same
pinned toolchain (`rust-toolchain.toml`, honoured automatically by rustup).

---

## Signing

GitHub build provenance attestations, sigstore-backed and keyless:

```bash
gh attestation verify skillmap-linux-x64.tar.gz --repo TakasiVenkataSandeep-08/agentskillmap
```

Deliberately **not** a long-lived signing key. A key this project would have to
store, rotate, and eventually mishandle is a worse story than an attestation
bound to the workflow identity that produced the artifact — and the attestation
answers the question people actually have, which is "did this come from that
repository's release workflow", not "does someone hold a matching private key".

The attestation covers the published archives. The npm packages are published
with `--provenance`, which is the same mechanism through npm's registry.

---

## Install paths

### npm (recommended for CI)

```bash
npm install --save-dev skillmap
```

esbuild's shape: the `skillmap` package contains a Node shim and no binary. Each
platform's binary lives in its own package (`@agentskillmap/linux-x64`, …) declaring
`os` and `cpu`, and all five are listed as `optionalDependencies`. npm installs
the one that matches.

**There is no `postinstall` script anywhere in this tree.** A postinstall that
downloads a binary is arbitrary code fetching an arbitrary payload at install
time over a channel npm does not verify — a fair description of the threat this
project exists to describe in other people's repositories. Instead the bytes
arrive as ordinary npm dependencies, with the same integrity hashes in the
lockfile as everything else.

The shim's one subtlety is exit codes. `skillmap ci` uses `0`/`1`/`2`/`3`/`4` to
mean different things (`docs/06-policy-and-lock.md`), so the wrapper passes the
child's status through unchanged, and maps "killed by a signal" to `4` rather
than to `0` — a crashed scan must never read as a passing check.

### GitHub Action

```yaml
- uses: TakasiVenkataSandeep-08/agentskillmap@v1
```

See `action.yml`. Pin the `ref` input to a tag: leaving it on a branch means the
rules that decide whether your build passes can change without a commit in your
repository.

### Homebrew

```bash
brew install TakasiVenkataSandeep-08/agentskillmap/skillmap
```

The formula is generated at release time by `scripts/homebrew-formula.sh` from
the checksums that were actually published, never hand-edited — a stale `sha256`
fails at install with a checksum mismatch, which reads to a user exactly like a
compromised download.

**The tap is live**, at `TakasiVenkataSandeep-08/homebrew-agentskillmap`, carrying
`Formula/skillmap.rb`. Verified against v0.5.0: the four archive checksums in the
published formula match the release's `SHA256SUMS`, and the install was run.

**Updating it is automatic, and gated on a second credential.** The release
workflow regenerates the formula from the checksums it just published and pushes
it into the tap. That is a write to a *different* repository, so the workflow's
own `github.token` cannot do it: it needs `HOMEBREW_TAP_TOKEN`, a fine-grained
PAT with `Contents: write` on `homebrew-agentskillmap`. Absent, the step skips
with a notice and the tap keeps serving the previous version — a release that
shipped binaries is not marked failed for want of a convenience.

The tap's own README says the file is generated and must not be edited by hand,
because editing it is how a checksum silently stops matching the bytes it names.

### From source

```bash
cargo install --git https://github.com/TakasiVenkataSandeep-08/agentskillmap skillmap-cli
```

**Not `cargo install skillmap-cli` from crates.io**, and this is a real
limitation rather than an oversight. Cargo packages only files beneath a
package's own directory, so a crates.io release of `skillmap-rules` cannot carry
`rules/` and `queries/` from the workspace root, and its build script would find
nothing and refuse to build. The two ways out are both worse than the gap:
moving the rule trees inside the crate would bury the contributor-facing surface
that invariant 7 exists to keep visible, and committing a synchronized copy
inside the crate is a second copy that drifts. `--git` builds from a checkout,
where both trees are present.

---

## What is not here

- **The Homebrew tap repository.** Formula generated and released; nowhere to
  push it yet.
- **crates.io publishing**, for the reason above.
- **Windows on ARM, and 32-bit anything.** Five targets ship; the npm wrapper
  names the others and points at `cargo install --git`.
- **A cross-machine reproducibility check.** CI proves two directories on one
  runner agree. Two different machines agreeing is the stronger claim, and
  checking it needs a second builder this project does not have.
