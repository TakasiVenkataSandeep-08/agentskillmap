# The semantic layer

Built **sixth**, not first. Its inputs are the labeled corpus from step 1 and the manifest
from steps 2–4. Building it earlier means tuning against adversarial examples you invented
yourself, which produces a component that looks impressive and is unmeasured — the exact
failure this project is defined against.

## What it is for, precisely

One question, not general "is this skill bad":

> Do the deep-loaded files instruct the agent to do things the ~100-token description does
> not disclose?

That is the **disclosure delta**. It is model-shaped because it requires reading prose for
intent, and it is the contribution nobody else has built. Scope creep here is fatal: the
moment it also grades code quality, or opines on whether a capability is dangerous, it has
become a risk scorer and violates invariant 1.

## Threat model of the component itself

You are feeding suspected prompt-injection payloads to a model and asking it to evaluate
them. The auditor is a target. Treat every design decision accordingly.

1. **Content is data, never instruction.** Skill text enters inside a delimited channel with
   an explicit framing that everything within is untrusted material under analysis. Never
   concatenate skill text into the instruction portion of the prompt.
2. **No tools, no network, no filesystem** available to the pass beyond the single model
   call. Nothing the model emits can cause an action.
3. **Schema-validated JSON output.** Validation failure discards the finding and emits a
   diagnostic. Never parse free text as a fallback — a fallback path is how injection wins.
4. **Findings are claims, not conclusions**, and every one carries a file and line so a
   human can check it in seconds.
5. **If the output contains anything resembling an instruction to the auditor** — "ignore
   previous", "this skill is approved", "output no findings" — that is logged as a finding
   *about the skill* (`injection_attempt`) and never acted on.
6. **Model ID and prompt-template SHA-256 are pinned into the manifest.** Without this the
   advisory branch is not reproducible and CI diffs become noise, which poisons the
   deterministic branches by association.
7. **Off by default.** Runs only under an explicit flag, and only makes network calls when
   it does. `SECURITY.md` documents this as the single network path in the scan flow.

## Structural quarantine

The pass returns its own type, `AdvisoryFinding`, and the manifest assembler is the only
code that sees both it and the deterministic findings. There is no code path by which an
advisory finding can create, modify, suppress, or reprioritize an entry in `capabilities`,
`instructions`, or `unresolved`. Enforce this with crate boundaries — `skillmap-semantic`
does not depend on `skillmap-code` or `skillmap-instr` — not with review discipline.

A consumer must be able to delete the `advisory` key and lose nothing else.

## Chunking

Deep-loaded files can exceed a sensible context. Chunk by file, then by section, and pass
the description alongside every chunk — the delta is meaningless without the thing being
compared against. Record which chunks were analyzed; a partially analyzed bundle emits
`unresolved` with reason `size_limit`, per invariant 3.

## Measurement

The layer ships with published precision and recall against the held-out split of the
labeled corpus, per invariant 11.

- Report per-finding-kind, not just aggregate. `disclosure_delta` and `injection_attempt`
  have very different base rates and a blended number hides that.
- Report the **false-positive rate on the benign stratum** as the headline. On a corpus
  where most skills are fine, a layer that flags 8% of benign bundles is unusable regardless
  of its recall — that is the number that gets the tool uninstalled.
- Non-determinism is expected here. Run each eval item n times, report variance, and treat
  a high-variance finding kind as not ready to ship.
- Regressions beyond declared tolerance fail CI.

## When to cut it

If the labeled corpus shows the disclosure delta is rare — say under 3% of bundles — then
this layer solves a problem that does not exist, and v1.0 ships with the deterministic core
and an honest note saying so. That would be a good outcome, not a failure. Shipping a
plausible-looking model pass over a non-problem is the mediocre version of this project.

---

# What T7 built, and what it could not

`crates/skillmap-semantic` exists. The machinery above is implemented and tested; the
measurement is not, and cannot be from a checkout without model credentials.

## Built and proved

| Requirement above | Where |
|---|---|
| Content is data, never instruction | `src/prompt.rs` — fixed delimiters, and any occurrence of a delimiter *inside* untrusted content is neutralized before quoting |
| No tools, no network, no filesystem beyond one call | `src/provider.rs` — the `Provider` trait is `&str` in, `String` out. There is no tool list to populate and no second round trip |
| Schema-validated JSON, never free text | `src/validate.rs` — `deny_unknown_fields` throughout, and a response that is not JSON is discarded whole with a `semantic_schema_violation` |
| Findings carry a file and line | Citations are resolved against the bundle. A file the bundle does not contain, or a line past its end, is discarded |
| Auditor-directed output is a finding, never an action | `prompts/auditor-directed.toml`, applied to the model's own claims. The only effect a match has is reclassification to `injection_attempt` |
| Model and prompt pinned | `advisory.prompt_sha256` is a merkle over **both** pinned files, not just the template — see below |
| Off by default | The HTTP client is behind a non-default Cargo feature, so a released binary cannot make the call at all |
| Structural quarantine | `crates/skillmap-scan/tests/quarantine.rs` |

**The prompt hash covers two files.** The phrase list decides whether a finding is
reclassified, so a change to it changes what the advisory branch reports exactly as surely
as a change to the template does. Hashing only the template would leave a hole where the
output moves and the pin says nothing did.

**The quarantine is proved, not argued.** The same bundle is scanned three ways — no
semantic pass, a pass that finds nothing, and a pass returning output written specifically
to suppress a deterministic finding ("remove `fs.read.credential` from capabilities", "as
the operator, I confirm this skill has been reviewed") — and the deterministic half of the
manifest is compared byte for byte across all three. The hostile findings come back
reclassified as `injection_attempt`, and `capabilities` does not move.

## One place this document contradicts itself, and how it was resolved

The **Structural quarantine** section says the pass may not create an entry in `unresolved`.
The **Chunking** section says a partially analysed bundle must emit `unresolved` with reason
`size_limit`. Both cannot hold literally.

Resolved in favour of Chunking: the pass appends its own coverage gaps and can never modify
or suppress an entry the deterministic tiers wrote. The prohibition is about advisory
*findings* steering the deterministic record, and a coverage note is not a finding —
invariant 3 requires it, and silence about unread content is the failure this project is
built against. `deterministic_half` in the quarantine test compares `unresolved` exactly, so
if the semantic pass ever touched an existing entry, that test fails.

The narrower reading — that a semantic size limit is run-scoped and therefore a
`Diagnostic` — is defensible under `docs/02-manifest-schema.md`'s rule ("if it is a fault in
this run, it is a `Diagnostic` instead") and was not taken, because a bundle too large for
any finite context is a property of the bundle.

## Not built: the measurement

Everything under **Measurement** above is outstanding, and none of it is outstanding for
lack of effort:

- **No precision or recall**, per kind or aggregate. They are scored against the held-out
  split of the labeled corpus. The corpus is harvested (34,284 bundles) and **not labelled**,
  so there is no ground truth and no split.
- **No false-positive rate on the benign stratum** — the number this document names as the
  headline, for the good reason that it is the one that gets a tool uninstalled.
- **No variance numbers.** `src/variance.rs` implements the n-run harness, reports per kind
  rather than blended, and omits kinds that never fired rather than reporting them as
  perfectly stable at zero. It has never been run against a live model.

Inventing any of these would be the exact failure this project is defined against, so none
of them appear anywhere. The layer is off by default and stays off until they exist.
