# Corpus scan — snapshot `2026-08`

## Method, and what these numbers do not mean

Read this before the tables. Every number below is a base rate over a **sample**, and the sample is not the ecosystem.

- **34284 distinct bundles**, deduplicated by content digest. A bundle vendored into several repositories counts once, so popular templates do not inflate the rates.
- **Head vs tail is reported separately.** 881 bundles came from sources a human curated (the `anthropics/skills` baseline, awesome-lists, operator-named repositories) and 33403 from GitHub code search. Curated sources measure what people chose to write about; only code search reaches the tail. Pooling them would describe neither population.
- **Code search has a hard ceiling.** GitHub returns at most 10 pages of 100 results, so the tail sample is bounded by the API, not by what exists. Treat tail counts as a floor.
- **Only public repositories** are reachable, and only those whose `SKILL.md` is indexed for code search.
- **Star counts come from the API**, never from secondary sources. Published star figures for this ecosystem disagree with each other and with the API.

### What each source yielded

| Source | Query | Repositories |
|---|---|---|
| `baseline` | `anthropics/skills` | 1 |
| `curated_list` | `ComposioHQ/awesome-claude-skills` | 1 |
| `code_search` | `filename:SKILL.md` | 200 |

### Structural versus lexical

The **structural** tables are exact: they come from the same parser the scanner uses, and say precisely what is in the bundle.

The **lexical** table is not. It counts bundles whose text *contains* a marker substring — a credential path, an `eval(`, a URL. It does not parse, does not establish reachability, and does not distinguish a live call from the same string inside a comment, a docstring, or a warning not to do the thing. **Every lexical number is an upper bound.** They are here because they size the problem cheaply and tell the rule engine which languages are worth grammars; they are not findings, they carry no provenance, and none of them appears in any manifest this project emits.

## Structure (exact)

| Measure | Head | Tail | All |
|---|---|---|---|
| Ships executable scripts | 20/881 (2.2%) | 3482/33403 (10.4%) | 3502/34284 (10.2%) |
| Has unreferenced files | 20/881 (2.2%) | 10286/33403 (30.7%) | 10306/34284 (30.0%) |
| Frontmatter parsed | 881/881 (100.0%) | 28654/33403 (85.7%) | 29535/34284 (86.1%) |
| Declares a version | 0/881 (0.0%) | 10254/33403 (30.6%) | 10254/34284 (29.9%) |
| Ships a LICENSE | 29/881 (3.2%) | 465/33403 (1.3%) | 494/34284 (1.4%) |

## The progressive-disclosure gap (exact)

The share of a bundle's bytes that an agent sees at session start. This is the asymmetry the project exists to measure: everything outside the description enters context later, on trigger, unobserved.

- Median description share: **2.09%** of bundle bytes (n=34283)
- Bundles where the description is under 1% of total bytes: 11139/34283 (32.4%)
- Bytes in files nothing points at: 1166210696 of 1629757921 total

## Languages present (exact)

The input to T4's grammar priorities: write rules for what the corpus actually contains, in that order.

| Language | Bundles containing it |
|---|---|
| `markdown` | 34279/34284 (99.9%) |
| `json` | 9273/34284 (27.0%) |
| `python` | 1776/34284 (5.1%) |
| `shell` | 1140/34284 (3.3%) |
| `unknown` | 968/34284 (2.8%) |
| `javascript` | 840/34284 (2.4%) |
| `text` | 583/34284 (1.7%) |
| `yaml` | 397/34284 (1.1%) |
| `typescript` | 362/34284 (1.0%) |
| `toml` | 97/34284 (0.2%) |
| `binary` | 85/34284 (0.2%) |
| `rust` | 12/34284 (0.0%) |
| `go` | 10/34284 (0.0%) |
| `dockerfile` | 5/34284 (0.0%) |
| `make` | 5/34284 (0.0%) |
| `ruby` | 1/34284 (0.0%) |

## Capability surface (lexical — upper bounds, not findings)

Bundles whose text contains the marker. See the method note above: these are substring matches, not analysis.

