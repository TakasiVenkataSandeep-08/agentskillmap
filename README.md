# agentskillmap

> The project is **agentskillmap**; the command it installs is **`skillmap`**. The longer name
> is what was available — `skillmap` is blocked on npm by an existing `skill-map` package and
> the `skillmap` GitHub organisation was taken in 2020 — and the short one is what you type.

## A measured corpus of the agent-skill ecosystem, and a differ built on it

The primary artifact here is **measurement**. `SKILL.md` skills are installed with one command
and run with your agent's permissions, and almost nothing published about them carries a
denominator. This repository harvested **34,284 distinct bundles** (snapshot `2026-08`,
deduplicated by content digest) and reports what is actually in them:

```
  ship an executable script            10.2%      carry files no documented path reaches   30.0%
  ship no parseable code at all        89.8%      frontmatter parses (head / tail)   100% / 85.7%

  prose directing fetch-then-execute     249 bundles  (0.73%)
  prose directing a write outside
    the bundle                           576 of the 10,660 prose-only bundles
                                                    that carry a code fence  (5.4%)
```

**275 bundles were then hand-labelled across 8 strata** — 252 scored, 23 too large to read
within one label's budget — and every detection rate below is computed against them and
published with a Wilson interval, including the ones that look bad. A second annotator
independently re-labelled 23 of the first 92: **18/23 agreement, and all five disagreements
went against the first annotator.** That is published too, because a single-annotator corpus is
a weaker artifact than a reviewed one and the reader has to be able to tell.

Nothing here is a base rate someone eyeballed. `corpus/report.md` carries the harvest with its
sampling bias stated before the findings; `corpus/labels.toml` carries the ground truth with a
note per bundle; `corpus/sample.json` carries the provenance so any label can be traced to the
exact bytes it describes.

### The tool that came out of it

**A capability differ for AI agent skills.** It records what a skill can do, with byte-level
evidence, and tells you when that changes.

Think `npm audit` for the folders your agent runs, not a scanner. The value is the **diff**: a
skill you approved last month quietly starting to read `~/.aws/credentials` is the thing this
catches, and the thing a pull request full of prompt edits will not show you.

**What it is not.** Not a linter, not a risk scorer, not a malware classifier, and **not an
auditor** — a clean report is not an assurance. It emits a manifest; your `policy.toml` decides
what is acceptable, and it never uses the words "safe", "malicious" or "severity" — a test
greps the output to keep it that way.

### The denominators, before the good numbers

Precision is 113/113 with zero false positives, and quoting that alone would be misleading.
Read these first:

- **Every capability figure describes 14.6% of the corpus.** The code plane can only fire on
  bundles shipping a file in Python, shell, JavaScript or TypeScript. The other 85.4% is prose,
  where two measured instruction signals now reach one shape each and three more have a firing
  rate and no recall at all.
- **84% of scanned bundles carry at least one `unresolved` entry**, about 4.5 computed targets
  apiece, and **40% of reported capabilities are `present` rather than `observed`** — the code
  is there and nothing established that it runs. "Zero false positives" and "the analysis was
  incomplete almost every time" are both true.
- **Recall runs 44% to 92%** on the terms with a usable sample. Three more read 100% at n=1 and
  n=2; those are decoration and the table says so.
- **A third of the harvest was never eligible for sampling at all** — 10,318 bundles carry a
  lexical marker with no parseable code, fall into no stratum, and are measured by nothing.

A determined author evades this easily, and the known limits in `docs/00-tasks.md` double as an
evasion guide. That is the cost of publishing them, and they are published anyway.

## Why

`SKILL.md` is an open standard read by Claude Code, Claude.ai, the Anthropic API, Codex,
Cursor, Gemini CLI, Antigravity, and Windsurf. A skill is arbitrary instructions plus
optional scripts, running with your agent's permissions, installed with one command from a
blog post.

The structural gap is progressive disclosure: the agent sees ~100 tokens of description at
session start. The reviewer reads `SKILL.md`, sees something benign, installs. The payload
lives in the deep files that only load on trigger, days later, mid-task, unobserved.

Human review of skills is structurally shallower than human review of code. This tool narrows
that gap mechanically — it does not close it, and the measured numbers below are the honest
account of how far it gets.

### Who this is for

**Someone operating a skill registry or marketplace.** The harvest found `community/` with
6,558 bundles, `SkillBank/` with 5,591, `composio-skills/` with 832. Thousands of third-party
submissions, no realistic human review capacity, and a direct interest in knowing what each one
can do before listing it. Recall matters less at that scale — 44% on one term still surfaces
hundreds of real findings — and precision is what decides whether the output is usable at all.
It is offline, deterministic and has no telemetry, so batch-scanning someone else's code is not
itself a disclosure.

