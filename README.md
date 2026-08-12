# skillmap

> Name is a placeholder — check crates.io and npm availability before first publish.

A supply-chain auditor for AI agent skills. It answers **"what does this skill make my agent
capable of doing?"** with byte-level evidence, and diffs that answer across versions.

It is not a linter, not a risk scorer, and not a malware classifier. It emits a capability
manifest; your `policy.toml` decides what is acceptable.

## Why

`SKILL.md` is an open standard read by Claude Code, Claude.ai, the Anthropic API, Codex,
Cursor, Gemini CLI, Antigravity, and Windsurf. A skill is arbitrary instructions plus
optional scripts, running with your agent's permissions, installed with one command from a
blog post.

The structural gap is progressive disclosure: the agent sees ~100 tokens of description at
session start. The reviewer reads `SKILL.md`, sees something benign, installs. The payload
lives in the deep files that only load on trigger, days later, mid-task, unobserved.

Human review of skills is structurally shallower than human review of code. This tool
closes that gap mechanically.

## The check that matters

```
$ skillmap ci
✗ example-skill  capability escalation vs skillmap.lock
    + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
      reads ~/.aws/credentials — added in this update
    ~ content changed  bc0ce45d → 5b337214
$ echo $?
1
```

That is a real run, against `fixtures/projects/v1.1` — a skill whose v1.0 read only project
files and whose v1.1 also reads `~/.aws/credentials`. `crates/skillmap-cli/tests/escalation.rs`
asserts every part of it, including that the report fits in eight lines.

Everything else in this repository exists to make that line trustworthy.

### Using it

```bash
npm install --save-dev skillmap
```

```bash
skillmap lock    # record what the skills in this project can do today; commit it
skillmap ci      # fail when that changes
skillmap scan    # print the capability manifest as JSON
skillmap rules   # list what this build can detect
```

Exit codes: `0` clean, `1` escalation vs the lock, `2` a capability `policy.toml` does not
permit, `3` both, `4` the check could not run. `4` is separate on purpose — *"could not run"*
must never read as *"ran and found nothing"*. Full format and semantics:
[`docs/06-policy-and-lock.md`](docs/06-policy-and-lock.md). The GitHub Action is
[`action.yml`](action.yml).

skillmap ships two skills of its own, and CI runs `skillmap ci` against them on every push
with the committed [`skillmap.lock`](skillmap.lock) and [`policy.toml`](policy.toml). A tool
that gates other people's repositories and not its own is making an untested claim.

### Installing it

| Channel | Command |
|---|---|
| npm | `npm install --save-dev skillmap` |
| GitHub Action | `uses: skillmap/skillmap@v1` — see [`action.yml`](action.yml) |
| Homebrew | `brew install skillmap/skillmap/skillmap` — formula ships with each release; the tap repo does not exist yet |
| Source | `cargo install --git https://github.com/skillmap/skillmap skillmap-cli` |

**No `postinstall` script anywhere.** The npm package holds a Node shim and no binary; each
platform's binary is its own package resolved through `optionalDependencies`, so the bytes
arrive with the same integrity hashes as any other dependency. A postinstall that downloads
a binary is arbitrary code fetching an arbitrary payload over a channel npm does not verify —
which is a fair description of the thing this project exists to find in other people's
repositories.

**Builds are reproducible and the releases are attested.** Two builds of the same commit from
different directories are byte-identical; the release workflow proves it before publishing
anything, and refuses to publish a binary containing the path of the machine that built it.
Verify a download with `gh attestation verify skillmap-linux-x64.tar.gz --repo skillmap/skillmap`,
or rebuild the tag yourself and compare — [`docs/07-distribution.md`](docs/07-distribution.md)
has the exact steps, and the two bugs that had to be fixed to make the claim true.

## Measured

Corpus snapshot `2026-08`, produced by skillmap at commit `242ecac`.
Full report: [`corpus/report.md`](corpus/report.md).

**34,284 distinct bundles** from 202 public repositories, deduplicated by content digest.
881 came from curated sources — the `anthropics/skills` baseline and an awesome-list — and
33,403 from GitHub code search, which is the only source that reaches the ecosystem's tail.

