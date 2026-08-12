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
each bundle's source without consulting skillmap's output. **52 bundles are labelled and
scored, 9 were too large to read, and 69 are not yet labelled**. One stratum,
`code_other_marker`, is now labelled **completely** (15/15) — it carries 46% of the population
and was the row the corpus-wide estimate rested on — and unlabelled is reported
as unlabelled, never folded into a denominator as though it had been checked.

At n=52 the intervals are still wide, which is the honest reading and the reason every rate
carries one:

| Metric | Result |
|---|---|
| `fs.read.credential` precision | 4/4 (100%, 95% CI 51.0–100%) |
| `fs.read.credential` recall | 4/8 (50%, 95% CI 21.5–78.5%) |
| False positives, `code_clean` (headline) | 0/17 (0%, **95% CI 0–18.4%**) |
| Bundles with any `unresolved` entry | 50/52 (96.2%, 95% CI 87.0–98.9%) |
| Real disclosure delta | **12.1% weighted** (95% CI 1.8–22.5%), see below |

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

### What fifty-two bundles found

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

### The cut criterion: the evidence now says do not cut

`docs/04-semantic-layer.md` says to **cut the semantic layer** if the labelled corpus shows the
disclosure delta in under ~3% of bundles. This is the first batch that can answer it.

The sample is deliberately not proportional, so per-stratum rows cannot be pooled — a
corpus-wide rate needs them weighted by population. `code_other_marker` carries 46% of that
population and is now labelled **completely**:

| Stratum | delta | labelled | share of population |
|---|---|---|---|
| `code_clean` | 0/17 | partial | 21% |
| `code_credential` | 0/11 | partial | 22% |
| `code_other_marker` | **3/14** | **complete (15/15)** | **46%** |
| `disclosure_shape` | 2/10 | partial | 11% |

**Weighted: 12.1% of the code-bearing corpus, 95% CI 1.8–22.5%.** Computed by the eval harness
and suppressed unless every stratum carries at least five labels. The interval no longer
includes zero.

Under a stricter standard — dropping the one benign delta, a counter file behind a description
reading "Development skill from everything-claude-code" — it is **8.8%**. Both are several
times the threshold.

**So on current evidence the layer should not be cut.** That is a real answer to a question
that has been open since T7, and it is worth naming what still qualifies it: the labels are
single-annotator and unreviewed; four of five strata are partial; the interval is a normal
approximation, which is the thing Wilson exists to avoid at small n. A second annotator
disagreeing with two of the five deltas would change the conclusion, and nothing here prevents
that.

The four non-benign deltas are all the same shape, and it is not the shape the project
expected. None is a hidden payload. Each is a skill whose ~100-token description omits that it
sends data to a third party or writes outside itself:

- a CSS-animation generator that calls a hosted model with an API key;
- a Microsoft 365 server with **no frontmatter at all**, which loads `.env`, exchanges a client
  secret, and reads mail, calendar and files;
- a methodology skill whose install script copies itself into another agent's skills directory;
- a **content-moderation** skill that sends the text it is moderating to two external APIs.

The last is the sharpest: a security skill whose description lists five situations to invoke it
and never says the text leaves the machine. `SKILL.md`'s body names both services. Whether
that trade is acceptable is exactly the judgement `policy.toml` exists to hold — the manifest's
job is to make it visible.

**The distinction that produced four of these:** *disclosed to the reviewer* is not *disclosed
to the agent*. Each names its behaviour somewhere in `SKILL.md`'s body, which a reviewer who
opens the file will read and the agent — seeing ~100 tokens at session start — will not. Both
of the repository's definitions take the description as the baseline, so they count, and every
label records the line number so a stricter reading can be applied without re-reading anything.
