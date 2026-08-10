---
description: Scaffold a new rule triple (TOML metadata + tree-sitter .scm query + fixtures) from the reference credential-read rule, then walk through authoring it via the rule-author skill.
---

Arguments: `$ARGUMENTS` is `<language> <rule-id>`, e.g. `python exec-eval` or `bash net-curl`.

Parse `$ARGUMENTS` into `<language>` and `<rule-id>`. If either is missing, stop and ask for
both — do not guess a rule-id from context.

Then:

1. Read the reference triple in full before writing anything:
   `rules/python/credential-read.toml`, `queries/python/credential-read.scm`,
   `fixtures/python/credential-read/positive.py`,
   `fixtures/python/credential-read/negative.py`,
   `fixtures/python/credential-read/expected.json`. Also read
   `docs/03-rules-authoring.md` in full — it is the spec this scaffold has to satisfy.

2. Check whether `rules/<language>/<rule-id>.toml`, `queries/<language>/<rule-id>.scm`, or
   `fixtures/<language>/<rule-id>/` already exist. If any do, stop and report the conflict
   instead of overwriting.

3. Create the three-file skeleton, matching the reference shape:
   - `rules/<language>/<rule-id>.toml` — `id`, `language`, `capability` (must already exist
     in the taxonomy table in `docs/02-manifest-schema.md` — if the right capability term
     doesn't exist yet, stop and say this needs a schema-version event first, don't invent
     one), `tier` (`proven` for code-plane, `pattern` for instruction-plane markdown rules),
     `query` path, a `[captures]` table with at minimum `site`, a `[match]` table (empty
     list to fill in, never string patterns baked into the query), and a `[docs]` table
     with `summary`, `rationale`, `false_positive_notes` left as prompts for the author to
     fill in honestly — do not invent plausible-sounding rationale text.
   - `queries/<language>/<rule-id>.scm` — a skeleton with a comment header pointing at what
     structural pattern needs to be matched, left for the author to write; do not fabricate
     a query that hasn't been checked against the grammar.
   - `fixtures/<language>/<rule-id>/positive.<ext>`, `negative.<ext>`, `expected.json` — do
     **not** invent contents. Leave `positive`/`negative` as empty stubs with a comment
     saying what real-world code needs to go there, and note explicitly that the negative
     fixture must come from a real bundle, per invariant 8 — this scaffold cannot supply
     that from imagination.

4. Invoke the `rule-author` skill to walk through filling in the scaffold correctly — do
   not try to complete the rule from this command's instructions alone; the skill has the
   full list of what's easy to get wrong (capture roles, span size, dynamic targets, the
   closed capability vocabulary).

5. Remind the user at the end: `skillaudit rules validate` does not exist yet (it's task
   T4 in `docs/00-tasks.md`), so nothing here is machine-checked — the triple has to be
   reviewed by hand against `docs/03-rules-authoring.md` until then.
