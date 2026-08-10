# Step 1 — the corpus scan

This is the first thing built and the first thing published. It gates the format-support
decision, produces the labeled data the semantic layer needs, and is the artifact that gets
the project noticed.

**It is also the kill switch.** If the base rates come back boring — 2% ship scripts, nobody
touches credentials — the risk is theoretical and you have killed a bad idea for the cost of
a weekend. Publish either way. A negative result honestly reported is worth more to your
credibility than a scanner nobody needs.

## Deliverable

`skillaudit corpus` — a subcommand in `crates/skillaudit-corpus`, not a throwaway script.
It shares the parser and inventory code with the main path, which is exactly why it goes
first: writing it forces the parser and the manifest into existence against real input
instead of imagined input.

Outputs:
- `corpus/index.json` — one record per bundle, content-addressed, deduplicated by digest
- `corpus/raw/<digest>/` — archived bundle contents, so results are reproducible later
- `corpus/report.md` — the human-readable findings

## Sources

1. `github.com/anthropics/skills` (baseline: what "good" looks like)
2. GitHub code search for `path:**/SKILL.md`, paginated
3. The curated lists — `ComposioHQ/awesome-claude-skills` and similar
4. Public marketplace and registry listings
5. High-star single-skill repos surfaced by the listicle ecosystem

**Rate limits are the practical constraint.** Unauthenticated GitHub is 60 requests/hour;
authenticated is 5,000. Require a `GITHUB_TOKEN` and fail fast with a clear message if it is
absent. Cache aggressively by digest — a re-run must not re-fetch.

**Sampling discipline:** record how each bundle was discovered. A corpus drawn only from
"top 50 skills" listicles measures the curated head, not the ecosystem. Report head and tail
separately or the base rates are meaningless. Note also that star counts across the blog
coverage of this ecosystem are wildly inconsistent for the same repos — do not use secondary
sources for any number you publish. Read the API.

## Measurements

Purely mechanical. No model, no judgement. Report counts and percentages, with the
denominator stated every time.

**Structure**
- Bundles with executable scripts alongside `SKILL.md`, by language
- File count, total bytes, and bytes reachable only via `reference` load phase
- Ratio of `always`-phase bytes (the ~100-token description) to total bundle bytes —
  this quantifies the progressive-disclosure gap across the ecosystem
- Bundles containing `unreferenced` files

**Capability surface**
- References to credential paths: `.env`, `~/.aws`, `~/.ssh`, `~/.config/gh`, keychain
- Secret-bearing env var reads: `*_TOKEN`, `*_KEY`, `*_SECRET`, `GITHUB_TOKEN`
- Outbound network in scripts, and whether hosts are static or computed
- Writes to `CLAUDE.md`, `settings.json`, hooks, statusline config
- `eval` / `exec` / `source` of computed content; encode-decode chains
- Install-time network fetch (`postinstall`, `curl | sh` in docs)

**Governance**
- Bundles with any version marker at all
- Bundles updated after first publication (requires two snapshots — start the clock now,
  even if the second pass is weeks out)
- License present / absent
- Publisher: official, verified, or anonymous

**Format spread** — the input to your scope decision
- Which discovery conventions actually appear in the wild, with counts
- Whether frontmatter beyond `name` and `description` is used, and by whom
- Whether any agent-specific divergence exists that a single parser cannot absorb

## Format-scope decision rule

Decide from the data, not from ambition. Support a resolver in v1 if its convention appears
in ≥5% of the corpus, or if it is the default for an agent with material install share.
Everything else gets an issue and a `Resolver` impl waiting for a contributor — which is
cheap precisely because the parser is shared.

## Labeling pass

After the mechanical scan, hand-label a stratified sample (~150 bundles) for disclosure
delta: does the deep content instruct capabilities the description doesn't disclose?
Two labels minimum per bundle, disagreements adjudicated and recorded as such.

This sample is the ground truth the semantic layer is measured against, and it is the reason
the semantic layer is built sixth rather than first. It is also the single most valuable
asset the project will produce — it is not reproducible by a competitor without redoing the
work. Version it, license it clearly, and keep it in-repo.

## Report

Lead with the base rates and the denominators. State the sampling method and its bias before
the findings, not in a footnote. Include the negative results. Name no maintainer as a
suspect — describe patterns, not people, and follow `SECURITY.md` disclosure timelines for
anything that looks live.

The report's job is to establish that the problem is real and that you measure carefully.
Both halves matter; the second is what makes the scanner credible when it ships.