**Someone who installs skills and wants to know when they change.** Point it at
`~/.claude/skills`, lock, and re-run. This is the drift case, and it is where a mediocre recall
still buys you something: you are watching for *changes* in shapes that are covered, not
enumerating everything that exists.

**Someone who needs numbers about this ecosystem.** Security researchers and vendors writing
about agent-skill supply chains, and anyone arguing about how large the problem is. The harvest,
the strata, the 275 labels and the per-term rates are all in-repo and reproducible: the report
states its sampling bias before its findings, the labels carry a note per bundle, and the sample
carries provenance so any claim can be traced to bytes. Cite it, disagree with it, or re-run it —
`GITHUB_TOKEN=… cargo run -p skillmap-corpus -- --snapshot 2026-08`. This is the part of the
project that does not depend on the scanner being good.

**Not yet for:** deciding whether an unknown skill is safe to install. A clean report from a
tool with this recall is not evidence of much, and a determined author evades it easily. The
known limits are catalogued in `docs/00-tasks.md` — unsupported languages, wrappers the engine
cannot follow, paths computed at runtime — and that list doubles as an evasion guide, which is
why it is published rather than buried.

## The check that matters

```
$ skillmap ci --scope user
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

**`--scope user` is the interesting half**, and the corpus is why. Of 34,284 harvested bundles
only **9% sit in a project's own agent directory**; the rest are published rather than
consumed. A project skill is committed, so somebody was already going to review that diff.
A user skill under `~/.claude/skills` is installed with one command, applies to every project
on the machine, and is never looked at again.

Everything else in this repository exists to make that line trustworthy.

### Using it

```bash
npm install --save-dev agentskillmap
```

```bash
skillmap lock    # record what the skills in this project can do today; commit it
skillmap ci      # fail when that changes
skillmap scan                   # a canonical JSON array of manifests
skillmap scan --format human    # a few lines per skill, for reading
skillmap hook install           # run the user-scope check at every session start
skillmap rules   # list what this build can detect
```

**Two places skills live, and the second is the one nobody watches.**

```bash
skillmap lock --scope user   # ~/.claude/skills — applies to EVERY project
skillmap ci   --scope user
```

A project-scope skill is committed, so a pull request already shows it changed —
skillmap tells you what that change *means*. A user-scope skill is installed with
one command, applies everywhere, and is never reviewed again. Of 34,284 harvested
bundles only **9% sit in a project's own agent directory**; most consumption is
the other kind.

The user lock goes to `~/.skillmap/user.lock`, **never into the repository**:
`~/.claude/skills` is a different set on every machine, so a committed lock of it
would fail for everyone except whoever generated it. The policy file follows the
same rule — a machine-wide check must not change its answer depending on which
directory you ran it from.

A CI runner has no `~/.claude/skills`, so `--scope user` there finds nothing and
exits 0. Every run prints the bundle count it looked at, so that zero is visible
rather than silent. It is a local check, not a gate.

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
| npm | `npm install --save-dev agentskillmap` — the package is `agentskillmap`, the command it installs is `skillmap`; not published yet |
| GitHub Action | `uses: TakasiVenkataSandeep-08/agentskillmap@v1` — see [`action.yml`](action.yml) |
| Homebrew | `brew install TakasiVenkataSandeep-08/agentskillmap/skillmap` — **live**; formula regenerated with fresh checksums on every release |
| Source | `cargo install --git https://github.com/TakasiVenkataSandeep-08/agentskillmap skillmap-cli` |

**No `postinstall` script anywhere.** The npm package holds a Node shim and no binary; each
platform's binary is its own package resolved through `optionalDependencies`, so the bytes
arrive with the same integrity hashes as any other dependency. A postinstall that downloads
a binary is arbitrary code fetching an arbitrary payload over a channel npm does not verify —
which is a fair description of the thing this project exists to find in other people's
repositories.

**Builds are reproducible and the releases are attested.** Two builds of the same commit from
different directories are byte-identical; the release workflow proves it before publishing
anything, and refuses to publish a binary containing the path of the machine that built it.
Verify a download with `gh attestation verify skillmap-linux-x64.tar.gz --repo TakasiVenkataSandeep-08/agentskillmap`,
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
each bundle's source without consulting skillmap's output. **every code-bearing stratum is
labelled completely** — `code_clean` 40/40, `code_credential` 40/40, `code_other_marker` 15/15,
`disclosure_shape` 20/20. 92 bundles scored, 23 too large to read, and `prose_only` (15)
deliberately unlabelled: those bundles contain no supported-language file by construction, so
a label there would record the stratum definition rather than a reading — and unlabelled is reported
as unlabelled, never folded into a denominator as though it had been checked.