| Marker | Head | Tail | All | Of which, only in unreferenced files |
|---|---|---|---|---|
| `credential_paths` | 4/881 (0.4%) | 3065/33403 (9.1%) | 3069/34284 (8.9%) | 564/34284 (1.6%) |
| `secret_env` | 8/881 (0.9%) | 5859/33403 (17.5%) | 5867/34284 (17.1%) | 550/34284 (1.6%) |
| `network` | 866/881 (98.2%) | 18561/33403 (55.5%) | 19427/34284 (56.6%) | 3306/34284 (9.6%) |
| `agent_config_write` | 1/881 (0.1%) | 882/33403 (2.6%) | 883/34284 (2.5%) | 140/34284 (0.4%) |
| `dynamic_eval` | 43/881 (4.8%) | 8402/33403 (25.1%) | 8445/34284 (24.6%) | 1019/34284 (2.9%) |
| `install_fetch` | 1/881 (0.1%) | 542/33403 (1.6%) | 543/34284 (1.5%) | 123/34284 (0.3%) |
| `encoding_chain` | 7/881 (0.7%) | 1353/33403 (4.0%) | 1360/34284 (3.9%) | 449/34284 (1.3%) |

The last column is the shape worth looking at: machinery present in a bundle but only in files no documented path reaches. It is a lead for the labelling pass, not a conclusion about any bundle.

## Format spread (exact)

Whether the ecosystem uses frontmatter a single parser cannot absorb, and the input to the ≥5% resolver-scope rule in `docs/01-corpus-scan.md`.

