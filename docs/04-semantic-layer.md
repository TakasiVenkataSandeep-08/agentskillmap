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