At n=92 the intervals are meaningful for the first time, which is the honest reading and the reason every rate
carries one:

| Metric | Result |
|---|---|
| `fs.read.credential` precision | 13/13 (100%, 95% CI 77.2–100%) |
| `fs.read.credential` recall | **13/18 (72.2%, 95% CI 49.1–87.5%)** |
| `net.egress` precision | 45/45 (100%, 95% CI 92.1–100%) |
| `net.egress` recall | **45/49 (91.8%, 95% CI 80.8–96.8%)** |
| `env.read.secret` precision | 23/23 (100%, 95% CI 85.7–100%) |
| `env.read.secret` recall | **23/28 (82.1%, 95% CI 64.4–92.1%)** |
| `process.exec` precision | 3/3 (100%, 95% CI 43.9–100%) |
| `process.exec` recall | 3/5 (60.0%, 95% CI 23.1–88.2%) — n too small to read |
| `process.exec.dynamic` precision | 2/2 (100%, 95% CI 34.2–100%) |
| `process.exec.dynamic` recall | 2/2 (100.0%, 95% CI 34.2–100%) — n far too small to read |
| `code.dynamic_eval` precision | 1/1 (100%, 95% CI 20.7–100%) |
| `code.dynamic_eval` recall | 1/1 (100.0%, 95% CI 20.7–100%) — n=1; see below |
| `fs.read.outside_bundle` precision | 12/12 (100%, 95% CI 75.8–100%) |
| `fs.read.outside_bundle` recall | 12/27 (44.4%, 95% CI 27.6–62.7%) |
| `fs.write.outside_bundle` precision | 14/14 (100%, 95% CI 78.5–100%) |
| `fs.write.outside_bundle` recall | 14/25 (56.0%, 95% CI 37.1–73.3%) |
| False positives, `code_clean` (headline) | **0/36 (0%, 95% CI 0–9.6%)** |
| Bundles with any `unresolved` entry | 91/92 (98.9%, 95% CI 94.1–99.8%) |
| Real disclosure delta | **12.9% weighted** (95% CI 2.6–23.3%), see below |

**Precision is 113/113 across all eight scored terms and the false-positive rate is 0 across all
four code strata** — 92 bundles, not one spurious capability, on a rule set that now fires on
half of them. The benign stratum's 95% upper bound is **9.6%**.

That last part is what a broad rule puts at risk. `net.egress` fires on 45 of 92 bundles;
a rule that common is exactly how a benign stratum gets lit up, and `code_clean` held at 0/36.

**Those figures are the code plane only.** Capabilities and instruction signals live in
different fields of the manifest, and every number above iterates `capabilities`. Read the
`0/36` as *no spurious capability*, not as *nothing fired*.

#### The instruction plane, measured separately

The `instruction.*` signals are `tier = "pattern"` — prose regex, the weakest tier. T13
hand-labelled 156 bundles across four strata to find out whether the three that had never
been measured were worth keeping. **Two were not, and were removed at schema 1.4.0.**

| Signal | Precision | Recall | Status |
|---|---|---|---|
| `instruction.exec_directive` | 31/31 (100%) | 31/35 (88.6%) | shipped |
| `instruction.directs_outside_write` | 37/38 (97.4%) | 37/37 (100%) | shipped |
| `instruction.fetch_as_instruction` | **26/26 held out** | 26/29 (89.7%) | rewritten at 1.4.0 |
| `instruction.config_mutation` | 21/32 (65.6%) | 21/48 (43.8%) | **held, not repaired** |
| ~~`instruction.exfil`~~ | 2/36 (5.6%) | 2/12 | **withdrawn at 1.4.0** |
| ~~`instruction.silence`~~ | — | — | **withdrawn: never had a rule** |
| ~~`instruction.privilege_claim`~~ | — | — | **withdrawn: never had a rule** |

`exfil` was withdrawn because its failure is not tunable: in this corpus `send` and
`transfer` usually mean moving crypto tokens, and the largest group of false positives is
prose *forbidding* the behaviour. Two repairs were measured — qualifying the noun gave 1/30,
adding a negation guard gave 0/7, removing both true positives along with 23 false ones.

`fetch_as_instruction` now detects one thing: **the bundle's operative instructions are not
in the bundle.** Its precision column is the held-out figure — 30 bundles drawn and read
after the rule was written, because the rule was rewritten *after* its other strata were
labelled and those are in-sample for it. In-sample recall is 13/26, and that gap is published
beside the flattering number rather than under it.