| Frontmatter key | Bundles | Above 5% threshold |
|---|---|---|
| `version` | 10253/34284 (29.9%) | yes |
| `tags` | 8497/34284 (24.7%) | yes |
| `metadata` | 8217/34284 (23.9%) | yes |
| `triggers` | 6000/34284 (17.5%) | yes |
| `id` | 5611/34284 (16.3%) | yes |
| `license` | 5255/34284 (15.3%) | yes |
| `source` | 4209/34284 (12.2%) | yes |
| `risk` | 3389/34284 (9.8%) | yes |
| `repository` | 3121/34284 (9.1%) | yes |
| `keywords` | 3065/34284 (8.9%) | yes |
| `date_added` | 2927/34284 (8.5%) | yes |
| `author` | 2426/34284 (7.0%) | yes |
| `homepage` | 1836/34284 (5.3%) | yes |
| `disable-model-invocation` | 1698/34284 (4.9%) | no |
| `category` | 1569/34284 (4.5%) | no |
| `allowed-tools` | 1399/34284 (4.0%) | no |
| `examples` | 1156/34284 (3.3%) | no |
| `compatibility` | 999/34284 (2.9%) | no |
| `requires` | 966/34284 (2.8%) | no |
| `user-invocable` | 915/34284 (2.6%) | no |
| `platforms` | 740/34284 (2.1%) | no |
| `compatible` | 637/34284 (1.8%) | no |
| `tools` | 451/34284 (1.3%) | no |
| `slug` | 413/34284 (1.2%) | no |
| `origin` | 377/34284 (1.0%) | no |
| `emoji` | 313/34284 (0.9%) | no |
| `display_name` | 209/34284 (0.6%) | no |
| `argument-hint` | 206/34284 (0.6%) | no |
| `type` | 197/34284 (0.5%) | no |
| `source_type` | 188/34284 (0.5%) | no |
| `source_repo` | 158/34284 (0.4%) | no |
| `references` | 133/34284 (0.3%) | no |
| `skill_id` | 132/34284 (0.3%) | no |
| `credentials` | 130/34284 (0.3%) | no |
| `collaborates_with` | 129/34284 (0.3%) | no |
| `domain` | 127/34284 (0.3%) | no |
| `mitre_attack` | 122/34284 (0.3%) | no |
| `subdomain` | 122/34284 (0.3%) | no |
| `priority` | 121/34284 (0.3%) | no |
| `created` | 117/34284 (0.3%) | no |
| `d3fend` | 113/34284 (0.3%) | no |
| `updated` | 111/34284 (0.3%) | no |
| `license_source` | 108/34284 (0.3%) | no |
| `ethics_required` | 95/34284 (0.2%) | no |
| `frameworks` | 95/34284 (0.2%) | no |
| `permissions` | 94/34284 (0.2%) | no |
| `install` | 84/34284 (0.2%) | no |
| `user-invokable` | 79/34284 (0.2%) | no |
| `auto-activate` | 63/34284 (0.1%) | no |
| `capabilities` | 63/34284 (0.1%) | no |
| `executable` | 63/34284 (0.1%) | no |
| `price_usd` | 63/34284 (0.1%) | no |
| `content_type` | 56/34284 (0.1%) | no |
| `when_to_use` | 54/34284 (0.1%) | no |
| `displayName` | 49/34284 (0.1%) | no |
| `status` | 49/34284 (0.1%) | no |
| `models` | 45/34284 (0.1%) | no |
| `preamble-tier` | 41/34284 (0.1%) | no |
| `env` | 39/34284 (0.1%) | no |
| `outputs` | 39/34284 (0.1%) | no |
| `lead_agent` | 35/34284 (0.1%) | no |
| `library` | 35/34284 (0.1%) | no |
| `office` | 35/34284 (0.1%) | no |
| `payment` | 35/34284 (0.1%) | no |
| `model` | 34/34284 (0.0%) | no |
| `read_when` | 34/34284 (0.0%) | no |
| `inputs` | 33/34284 (0.0%) | no |
| `scope` | 33/34284 (0.0%) | no |
| `summary` | 33/34284 (0.0%) | no |
| `trigger` | 32/34284 (0.0%) | no |
| `protocols` | 31/34284 (0.0%) | no |
| `role` | 31/34284 (0.0%) | no |
| `output-format` | 30/34284 (0.0%) | no |
| `title` | 30/34284 (0.0%) | no |
| `depends-on` | 28/34284 (0.0%) | no |
| `requirements` | 28/34284 (0.0%) | no |
| `changelog` | 27/34284 (0.0%) | no |
| `execution` | 27/34284 (0.0%) | no |
| `source-books` | 27/34284 (0.0%) | no |
| `price` | 26/34284 (0.0%) | no |
| `context` | 25/34284 (0.0%) | no |
| `discovery` | 24/34284 (0.0%) | no |
| `plugin` | 24/34284 (0.0%) | no |
| `requires_commands` | 24/34284 (0.0%) | no |
| `tested_date` | 23/34284 (0.0%) | no |
| `tested_with` | 23/34284 (0.0%) | no |
| `pricing` | 21/34284 (0.0%) | no |
| `generated` | 20/34284 (0.0%) | no |
| `requires.env` | 20/34284 (0.0%) | no |
| `support` | 20/34284 (0.0%) | no |
| `confidence_default` | 17/34284 (0.0%) | no |
| `dependencies` | 17/34284 (0.0%) | no |
| `AIGC` | 16/34284 (0.0%) | no |
| `source_section` | 16/34284 (0.0%) | no |
| `agent_created` | 15/34284 (0.0%) | no |
| `car` | 15/34284 (0.0%) | no |
| `tagline` | 15/34284 (0.0%) | no |
| `business_outcome` | 14/34284 (0.0%) | no |
| `everyday_skill` | 14/34284 (0.0%) | no |
| `pack_tier` | 14/34284 (0.0%) | no |
| `primary_framework` | 14/34284 (0.0%) | no |
| `product_id` | 14/34284 (0.0%) | no |
| `ships_with` | 14/34284 (0.0%) | no |
| `alias` | 13/34284 (0.0%) | no |
| `command-dispatch` | 13/34284 (0.0%) | no |
| `commands` | 13/34284 (0.0%) | no |
| `config` | 13/34284 (0.0%) | no |
| `related_skills` | 13/34284 (0.0%) | no |
| `adr` | 12/34284 (0.0%) | no |
| `api_base` | 12/34284 (0.0%) | no |
| `hooks` | 12/34284 (0.0%) | no |
| `layer` | 12/34284 (0.0%) | no |
| `security` | 12/34284 (0.0%) | no |
| `workflow` | 12/34284 (0.0%) | no |
| `command-tool` | 11/34284 (0.0%) | no |
| `model_hint` | 11/34284 (0.0%) | no |
| `authors` | 10/34284 (0.0%) | no |
| `cloud_safe` | 10/34284 (0.0%) | no |
| `enabled` | 10/34284 (0.0%) | no |
| `languages` | 10/34284 (0.0%) | no |
| `package` | 10/34284 (0.0%) | no |
| `required_environment_variables` | 10/34284 (0.0%) | no |
| `requires_env` | 10/34284 (0.0%) | no |
| `trigger_keywords` | 10/34284 (0.0%) | no |
| `when-to-use` | 10/34284 (0.0%) | no |
| `complexity` | 9/34284 (0.0%) | no |
| `description_zh` | 9/34284 (0.0%) | no |
| `effort` | 9/34284 (0.0%) | no |
| `nist_csf` | 9/34284 (0.0%) | no |
| `agent` | 8/34284 (0.0%) | no |
| `description_en` | 8/34284 (0.0%) | no |
| `includes` | 8/34284 (0.0%) | no |
| `library_version` | 8/34284 (0.0%) | no |
| `os` | 8/34284 (0.0%) | no |
| `source_url` | 8/34284 (0.0%) | no |
| `sources` | 8/34284 (0.0%) | no |
| `website` | 8/34284 (0.0%) | no |
| `color` | 7/34284 (0.0%) | no |
| `command-arg-mode` | 7/34284 (0.0%) | no |
| `entrypoint` | 7/34284 (0.0%) | no |
| `files` | 7/34284 (0.0%) | no |
| `globs` | 7/34284 (0.0%) | no |
| `icon` | 7/34284 (0.0%) | no |
| `language` | 7/34284 (0.0%) | no |
| `mcp` | 7/34284 (0.0%) | no |
| `owner` | 7/34284 (0.0%) | no |
| `runtime` | 7/34284 (0.0%) | no |
| `badge` | 6/34284 (0.0%) | no |
| `confidence` | 6/34284 (0.0%) | no |
| `connectors` | 6/34284 (0.0%) | no |
| `department` | 6/34284 (0.0%) | no |
| `entry_point` | 6/34284 (0.0%) | no |
| `gbrain` | 6/34284 (0.0%) | no |
| `lemonsqueezy_variant_id` | 6/34284 (0.0%) | no |
| `od` | 6/34284 (0.0%) | no |
| `platform` | 6/34284 (0.0%) | no |
| `url` | 6/34284 (0.0%) | no |
| `when` | 6/34284 (0.0%) | no |
| `arguments` | 5/34284 (0.0%) | no |
| `attribution` | 5/34284 (0.0%) | no |
| `categories` | 5/34284 (0.0%) | no |
| `clawdis` | 5/34284 (0.0%) | no |
| `docs` | 5/34284 (0.0%) | no |
| `last_updated` | 5/34284 (0.0%) | no |
| `paths` | 5/34284 (0.0%) | no |
| `prerequisites` | 5/34284 (0.0%) | no |
| `primaryEnv` | 5/34284 (0.0%) | no |
| `required_env` | 5/34284 (0.0%) | no |
| `structure` | 5/34284 (0.0%) | no |
| `user_invocable` | 5/34284 (0.0%) | no |
| `alwaysApply` | 4/34284 (0.0%) | no |
| `auth` | 4/34284 (0.0%) | no |
| `auto_trigger` | 4/34284 (0.0%) | no |
| `benefits-from` | 4/34284 (0.0%) | no |
| `bins` | 4/34284 (0.0%) | no |
| `dataPolicy` | 4/34284 (0.0%) | no |
| `documentation` | 4/34284 (0.0%) | no |
| `entry` | 4/34284 (0.0%) | no |
| `estimated_time` | 4/34284 (0.0%) | no |
| `github` | 4/34284 (0.0%) | no |
| `instructions` | 4/34284 (0.0%) | no |
| `interactive` | 4/34284 (0.0%) | no |
| `level` | 4/34284 (0.0%) | no |
| `mcp_server` | 4/34284 (0.0%) | no |
| `privacy` | 4/34284 (0.0%) | no |
| `provides` | 4/34284 (0.0%) | no |
| `publisher` | 4/34284 (0.0%) | no |
| `requires_tools` | 4/34284 (0.0%) | no |
| `skill_type` | 4/34284 (0.0%) | no |
| `aliases` | 3/34284 (0.0%) | no |
| `assets` | 3/34284 (0.0%) | no |
| `atlas_techniques` | 3/34284 (0.0%) | no |
| `authorUrl` | 3/34284 (0.0%) | no |
| `auto_invoke` | 3/34284 (0.0%) | no |
| `best_for` | 3/34284 (0.0%) | no |
| `clawhub` | 3/34284 (0.0%) | no |
| `compatible-with` | 3/34284 (0.0%) | no |
| `date` | 3/34284 (0.0%) | no |
| `disableModelInvocation` | 3/34284 (0.0%) | no |
| `email` | 3/34284 (0.0%) | no |
| `input` | 3/34284 (0.0%) | no |
| `intent` | 3/34284 (0.0%) | no |
| `invocable` | 3/34284 (0.0%) | no |
| `metadata.openclaw` | 3/34284 (0.0%) | no |
| `npm` | 3/34284 (0.0%) | no |
| `output` | 3/34284 (0.0%) | no |
| `progressive` | 3/34284 (0.0%) | no |
| `requires_auth` | 3/34284 (0.0%) | no |
| `scenarios` | 3/34284 (0.0%) | no |
| `spec_version` | 3/34284 (0.0%) | no |
| `tools_required` | 3/34284 (0.0%) | no |
| `upstream` | 3/34284 (0.0%) | no |
| `workers` | 3/34284 (0.0%) | no |
| `Quick Install` | 2/34284 (0.0%) | no |
| `Report Issues` | 2/34284 (0.0%) | no |
| `acceptLicenseTerms` | 2/34284 (0.0%) | no |
| `allowModelInvocation` | 2/34284 (0.0%) | no |
| `allowed_tools` | 2/34284 (0.0%) | no |
| `always` | 2/34284 (0.0%) | no |
| `api-reference` | 2/34284 (0.0%) | no |
| `authorEmail` | 2/34284 (0.0%) | no |
| `author_url` | 2/34284 (0.0%) | no |
| `baseUrl` | 2/34284 (0.0%) | no |
| `base_url` | 2/34284 (0.0%) | no |
| `bundle` | 2/34284 (0.0%) | no |
| `chainTo` | 2/34284 (0.0%) | no |
| `channels` | 2/34284 (0.0%) | no |
| `competitive_advantage` | 2/34284 (0.0%) | no |
| `configPaths` | 2/34284 (0.0%) | no |
| `creator` | 2/34284 (0.0%) | no |
| `credential_scope` | 2/34284 (0.0%) | no |
| `credits` | 2/34284 (0.0%) | no |
| `dependency` | 2/34284 (0.0%) | no |
| `depends` | 2/34284 (0.0%) | no |
| `difficulty` | 2/34284 (0.0%) | no |
| `disable` | 2/34284 (0.0%) | no |
| `endpoint` | 2/34284 (0.0%) | no |
| `env_vars` | 2/34284 (0.0%) | no |
| `external_tool` | 2/34284 (0.0%) | no |
| `formats` | 2/34284 (0.0%) | no |
| `gating` | 2/34284 (0.0%) | no |
| `has_executable_code` | 2/34284 (0.0%) | no |
| `hats` | 2/34284 (0.0%) | no |
| `hide` | 2/34284 (0.0%) | no |
| `installer` | 2/34284 (0.0%) | no |
| `instruction_only` | 2/34284 (0.0%) | no |
| `logo` | 2/34284 (0.0%) | no |
| `logoDark` | 2/34284 (0.0%) | no |
| `min_openclaw_version` | 2/34284 (0.0%) | no |
| `name_zh` | 2/34284 (0.0%) | no |
| `network` | 2/34284 (0.0%) | no |
| `notes` | 2/34284 (0.0%) | no |
| `openclaw` | 2/34284 (0.0%) | no |
| `optional_env` | 2/34284 (0.0%) | no |
| `optional_envs` | 2/34284 (0.0%) | no |
| `override-tools` | 2/34284 (0.0%) | no |
| `progressive_disclosure` | 2/34284 (0.0%) | no |
| `repo` | 2/34284 (0.0%) | no |
| `requiredEnv` | 2/34284 (0.0%) | no |
| `required_binaries` | 2/34284 (0.0%) | no |
| `required_commands` | 2/34284 (0.0%) | no |
| `required_context` | 2/34284 (0.0%) | no |
| `required_env_vars` | 2/34284 (0.0%) | no |
| `retrieval` | 2/34284 (0.0%) | no |
| `security_notes` | 2/34284 (0.0%) | no |
| `system_prompt` | 2/34284 (0.0%) | no |
| `target_audience` | 2/34284 (0.0%) | no |
| `theme` | 2/34284 (0.0%) | no |
| `validate` | 2/34284 (0.0%) | no |
| `x` | 2/34284 (0.0%) | no |
| `> 📖 **Complete setup guide**` | 1/34284 (0.0%) | no |
| `NOT for` | 1/34284 (0.0%) | no |
| `Psychologist` | 1/34284 (0.0%) | no |
| `acknowledgments` | 1/34284 (0.0%) | no |
| `actions` | 1/34284 (0.0%) | no |
| `activation` | 1/34284 (0.0%) | no |
| `agent-requested` | 1/34284 (0.0%) | no |
| `agents` | 1/34284 (0.0%) | no |
| `allowedTools` | 1/34284 (0.0%) | no |
| `apiBase` | 1/34284 (0.0%) | no |
| `api_key_env` | 1/34284 (0.0%) | no |
| `ar_description` | 1/34284 (0.0%) | no |
| `args` | 1/34284 (0.0%) | no |
| `auth_type` | 1/34284 (0.0%) | no |
| `author-url` | 1/34284 (0.0%) | no |
| `author_brand` | 1/34284 (0.0%) | no |
| `author_link` | 1/34284 (0.0%) | no |
| `author_pen_name` | 1/34284 (0.0%) | no |
| `author_website` | 1/34284 (0.0%) | no |
| `base_url_env` | 1/34284 (0.0%) | no |
| `based-on` | 1/34284 (0.0%) | no |
| `binaries` | 1/34284 (0.0%) | no |
| `bond_type` | 1/34284 (0.0%) | no |
| `bonded_agent` | 1/34284 (0.0%) | no |
| `capabilityClasses` | 1/34284 (0.0%) | no |
| `chinese_name` | 1/34284 (0.0%) | no |
| `comind_version` | 1/34284 (0.0%) | no |
| `command` | 1/34284 (0.0%) | no |
| `commandArgMode` | 1/34284 (0.0%) | no |
| `commandDispatch` | 1/34284 (0.0%) | no |
| `commandTool` | 1/34284 (0.0%) | no |
| `compatible_agents` | 1/34284 (0.0%) | no |
| `compliance` | 1/34284 (0.0%) | no |
| `compression_schedule` | 1/34284 (0.0%) | no |
| `config_paths` | 1/34284 (0.0%) | no |
| `coreSkill` | 1/34284 (0.0%) | no |
| `cost` | 1/34284 (0.0%) | no |
| `createdAt` | 1/34284 (0.0%) | no |
| `credential` | 1/34284 (0.0%) | no |
| `cron` | 1/34284 (0.0%) | no |
| `d3fend_techniques` | 1/34284 (0.0%) | no |
| `daemon` | 1/34284 (0.0%) | no |
| `data_access` | 1/34284 (0.0%) | no |
| `data_sent` | 1/34284 (0.0%) | no |
| `density-score` | 1/34284 (0.0%) | no |
| `dependencies_skills` | 1/34284 (0.0%) | no |
| `dharmic_gates` | 1/34284 (0.0%) | no |
| `differentiators` | 1/34284 (0.0%) | no |
| `dimensions` | 1/34284 (0.0%) | no |
| `directories` | 1/34284 (0.0%) | no |
| `display-name` | 1/34284 (0.0%) | no |
| `displayNameEn` | 1/34284 (0.0%) | no |
| `distribution` | 1/34284 (0.0%) | no |
| `dogfooded_in` | 1/34284 (0.0%) | no |
| `dsl` | 1/34284 (0.0%) | no |
| `engine_version` | 1/34284 (0.0%) | no |
| `entryPoint` | 1/34284 (0.0%) | no |
| `esim` | 1/34284 (0.0%) | no |
| `ethical_constraint` | 1/34284 (0.0%) | no |
| `evidenceFiles` | 1/34284 (0.0%) | no |
| `exclude_tools` | 1/34284 (0.0%) | no |
| `execution-mode` | 1/34284 (0.0%) | no |
| `execution-tier` | 1/34284 (0.0%) | no |
| `export_to_journal` | 1/34284 (0.0%) | no |
| `external_access` | 1/34284 (0.0%) | no |
| `external_merge_rule` | 1/34284 (0.0%) | no |
| `external_source` | 1/34284 (0.0%) | no |
| `failures` | 1/34284 (0.0%) | no |
| `features` | 1/34284 (0.0%) | no |
| `first_time_user_instructions` | 1/34284 (0.0%) | no |
| `flights` | 1/34284 (0.0%) | no |
| `force_tool_turns` | 1/34284 (0.0%) | no |
| `free_endpoints` | 1/34284 (0.0%) | no |
| `functions` | 1/34284 (0.0%) | no |
| `generatedBy` | 1/34284 (0.0%) | no |
| `governs` | 1/34284 (0.0%) | no |
| `health_check` | 1/34284 (0.0%) | no |
| `hidden` | 1/34284 (0.0%) | no |
| `hook_point` | 1/34284 (0.0%) | no |
| `hotels` | 1/34284 (0.0%) | no |
| `how to get access` | 1/34284 (0.0%) | no |
| `inject` | 1/34284 (0.0%) | no |
| `integration_test` | 1/34284 (0.0%) | no |
| `invocation` | 1/34284 (0.0%) | no |
| `issues_url` | 1/34284 (0.0%) | no |
| `label` | 1/34284 (0.0%) | no |
| `lang` | 1/34284 (0.0%) | no |
| `lastAudited` | 1/34284 (0.0%) | no |
| `legacy_aliases` | 1/34284 (0.0%) | no |
| `maintainer` | 1/34284 (0.0%) | no |
| `match` | 1/34284 (0.0%) | no |
| `mcp-server` | 1/34284 (0.0%) | no |
| `mcpServers` | 1/34284 (0.0%) | no |
| `mcp_integration` | 1/34284 (0.0%) | no |
| `mcp_tools` | 1/34284 (0.0%) | no |
| `metadata.clawdbot` | 1/34284 (0.0%) | no |
| `metadata.openclaw.requires.bins` | 1/34284 (0.0%) | no |
| `min_core_version` | 1/34284 (0.0%) | no |
| `min_sdk_version` | 1/34284 (0.0%) | no |
| `min_version` | 1/34284 (0.0%) | no |
| `mode` | 1/34284 (0.0%) | no |
| `model-invocable` | 1/34284 (0.0%) | no |
| `namespace` | 1/34284 (0.0%) | no |
| `network_access` | 1/34284 (0.0%) | no |
| `network_requests` | 1/34284 (0.0%) | no |
| `nist_ai_rmf` | 1/34284 (0.0%) | no |
| `on_demand` | 1/34284 (0.0%) | no |
| `openclaw_min_version` | 1/34284 (0.0%) | no |
| `parameters` | 1/34284 (0.0%) | no |
| `payment_currency` | 1/34284 (0.0%) | no |
| `payment_network` | 1/34284 (0.0%) | no |
| `payment_protocol` | 1/34284 (0.0%) | no |
| `persistence` | 1/34284 (0.0%) | no |
| `personas` | 1/34284 (0.0%) | no |
| `phase` | 1/34284 (0.0%) | no |
| `platform_integration_requirements` | 1/34284 (0.0%) | no |
| `port` | 1/34284 (0.0%) | no |
| `ports` | 1/34284 (0.0%) | no |
| `postInstall` | 1/34284 (0.0%) | no |
| `price_currency` | 1/34284 (0.0%) | no |
| `price_per_call` | 1/34284 (0.0%) | no |
| `pricing_endpoint` | 1/34284 (0.0%) | no |
| `protocol` | 1/34284 (0.0%) | no |
| `protocol_version` | 1/34284 (0.0%) | no |
| `provider` | 1/34284 (0.0%) | no |
| `publish` | 1/34284 (0.0%) | no |
| `publishedAt` | 1/34284 (0.0%) | no |
| `publisher_wallet` | 1/34284 (0.0%) | no |
| `quality` | 1/34284 (0.0%) | no |
| `refresh` | 1/34284 (0.0%) | no |
| `registry` | 1/34284 (0.0%) | no |
| `relay_url` | 1/34284 (0.0%) | no |
| `release` | 1/34284 (0.0%) | no |
| `repo_url` | 1/34284 (0.0%) | no |
| `require-explicit` | 1/34284 (0.0%) | no |
| `required` | 1/34284 (0.0%) | no |
| `required_credentials` | 1/34284 (0.0%) | no |
| `required_env_note` | 1/34284 (0.0%) | no |
| `required_privileges` | 1/34284 (0.0%) | no |
| `required_secrets` | 1/34284 (0.0%) | no |
| `requires-env` | 1/34284 (0.0%) | no |
| `requires.bins` | 1/34284 (0.0%) | no |
| `requiresEnvVars` | 1/34284 (0.0%) | no |
| `requires_approval` | 1/34284 (0.0%) | no |
| `requires_binaries` | 1/34284 (0.0%) | no |
| `requires_bins` | 1/34284 (0.0%) | no |
| `requires_config` | 1/34284 (0.0%) | no |
| `research_coverage` | 1/34284 (0.0%) | no |
| `risk_description` | 1/34284 (0.0%) | no |
| `risk_level` | 1/34284 (0.0%) | no |
| `risk_tier` | 1/34284 (0.0%) | no |
| `safety_features` | 1/34284 (0.0%) | no |
| `sasmp_version` | 1/34284 (0.0%) | no |
| `schedule` | 1/34284 (0.0%) | no |
| `schema` | 1/34284 (0.0%) | no |
| `schema_version` | 1/34284 (0.0%) | no |
| `scripts` | 1/34284 (0.0%) | no |
| `security-note` | 1/34284 (0.0%) | no |
| `security_level` | 1/34284 (0.0%) | no |
| `self_improvement` | 1/34284 (0.0%) | no |
| `self_improvement_enabled` | 1/34284 (0.0%) | no |
| `session_tracking` | 1/34284 (0.0%) | no |
| `settings` | 1/34284 (0.0%) | no |
| `sha256` | 1/34284 (0.0%) | no |
| `shakti_flow` | 1/34284 (0.0%) | no |
| `skillType` | 1/34284 (0.0%) | no |
| `skill_name` | 1/34284 (0.0%) | no |
| `skill_version` | 1/34284 (0.0%) | no |
| `skillhub` | 1/34284 (0.0%) | no |
| `skills` | 1/34284 (0.0%) | no |
| `skills_count` | 1/34284 (0.0%) | no |
| `skills_url` | 1/34284 (0.0%) | no |
| `skip` | 1/34284 (0.0%) | no |
| `smokeTests` | 1/34284 (0.0%) | no |
| `sourceCode` | 1/34284 (0.0%) | no |
| `squad` | 1/34284 (0.0%) | no |
| `stages` | 1/34284 (0.0%) | no |
| `store` | 1/34284 (0.0%) | no |
| `subcategory` | 1/34284 (0.0%) | no |
| `super_user_bypass` | 1/34284 (0.0%) | no |
| `supersedes` | 1/34284 (0.0%) | no |
| `supporting_agents` | 1/34284 (0.0%) | no |
| `thinking_model` | 1/34284 (0.0%) | no |
| `tier` | 1/34284 (0.0%) | no |
| `timeout` | 1/34284 (0.0%) | no |
| `toolkit_root` | 1/34284 (0.0%) | no |
| `topic` | 1/34284 (0.0%) | no |
| `total_endpoints` | 1/34284 (0.0%) | no |
| `trial_days` | 1/34284 (0.0%) | no |
| `triggerWords` | 1/34284 (0.0%) | no |
| `trustScore` | 1/34284 (0.0%) | no |
| `twitter` | 1/34284 (0.0%) | no |
| `update` | 1/34284 (0.0%) | no |
| `updatedAt` | 1/34284 (0.0%) | no |
| `usage_hint` | 1/34284 (0.0%) | no |
| `var` | 1/34284 (0.0%) | no |
| `verification` | 1/34284 (0.0%) | no |
| `visibility` | 1/34284 (0.0%) | no |
| `wallet` | 1/34284 (0.0%) | no |
| `whenToUse` | 1/34284 (0.0%) | no |
| `workspace_access` | 1/34284 (0.0%) | no |
| `zhName` | 1/34284 (0.0%) | no |

