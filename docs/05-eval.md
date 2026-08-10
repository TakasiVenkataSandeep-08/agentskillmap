# Eval

Eval is a shipped artifact, not a phase. It is the operational definition of "not a stub":
**measured precision and recall against a public labeled corpus, published per release.**

Without it, the quality claim is unfalsifiable and the project is indistinguishable from a
regex script with good documentation.

## Three suites

### 1. Fixture suite (per-rule, deterministic)

Every rule's positive and negative fixtures, run on every commit. Binary pass/fail.
Catches rule regressions immediately. Fast — must stay under a few seconds so nobody is
tempted to skip it.

### 2. Corpus suite (aggregate, deterministic)

The full labeled corpus from step 1. Reports precision and recall **per capability term**,
not blended. A tool with 0.94 aggregate precision that misses every `net.fetch_then_execute`
is not a good tool, and the aggregate hides it.

Also reports:
- **`unresolved` rate** — what fraction of the corpus the analysis could not fully cover.
  This number going *up* on a release is acceptable if it reflects newly-honest reporting;
  it going quietly *down* while recall is flat means something is being silently dropped
  (invariant 3 violation). Watch it.
- **Coverage** — bundles where at least one file parsed, by language.

### 3. Adversarial suite (red team)

Skills you author to defeat your own scanner. Minimum set, each with an expected outcome:

| Case | Expected |
|---|---|
| Obfuscated exec (base64 → decode → `eval`) | `code.obfuscation` + `code.dynamic_eval` |
| Credential read via computed path | `unresolved: computed_target`, **not** silence |
| Payload in an `unreferenced` file | inventory flags `unreferenced` |
| Capability added in v1.1 only | diff reports escalation |
| Injection inside a `reference` file | `injection_attempt`, and the auditor is unaffected |
| Sink in dead code | `reachability: present`, not `observed` |
| Legitimate skill that needs shell + network | reported, **no** score, no alarm language |
| Documentation that mentions `~/.aws/credentials` in an example | no finding |

The last two are as important as the rest. A scanner that cannot stay quiet on legitimate
behaviour will not survive contact with users.

## Metrics discipline

- **Held-out split**, fixed by seed, never tuned against. If you tune on the test split
  the number is decoration.
- **The headline metric is false-positive rate on the benign stratum.** On a corpus where
  most skills are fine, that is what determines whether the tool gets uninstalled.
- Published in the README per release, with the corpus version and commit that produced it.
- CI gate: regression beyond declared tolerance fails the build (invariant 11).
- When a metric moves, the release notes say why. "Improved detection" is not an explanation.

## Anti-goal

Do not optimize toward a benchmark number. The corpus is a sample of a fast-moving ecosystem
and will drift; a scanner tuned to 0.99 on it is overfit to last quarter's skills. Prefer
a slightly worse number with a rule set a human can read and reason about.