`config_mutation` is measured and deliberately **not** shipped repaired. A repair closes half
its misses (recall 43.8% → 79%) and leaves precision at ~64%, and what remains is unreachable
by any pattern: security scanners enumerating what they detect, git-hooks skills where the
word means something else entirely, and two cases where the rule is arguably right and the
definition too narrow. 64% is far below the bar the two shipped signals set, so it stays as it
is with the evidence recorded.

**The last column is why this is a firing rate and not a false-positive rate.** Every firing
was read. The three from `directs_outside_write` each install a skill into an agent workspace
directory and are genuine detections — `code_clean` means *no credential marker*, not
*harmless*, and these bundles were never read for instruction signals, so their empty
`capabilities` arrays say nothing about this plane.

**Two of the five have real ground truth.** Both got it the same way, and it is the only way
that works here: a stratum drawn for the signal and hand-labelled **before** the rule was
written.

| Signal | Precision | Recall |
|---|---|---|
| `instruction.exec_directive` | **31/31 (100%, 95% CI 89.0–100%)** | **31/35 (88.6%, 95% CI 74.0–95.5%)** |
| `instruction.directs_outside_write` | **37/38 (97.4%, 95% CI 86.5–99.5%)** | **37/37 (100%, 95% CI 90.6–100%)** |

`instruction.directs_outside_write` reports prose directing the agent to write to, copy into,
or make executable a path outside the bundle — a shell profile, a config directory, a `PATH`
bin directory, the agent's own skills directory. It exists because **89.8% of harvested
bundles ship no parseable file at all**, and a third of those carry runnable code in fenced
blocks that nothing looked at. Measured over 80 prose-only bundles across two strata.

Its single false positive is a copy annotated `WRONG` in a section demonstrating a common
mistake: prose *about* the shape, matched *as* the shape. A `pattern`-tier rule cannot tell
those apart, which is the tier's definition rather than a defect.

**Three shapes were proposed for this and withdrawn before a label was written** — directing
egress, credential access, and subprocess spawning. In a prose-only bundle the dominant genre
is reference material, so a network call inside a code sample is documentation; at 23–26% base
rates with no contextual separator they were noise generators. Requiring an operative heading
was measured as a rescue and failed, at 30% of the *control* stratum against 25–40% of the
positives. What survives is a shape that carries its own intent: reference material
demonstrates logic and never mutates the reader's machine as an illustration.

**A fifth of that stratum is one shape worth knowing about.** Nine of forty positives, from
nine different publishers, are a documented setup step that fetches *another skill* from a
vendor URL straight into the agent's own skills directory. The fetched bytes are never
reviewable by reading the bundle, and the destination is what the agent loads from on every
later session.

`instruction.exec_directive` reports prose directing the agent to fetch remote content and
execute it — `curl … | sh`, or fetching a `.sh`/`.py` and running it.

Each signal is scored over its own two strata and no others. The strata drawn for one signal
were never read for the other, so a firing outside them is unmeasured rather than wrong, and
counting it either way would invent a number.

**This closed the largest detection gap in the tool.** A payload in a fenced code block
inside `SKILL.md` was invisible to every plane, while the identical bytes in a script file
were caught cleanly — and a fence is how the documented marketplace-poisoning campaign
delivered its payload. It is reported as an instruction, never as a capability: the prose
directs execution, the bundle's own code does not perform it.

**It is not a verdict, and the corpus is emphatic.** Of 40 bundles drawn for this shape, 35
carry it and nearly all are ordinary installer instructions for real tools. The same shape
spans a bare pipe into `sudo bash`, a fetch paged with `less` before running, and a fetch
whose SHA-256 is verified first. All execute remote code; which of them a repository
tolerates is `policy.toml`'s question.

**A known evasion, confirmed by construction rather than theorised.** A closing fence
delimiter carrying an info string shifts the pairing of every fence after it, so a later
directive lands inside a block the grammar reads as having no language, and the rule goes
silent. The identical command in a correctly paired document fires. An agent reading the
prose is unaffected, because it never parses fences. One of the four misses is this; the
other three split the fetch and the execution across lines, or omit the URL scheme.

**Both firings were read and both are false positives.** One is a test-results document whose
recommendations list points its human maintainer at `AGENTS.md` as somewhere a daily-routine
step might live — a roadmap item, not a skill rewriting agent config. The other is a
network-DLP skill whose threat-model section warns that a compromised skill can POST workspace
contents to an external server: prose *against* the behaviour, which is the failure mode that
rule's own `false_positive_notes` predicted and which its negative fixture — this repository's
threat-model text — guards.