The two populations are not alike, and that gap is the finding:

| Measured, exactly | Curated head | Tail | Ratio |
|---|---|---|---|
| Ships executable scripts | 2.2% | **10.4%** | 4.7× |
| Has files nothing references | 2.2% | **30.7%** | 14× |
| Mentions a credential path¹ | 0.4% | **9.1%** | 23× |
| Mentions a secret-bearing env var¹ | 0.9% | **17.5%** | 19× |
| Mentions `eval` / `exec` / subprocess¹ | 4.8% | **25.1%** | 5× |

Denominators are 881 (head) and 33,403 (tail). Anyone sampling only curated lists — which is
what most writing about this ecosystem does — would conclude the risk was theoretical.

**The progressive-disclosure gap.** The median bundle shows an agent **2.09%** of its bytes
at session start; 32.4% show under 1%. Across the corpus, **1.17 GB of 1.63 GB (72%) sits in
files nothing points at.** That asymmetry is the whole reason this project exists.

**The lead worth chasing.** 1.6% of bundles mention a credential path *only* in files no
documented path reaches — the disclosure-delta shape, and the starting list for labelling.

¹ Lexical: substring matches, not analysis. They do not parse, establish no reachability, and
carry no provenance, so they are **upper bounds** and never appear in a manifest. `corpus/report.md`
labels every one.

### Measured against ground truth — provisionally

The labelling pass has started. `corpus/sample.json` draws **130 bundles** by a seeded,
stratified sample; `corpus/labels.toml` carries the ground truth so far, produced by reading
each bundle's source without consulting skillmap's output. **41 bundles are labelled and
scored, 9 were too large to read, and 80 are not yet labelled** — and unlabelled is reported
as unlabelled, never folded into a denominator as though it had been checked.

At n=41 the intervals are still wide, which is the honest reading and the reason every rate
carries one:

| Metric | Result |
|---|---|
| `fs.read.credential` precision | 4/4 (100%, 95% CI 51.0–100%) |
| `fs.read.credential` recall | 4/7 (57.1%, 95% CI 25.0–84.2%) |
| False positives, `code_clean` (headline) | 0/17 (0%, **95% CI 0–18.4%**) |
| Bundles with any `unresolved` entry | 39/41 (95.1%, 95% CI 83.9–98.7%) |
| Real disclosure delta | 3/41 — **not poolable**, see below |

**These are not yet quality numbers.** A 95% upper bound of 18.4% on the headline metric means
the sample still cannot distinguish a good scanner from a mediocre one. They are published
anyway, because the alternative — publishing nothing while the tool ships — is what the
numbers exist to prevent. The labels are **single-annotator and unreviewed**; inter-annotator
agreement is unmeasured.

**Every current miss is the acknowledged kind**, and the recall number alone cannot say so:
the scanner reports no capability, but it is not silent — it emits `unresolved: computed_target`
on the exact line, saying it saw a read whose path it could not resolve. A miss the reader can
see is categorically different from one they cannot. That was not true two commits ago; see
the third defect below.

### What forty-one bundles found

Three defects, one of them mine.

- **`sh.credential-read.dotfile` reported `cat > .env` — a *write* — as a read.**
  tree-sitter-bash parses `<`, `>` and `>>` to one `file_redirect` node and the query never
  said which it wanted, so every setup script that generates a `.env` was flagged.
  **Fixed** in the query, with fixtures both ways.
- **Nothing detected `dotenv` at all.** Three of the first four real credential reads were
  `load_dotenv()` or `require('dotenv').config()` — which is how this ecosystem actually reads
  credentials, as opposed to the `open("~/.aws/credentials")` shape the rules were written
  for. **Fixed**: three new rules, python/javascript/typescript, which took a `.toml` and a
  `.scm` and no Rust at all. Recall went 1/4 → 3/4 with no new false positives.
- **One labelling error, mine.** A bundle I labelled clean also runs `grep -q "^${var}=" .env`,
  which reads the file. The scanner was right. Corrected in place with the reasoning kept,
  because on a sample this size it is the most concrete evidence available that
  `reviewed_by` being empty is a real weakness rather than a formality.

