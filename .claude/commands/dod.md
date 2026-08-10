---
description: Run the definition-of-done checklist from AGENTS.md against the current working diff, using the invariant-auditor subagent for the invariant-compliance items.
---

Run the definition-of-done checklist (bottom of `AGENTS.md`) against the working diff.
Re-read `AGENTS.md`'s checklist directly rather than relying on memory of it — it is
canonical and this command must stay consistent with edits to it.

1. Get the diff: `git status` for untracked files, `git diff` (and `git diff --staged`) for
   changes. If there is no diff, say so and stop — there is nothing to check.

2. Dispatch the `invariant-auditor` subagent against this diff. It covers most of the
   checklist directly:
   - Fixtures added (positive + negative where applicable) → invariant 8 check
   - `unresolved` emitted for anything not fully analyzed → invariant 3 check
   - Provenance on every new finding type → invariant 4 check
   - No new `unwrap`/`expect`/`panic!` in a library crate → invariant 10 check
   - Schema version bumped if the manifest shape changed, with a migration note →
     invariant 2 / 5 checks plus the schema-version-event rule

3. Check the two remaining checklist items yourself, since they're process facts rather
   than code-content facts the subagent is built to find:
   - **Determinism test still byte-identical** — if `skillaudit-core`'s determinism test
     exists and is runnable, run it (`cargo test` targeting the determinism test) and
     report pass/fail. If it doesn't exist yet (pre-T1), say so explicitly rather than
     marking this item done.
   - **Docs updated in the same commit** — check whether files this diff touches have a
     corresponding doc (`docs/*.md`, `ARCHITECTURE.md`, `AGENTS.md`) that describes the
     changed behavior, and whether that doc changed too. Flag anything that looks like a
     behavior change with no doc update alongside it.

4. Report as a checklist: each item marked done / not done / not applicable-yet (with why),
   plus the invariant-auditor's findings inline under the items they correspond to. Do not
   mark an item done because it's plausible — mark it done because you or the subagent
   actually verified it against the diff.