**Writing that paragraph tripped the rule.** The first draft of the sentence above described
the false positive in the same grammar the rule matches, and
`no_instruction_rule_fires_on_this_repositorys_own_documentation` failed the build until it
was rephrased. That is better evidence than either corpus firing: a `pattern`-tier rule cannot
distinguish describing a behaviour from instructing it, because the tier is a regex over prose
and nothing more. It is the definition of the tier, not a defect to be narrowed away — which
is why these findings are quarantined from `capabilities` and why the manifest reports the
sentence and its byte range rather than a verdict.

**The other three signals have no recall number, and that is a real gap rather than an
oversight.** Across all 92 originally-labelled bundles they fired twice in total, and both
firings were read and judged wrong — so those three have **zero adjudicated true positives**
on that corpus. No annotator judged the prose there, so `capabilities = []` means "not looked
for" with respect to `instruction.*`, never "not present", and a rate against those labels
would book every genuine detection as a false positive. None is computed.

The contrast with the two measured signals is the argument for how the other three should be
finished: draw a stratum for the signal, label it before writing the rule, and a real
precision and recall follow. Both measured signals got there that way, and the second one
also shows the cost of getting the term wrong first — three candidate shapes were drawn for,
found to be reference material rather than instruction, and withdrawn before a label was
written. Roughly two days per signal when the term is right, and it is the difference between
"quiet" and "measured".

`crates/skillmap-eval/tests/instruction_stratum.rs` records the adjudicated counts, fails when
a rule widens without someone reading the new hits, and fails when a signal ships with no
adjudicated entry at all — so the next rule in this plane cannot arrive unmeasured.

**Eight terms now have ground truth, and every one of them has a rule.** A second reading pass
over all 92 bundles hunted for `net.egress`, `env.read.secret`, `process.exec`,
`process.exec.dynamic` and `code.dynamic_eval`; a third added the two `outside_bundle` terms.
The denominators are the finding:

```
  net.egress               49/92 bundles     env.read.secret          28/92
  fs.read.outside_bundle   27/92             fs.write.outside_bundle  25/92
  fs.read.credential       18/92             process.exec              5/92
  process.exec.dynamic      2/92             code.dynamic_eval         1/92
```

Three more terms ship **declared unmeasured** — `code.obfuscation`,
`net.fetch_then_execute` and `fs.write.agent_config`. Each is a chain judgement or too rare
for a rate, each is named in `corpus/labels.toml`, and the eval prints their bundle counts
directly above the false-positive block they are excluded from. A rule cannot ship for a term
in neither list: `crates/skillmap-eval/tests/gate.rs` fails the build.

**`net.egress` is in 53% of the labelled corpus — nearly three times `fs.read.credential`.**
It went from recall 0/49 to **45/49 (91.8%)** at precision 45/45, with the benign stratum
unmoved throughout.

**The last third of that came from detecting vendor SDKs**, which the labelling pass measured
as the most common egress mechanism in the corpus and the least visible: `openai.chat.
completions.create(...)` reaches a hosted API without the word http appearing anywhere in the
call. The receiver is a local name bound to a constructor elsewhere in the file, and the engine
cannot follow a receiver to its constructor — so the **method chain** is the evidence instead.
That is safe here and would be reckless for `.get(` or `.fetch(`: `chat.completions.create` is
a shape nothing but an LLM client has reason to write, and it is matched three levels deep
precisely because `.create(` alone is an ORM method.

Four misses remain, and three of them are declined rather than unsolved: `Linkedin(...)`,
`enable_remote_sync(...)` and `new Imap(...)` each appear in exactly one bundle, and naming
them would raise recall while lowering what the number means — the same call made about
`.beanstalk` and `.fluxa-ai-wallet-mcp`. The fourth is a wrapper that renames the call
(`proxyFetch(url)`), which needs the interprocedural analysis this engine does not have.

**The two `outside_bundle` terms have low recall on purpose, and that is what makes them
honest.** 37.0% and 36.0%, at perfect precision. For these two the rule's `[match]` data is
almost the *definition* of the term rather than a list of interesting names — so labelling by
"does this path start with `/` or `~/`" would have made precision 1.0 by construction and
measured nothing at all. The labels judge the **act** instead, including paths a reader can see
are outside and the engine cannot resolve: a directory read from config, a base path passed as
an argument, a variable assigned twice. Those are the misses, and they say something true —
real skills build paths at runtime, and constant folding reaches about a third of them.

**Measuring them first is the reason they are shippable.** Their first measured run scored
precision 66.7% and 52.4%, and took the benign stratum from 0/36 to 4/36. Two causes: eight of
the thirteen false positives were `>/dev/null`, which discards output rather than writing a
file; and a latent bug in the engine's literal extractor, which strips leading letters to reach
past python's `r"`/`b"` prefixes and was applying that to *unquoted* values too — so a shell
`cat templates/default.toml` resolved to `/default.toml`, a bundle-relative read wearing an
absolute path. Harmless until a rule filtered on a leading `/`. A mangled path is worse than an
unresolved one: it is a confident wrong answer sitting in `detail.paths` where a reader takes it
for evidence.