- **A silent miss in JavaScript and TypeScript, from import style alone.** The literal form
  of the credential-read query had both a `fs.readFileSync(…)` and a bare `readFileSync(…)`
  branch. The *computed* form had only the first. So `fs.readFileSync(CONFIG_FILE)` reported
  `unresolved: computed_target` and `readFileSync(CONFIG_FILE)` reported **nothing at all** —
  the same read, the same language, differing only by whether the import was destructured,
  which is the modern idiom. Python had both branches from the start. **Fixed**, with fixtures.
  This is the one class of defect this project is least willing to ship: not a missed
  detection, but a missed detection that says nothing.

**More credential-read shapes, still uncovered: the agent's own config file, and per-skill
config directories.** One bundle
opens `~/.openclaw/openclaw.json` and parses it — and a *different* bundle in the same sample
tells users to put their API key in exactly that file. A wallet skill reads its API key from
`~/.config/solana-skill/config.json`. The prefix lists name `~/.aws`, `~/.ssh`, `.env` and
nothing agent-shaped or `~/.config`-shaped. The taxonomy has `fs.write.agent_config` and no
read counterpart at all. All open, because the path set should come from the corpus rather
than from another guess.

Note what every one of these misses has in common: **the path is computed**, not a string
literal. Real code builds credential paths from `homedir()` and constants. The rules were
written against literals.

This is the argument for doing the pass at all: every one of these was invisible to a test
suite written by the same person who wrote the rules, and every one turned up in the first
thirty-eight bundles of someone else's code.

**One thing the strata do not mean.** `code_clean` is the stratum with no credential-shaped
lexical marker, and one bundle in it reads the user's entire WeChat message history from local
SQLite databases. That is correctly not a `fs.read.credential` — chat history is not a
credential — and the term that fits, `fs.read.outside_bundle`, has no rule and is not scored.
So "benign stratum" means *no credential marker*, not *harmless*. The false-positive rate
measured over it is still the right headline; the name is not a claim about sensitivity.

### The disclosure delta: two reasons the cut criterion is still unanswerable

`docs/04-semantic-layer.md` says to **cut the semantic layer** if the labelled corpus shows the
disclosure delta in under ~3% of bundles. Three of 41 labelled bundles have one. That is 7.3%,
and quoting it would be wrong twice over.

**First, the sample is not proportional, so the rows do not pool.** It over-samples the
interesting strata on purpose:

| Stratum | delta | share of population |
|---|---|---|
| `code_clean` | 0/17 | 21% |
| `code_credential` | 0/11 | 22% |
| `code_other_marker` | **1/3** | **46%** |
| `disclosure_shape` | 2/10 | 11% |

A corpus-wide rate needs these weighted — and it is then dominated by `code_other_marker`,
which carries nearly half the population and which I have labelled **three** bundles of. Its
interval is 6–79%. The estimate is currently hostage to the least-sampled row, which makes
that stratum the obvious place to spend the next batch. The eval report prints these weights
beside every rate so nobody pools them by accident.

**Second, the threshold is unset.** Of the three deltas, one is a benign counter file behind a
description reading "Development skill from everything-claude-code"; one is a CSS-animation
generator that calls a hosted model with an API key its description never mentions; one is a
Microsoft 365 server with **no frontmatter at all** — no description, so nothing is disclosed
at session start — which loads `.env`, exchanges a client secret, and reads mail, calendar and
files. A stricter reading counting only the last two gives 2/41.

Both readings now sit above 3%, which is a change from the previous batch, and neither is an
estimate of the corpus. The labels record per-bundle reasoning so the rate can be recomputed
under either.

**A distinction this corpus needed a word for:** *disclosed to the reviewer* is not *disclosed
to the agent*. The animation generator names its API key requirement at line 67 of a 79-line
SKILL.md. A reviewer who opens the file learns it; the agent, which sees ~100 tokens of
description at session start, does not. Both of the repository's definitions take the
description as the baseline, so it counts — and the line number is recorded so a stricter
reading can be applied without re-reading the bundle.