**Frontmatter this project's strict parser refused: 4749/34284 (13.8%).** This is the number that decides whether refusing non-subset YAML is tenable. If it is not small, the parser widens — see `docs/00-tasks.md`.

## Where the sample came from (exact)

| Provenance | Bundles | Population |
|---|---|---|
| `baseline` | 18/34284 (0.0%) | head |
| `curated_list` | 863/34284 (2.5%) | head |
| `code_search` | 33403/34284 (97.4%) | tail |
| `explicit` | 0/34284 (0.0%) | head |

## Reached but not measured (29)

Recorded so that "we found nothing there" stays distinguishable from "we never looked".

- 28 × could not be cloned
- 1 × repository contained no SKILL.md

Bundle names are deliberately omitted. These are facts about this harvest — a clone that failed, a host antivirus that blocked a file — not findings about the bundles, and `docs/01-corpus-scan.md` is explicit that this report describes patterns rather than people.

---

## The decision this report exists to inform

`docs/01-corpus-scan.md` calls this the kill gate. If the base rates above are boring — few bundles ship scripts, hardly any touch credentials, the disclosure gap is small — then the risk this project addresses is theoretical, and the honest outcome is to publish that and stop. A negative result reported carefully is worth more than a scanner nobody needs.

No maintainer is named anywhere above, and no bundle is characterised as malicious. These are patterns, not accusations; see `SECURITY.md` for the disclosure process if something here looks live.