**`code.dynamic_eval` has a denominator of one, and 1/1 is not a result.** Its 95% interval runs
20.7%–100%. What *is* worth reading is the other direction: the rule fired on exactly one of 92
bundles and that one was correct, so the false-positive rate is a real measurement even though
the recall is not. The term is carried by the fixture and adversarial suites, not by the corpus.

The rule that made it shippable is a subtraction. **`.eval(` is PyTorch's mode switch** —
`model.eval()` means *stop training*, and appears across a large fraction of ML-adjacent
skills — while `.exec()` is a method on database cursors and on every compiled regular
expression in JavaScript. So there is no member form at all: bare `eval`/`exec`/`compile`,
`new Function`, `vm.run*`, and `setTimeout` **only with a string first argument**. This project
has now met that trap once per method name — `.get(`, `.fetch(`, `.exec(` — and the negative
fixtures carry all three.

**`env.read.secret`'s name regex was tuned against the labels, not invented.** Every
environment read in the 92 bundles was extracted and split by whether its bundle carries the
term. Against that set: **28 of 28 secret-bearing names match and 0 of 38 non-secret names do**
— `CACHE_KEY`, `PRIMARY_KEY`, `SORT_KEY`, `MAX_TOKENS`, `TOKENIZER`, `CLIENT_ID` and
`TENANT_ID` all held out, and a bare `_KEY$` deliberately absent because every cache and every
database row has one. That audit is what the labels are *for*: they were made by judging names,
before the regex existed, so the regex can be checked against them rather than the reverse.

**Reading the environment is not writing it.** `process.env.X` and `process.env.X = v` are the
same node differing only in which field of the parent they occupy, and tree-sitter has no
negation. So every pattern anchors on a *read* context. The alternative was reporting a
hand-rolled `.env` loader — which **sets** credentials — as a reader of them, and two corpus
bundles load `.env` exactly that way. This repository already shipped that error once in the
other plane, where `cat > .env` was reported as a credential read; a dedicated test now pins
both directions.

**`process.exec` and `process.exec.dynamic` ship at n=5 and n=2, and those numbers cannot
carry an argument.** 3/5 and 2/2 with perfect precision looks good and means very little — the
95% interval on 3/5 runs from 23% to 88%. They are published because the bar for scoring a term
is "looked for exhaustively", not "has a usable sample", and suppressing the terms where the
honest answer is *we looked and there is almost nothing here* would be the more misleading
choice. The fixture and adversarial suites carry these two terms; the corpus does not.

**Building those two rules corrected them twice.** First, an unconstrained `.exec(` fired on
`pattern.exec(text)` — a regular expression — and on any object with an `exec` method. Same
trap as `.get(` and `.fetch(`, caught by the negative fixture. Second, and more interesting:
`subprocess.run(cmd)` was reported as *dynamic* until three labelled bundles showed `cmd` was a
single-assignment literal list one line up (`cmd = ["ffmpeg", "-i", src]`). argv[0] is `ffmpeg`
and perfectly knowable — the engine can fold that, a tree-sitter query cannot, and it has no way
to tell a name that folds from one that does not. **The rule now declines to claim.** That costs
a real miss, and it is the right direction: asserting a program is unknowable when it is written
three words away is a false statement about what a reader can see.

**The labelling deliberately landed before any rule for these terms.** Widening the scored set
while a rule already fires would score every genuine detection as a false positive, because an
empty `capabilities` array means "not looked for", not "not present".
`crates/skillmap-eval/tests/gate.rs` makes shipping a rule for an unmeasured term a build
failure, so the ordering is enforced rather than remembered.

**And the first rule immediately caught a labelling error.** `net.egress` reported a bundle
whose label said no egress — while that label's own note said it *"refreshes the JWT against a
remote API"*. The capability was recorded in prose and never entered as a term. The scanner was
right, the label was wrong, and it is corrected in place with the reasoning kept. That is the
second demonstrated error by the first annotator, both in the same direction, and it is why
the denominator reads 49 rather than 48.

**A second annotator reviewed 23 of the 92 bundles and disagreed with five — winning all
five.** The review was independent: blind to the label file, without running the scanner, on a
seeded 15% control sample plus every judgement the first pass had flagged as contested. Raw
bundle-level agreement was **18/23 (78.3%)**, and every adjudicated disagreement went against
the first annotator. Three were the same systematic miss — egress through a **vendor SDK or a
wrapper**, where no protocol is named at the call site (`linkedin_api`, a viem `http()`
transport, `enable_remote_sync(auto_start=True)`). Re-sweeping the other 69 bundles for that
pattern found two more, which is why `net.egress` reads 49 and not 43.

