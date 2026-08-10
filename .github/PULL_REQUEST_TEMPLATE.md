## What this changes and why

<!--
One or two sentences. If this closes an issue, link it. If this is rule coverage, name the
capability term and the pattern it now catches; if it's engine/docs/infra, say what stage of
docs/00-tasks.md it belongs to.
-->

## Rule PR? (delete this section if not applicable)

- Capability term: `<taxonomy term from docs/02-manifest-schema.md>`
- Positive fixture: `fixtures/<lang>/<id>/positive.<ext>` — describe the case it triggers on
- Negative fixture: `fixtures/<lang>/<id>/negative.<ext>` — **drawn from a real bundle**
  (name it or link it), not invented
- `expected.json` — blessed via `skillmap rules bless`, not hand-written (if that command
  does not exist yet in this repo, say so and note it will need blessing once it does)

## Definition of done

Carried verbatim from `AGENTS.md`. Check what applies; if something doesn't apply, say why
in a comment rather than silently leaving it unchecked.

- [ ] Fixtures added (positive + negative where applicable)
- [ ] Determinism test still byte-identical
- [ ] `unresolved` emitted for anything not fully analyzed
- [ ] Provenance on every new finding type
- [ ] No new `unwrap` in a library crate
- [ ] Schema version bumped if the manifest shape changed, with a migration note
- [ ] Docs updated in the same commit