So **49 is a floor, not a count.** The method that produced it could not see SDK egress, and
only bundles importing a *named* SDK were re-swept. This is the most concrete evidence the
project has that `reviewed_by` being empty was a real weakness rather than a formality — and
69 bundles still carry that caveat in full.

**Recall is 72.2%**, from 38.9% before the corpus was labelled. The labelling found why it was
low: **every credential read in the corpus reaches its path by computation — not one uses a
string literal**, and the rules were written against literals. Constant folding took it to
61.1%; a third matching mode and a widened `~/.config/` prefix took it to 72.2%. Neither cost
a single false positive.

**Reading all five remaining misses changed what the backlog says.** They had been recorded as
data gaps — paths the rule lists do not name — and only two of them are:

- **Two are data, and deliberately left open.** One reads `<base>/.beanstalk/gateway.json`, one
  `~/.fluxa-ai-wallet-mcp/config.json`. Adding those directory names would close both and catch
  nothing else ever, since each string appears in exactly one bundle. Memorising the corpus
  raises recall and lowers what the number means.
- **Three are not data at all.** Two read a path passed in as a *function parameter*, and one
  takes it from **argv**. The fold is per-expression by design, so the callee genuinely does
  not know the path — and the argv case is not knowable by any static analysis at all. No list
  of paths could have closed them; interprocedural dataflow is the open design question, and it
  now has three examples behind it instead of none.

All five report `unresolved: computed_target` on the exact line. A miss the reader can see is
categorically different from one they cannot, and recall alone cannot tell them apart — which
is why both are published.

**A second annotator was added, and it moved the numbers.** 23 of the 92 bundles — a seeded
15% control plus every contested judgement — were relabelled independently, blind to the first
pass and without running the scanner. Raw agreement was **18/23 (78.3%)**, and all five
disagreements were adjudicated in the *second* annotator's favour.

Three of the five were one systematic miss: **egress through a vendor SDK or a wrapper**, where
no protocol appears in the call — `linkedin_api.Linkedin(...).get_conversations()`, a viem
`http()` transport, `enable_remote_sync(platform_url=..., auto_start=True)`. Sweeping the other
69 bundles for that same shape found two more. `net.egress` went 43 → 48 purely as a result of
being reviewed, which means **48 should be read as a floor**. The remaining 69 bundles carry the
single-annotator caveat in full.

**Every current miss is the acknowledged kind**, and the recall number alone cannot say so:
the scanner reports no capability, but it is not silent — it emits `unresolved: computed_target`
on the exact line, saying it saw a read whose path it could not resolve. A miss the reader can
see is categorically different from one they cannot. That was not true two commits ago; see
the third defect below.

### What ninety-two bundles found

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

- **The bundle this project is shaped around, in a random sample of fourteen.** Description:
  *"A totally legitimate skill that does nothing suspicious."* A file nothing references pipes
  a remote script into `bash`, sends `~/.ssh/id_rsa` through base64 to a collector, and
  downloads and runs a miner. The domains are fictional and the comments say "Exfiltrate some
  data", so it reads as a published demonstration payload rather than a live attack — but it is
  real, it is in the corpus, and it is exactly the shape the tool exists to catch.

  skillmap reports `fs.read.credential` at `maintenance.sh:11` and tags the file
  `unreferenced`. Both correct. It reports **none** of `net.fetch_then_execute`,
  `code.obfuscation`, `process.exec` or `net.egress` — one of four real capabilities — because
  no rule exists for any of them, and all four are terms already in the taxonomy. That is the
  clearest available statement of where this project stands: **the engine and the load-phase
  signal work; the rule set now covers every term the manifest can describe.**

- **Every credential read in the corpus computes its path.** Eight distinct shapes: dotenv the
  library, dotenv hand-rolled twice in two languages, per-application directories under
  `~/.config`, agent config JSON, agent-managed `credentials/` directories, per-tool dotfile
  directories, and a token cache beside the script. The rules matched string literals, so they
  matched almost none of it. **Constant folding took recall from 38.9% to 61.1%, and matching
  by containing directory took it to 72.2% — zero new false positives across 92 labelled
  bundles at each step** — it resolves joins, home-directory lookups and
  single-assignment constants, and reports a *partially* resolved path by filename when the
  location is unknowable. Which filenames matter stays in `rules/*.toml`; invariant 7 is intact.

- **A silent miss in JavaScript and TypeScript, from import style alone.** The literal form
  of the credential-read query had both a `fs.readFileSync(…)` and a bare `readFileSync(…)`
  branch. The *computed* form had only the first. So `fs.readFileSync(CONFIG_FILE)` reported
  `unresolved: computed_target` and `readFileSync(CONFIG_FILE)` reported **nothing at all** —
  the same read, the same language, differing only by whether the import was destructured,
  which is the modern idiom. Python had both branches from the start. **Fixed**, with fixtures.
  This is the one class of defect this project is least willing to ship: not a missed
  detection, but a missed detection that says nothing.

**The largest vocabulary gap: the OS credential store.** One bundle runs
`security find-generic-password -s "Claude Code-credentials" -w` on macOS, and `secret-tool` on
Linux, then greps `accessToken` and `refreshToken` out of the result — the agent's own OAuth
credentials, straight from the keychain. `fs.read.credential` is defined as "a known credential
path or secret-bearing env var"; a keychain is neither, so the manifest has **nothing to say
about it at all**. That is arguably the most direct route to stealing an agent's
authentication.

**Per-skill config directories: covered now, and the corpus decided the shape.** A wallet skill
reads its API key from `~/.config/solana-skill/config.json`; another reads
`~/.config/moltmarkets/credentials.json`. Widening `~/.config/gh/` to `~/.config/` catches both
and, measured across all 92 labelled bundles, costs nothing — a prefix broad enough to worry
about in the abstract and empirically quiet. A third bundle reads
`~/.clawdbot/credentials/homebridge.json`, where the filename is named after the integration and
the *directory* is the only knowable part; that needed a third matching mode, `path_contains`.

**The agent's own config file is still uncovered, for a different reason than assumed.** One
bundle opens `~/.openclaw/openclaw.json` and parses it — and a *different* bundle in the same
sample tells users to put their API key in exactly that file. But it reaches that path from
**argv**, so no prefix list would ever have caught it. What is actually missing is a term: the
taxonomy has `fs.write.agent_config` and no read counterpart, and reading agent config to
harvest the keys inside it is the more direct attack.

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

### The cut criterion: the evidence says do not cut

`docs/04-semantic-layer.md` says to **cut the semantic layer** if the labelled corpus shows the
disclosure delta in under ~3% of bundles.

**First, a confound that had to come out of the number.** 14.6% of the corpus has no
description at all — `SKILL.md` with no frontmatter. Every such bundle is a disclosure delta by
construction, and finding one needs `description_bytes == 0`, not a model. Six of the 56
labelled bundles are in that state; they are now reported separately and excluded, because a
rate that partly counts missing frontmatter must not be read as an argument for a semantic
layer.

**Second, the rows do not pool.** The sample is deliberately not proportional, so a
corpus-wide rate needs population weights. The two strata that carry deltas are now labelled
**completely**:

| Stratum | delta (described bundles) | labelled | share of population |
|---|---|---|---|
| `code_clean` | 0/33 | **complete** | 21% |
| `code_credential` | 0/26 | **complete** | 22% |
| `code_other_marker` | 3/14 | **complete** | **46%** |
| `disclosure_shape` | 3/11 | **complete** | 11% |

**Weighted: 12.9% of the code-bearing corpus, 95% CI 2.6–23.3%.** Four times the threshold,
computed by the harness and suppressed unless every stratum carries at least five labels.

**On current evidence the layer should not be cut.** What still qualifies that: **the
disclosure-delta labels are single-annotator and entirely unreviewed** — the second annotator
judged capability terms only, so none of the six deltas below has been checked by anyone
else — `prose_only` is unlabelled by design, and the interval is a normal
approximation — the thing Wilson exists to avoid at small n. Its lower bound sits at 2.6%, just
under the threshold, so a second annotator disagreeing with two of the six deltas would change
the conclusion.

**But the deltas are not the shape the project expected**, and that matters more than the
number. None is a concealed payload. Every one is a skill whose ~100-token description omits
that it sends data to a third party or writes outside itself — a CSS-animation generator and a
CI generator that call hosted models with API keys their descriptions never mention; a
methodology skill whose install script copies itself into another agent's skills directory; and
a **content-moderation skill that sends the text it is moderating to two external APIs**, with
a description listing five situations to invoke it and never saying the text leaves the
machine.

If that holds up, the semantic layer's value is in **undisclosed egress**, not in concealed
capability — a different prompt and a different eval than `docs/04-semantic-layer.md` describes.

**The distinction that produced all of them:** *disclosed to the reviewer* is not *disclosed to
the agent*. Each names its behaviour somewhere in `SKILL.md`'s body, which a reviewer who opens
the file reads and the agent does not. Both of the repository's definitions take the
description as the baseline, so they count, and every label records the line number so a
stricter reading can be applied without re-reading anything.
