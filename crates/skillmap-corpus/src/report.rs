//! The report, and the harvest that produces it.
//!
//! `docs/01-corpus-scan.md` sets the rules for what this file may say:
//!
//! > Lead with the base rates and the denominators. State the sampling method
//! > and its bias before the findings, not in a footnote. Include the negative
//! > results. Name no maintainer as a suspect — describe patterns, not people.
//!
//! All four are structural here rather than editorial. Every percentage is
//! rendered by [`percent`], which cannot print one without its denominator; the
//! bias section is emitted before any finding and is not optional; zero rows are
//! printed rather than skipped; and no repository is ever named in a row about a
//! capability marker — only in the provenance table, which carries no findings.

#![allow(
    clippy::integer_division,
    reason = "see the note in `measure`: every rate here is integer arithmetic on               purpose, because this project has no floats and a percentage that               printed differently on two platforms would undermine the one thing               the report is for."
)]

use crate::measure::{lexical_hit, marker_names, Measurements};
use crate::{
    archive::{Archive, LedgerEntry},
    measure, Error, Fetcher, Index, IndexRecord, Provenance, RepoRef, Skipped,
};
use skillmap_parse::{frontmatter, inventory, Limits};
use skillmap_resolve::ClaudeCode;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// Frontmatter keys that are not "extra" — the documented `SKILL.md` core.
const CORE_KEYS: &[&str] = &["name", "description"];

/// Keys that mark a bundle as carrying a version.
const VERSION_KEYS: &[&str] = &["version", "revision", "schema_version"];

/// Harvest `repos` into `archive`, measuring each bundle found.
///
/// Repositories already in the ledger at the same commit are not re-fetched;
/// that is the whole point of pinning the commit into the cache key.
///
/// # Errors
///
/// [`Error`] only for failures that stop the harvest. A repository that cannot
/// be fetched or holds no bundle is recorded in [`Index::skipped`] and the
/// harvest continues — one bad repository must not discard the rest of a run
/// that may have cost thousands of API calls.
pub fn harvest(
    repos: &[RepoRef],
    fetcher: &dyn Fetcher,
    archive: &Archive,
    snapshot: &str,
) -> Result<Index, Error> {
    harvest_with_sources(repos, fetcher, archive, snapshot, Vec::new())
}

/// Harvest, recording what each source contributed.
///
/// # Errors
///
/// As [`harvest`].
pub fn harvest_with_sources(
    repos: &[RepoRef],
    fetcher: &dyn Fetcher,
    archive: &Archive,
    snapshot: &str,
    sources: Vec<crate::SourceReport>,
) -> Result<Index, Error> {
    let mut ledger = archive.ledger();
    let mut records: Vec<IndexRecord> = Vec::new();
    let mut skipped: Vec<Skipped> = Vec::new();
    // Digests already recorded, so the same bundle vendored into several
    // repositories counts once. Without this the base rates double-count
    // whatever is most copied, which is exactly the popular stuff.
    let mut seen_digests: BTreeSet<String> = BTreeSet::new();

    let total = repos.len();
    for (position, repo) in repos.iter().enumerate() {
        // Progress goes to stderr from the library rather than through a callback.
        // This crate is a research tool run from a terminal, and a harvest of a
        // few hundred repositories is minutes of silence otherwise — which is
        // indistinguishable from a hang, and invites the Ctrl+C that used to
        // throw away the whole run's fetch cache.
        eprintln!("  [{}/{total}] {}", position + 1, repo.slug());
        let key = repo.cache_key();
        if let Some(entry) = ledger.entries.get(&key) {
            if let Some(reason) = &entry.empty_reason {
                skipped.push(Skipped {
                    repo: repo.slug(),
                    reason: format!("{reason} (cached)"),
                });
                continue;
            }
            // Re-measure from the archive rather than the network.
            for (root, digest) in &entry.bundles {
                if !seen_digests.insert(digest.clone()) {
                    continue;
                }
                match measure_archived(archive.bundle_dir(digest).as_path()) {
                    Ok(measurements) => records.push(IndexRecord {
                        digest: digest.clone(),
                        repo: repo.slug(),
                        commit: repo.commit.clone(),
                        bundle_root: root.clone(),
                        provenance: repo.provenance,
                        stars: repo.stars,
                        measurements,
                    }),
                    Err(error) => skipped.push(Skipped {
                        repo: repo.slug(),
                        reason: format!("archived bundle {root} could not be measured: {error}"),
                    }),
                }
            }
            continue;
        }

        let checkout = archive.checkout_dir(repo);
        let _ = std::fs::remove_dir_all(&checkout);
        if let Err(error) = fetcher.fetch(repo, &checkout) {
            skipped.push(Skipped {
                repo: repo.slug(),
                reason: format!("fetch failed: {error}"),
            });
            continue;
        }

        let bundles = find_bundles(&checkout);
        if bundles.is_empty() {
            let reason = "no SKILL.md found".to_owned();
            ledger.entries.insert(
                key,
                LedgerEntry {
                    bundles: BTreeMap::new(),
                    empty_reason: Some(reason.clone()),
                },
            );
            skipped.push(Skipped {
                repo: repo.slug(),
                reason,
            });
            let _ = std::fs::remove_dir_all(&checkout);
            archive.write_ledger(&ledger)?;
            continue;
        }

        let mut entry = LedgerEntry {
            bundles: BTreeMap::new(),
            empty_reason: None,
        };

        for bundle_path in bundles {
            let Some(root) = relative_slash(&checkout, &bundle_path) else {
                continue;
            };
            let Ok((digest, measurements)) = measure_bundle(&bundle_path) else {
                skipped.push(Skipped {
                    repo: repo.slug(),
                    reason: format!("bundle {root} could not be parsed"),
                });
                continue;
            };
            // Archiving is not allowed to be fatal, for the same reason fetching
            // is not: this function's own contract says one bad repository must
            // not discard a run that cost thousands of API calls. A real harvest
            // hit `os error 225` — a Windows Defender block on one third-party
            // `SKILL.md` — and lost 202 repositories of work to a `?`.
            //
            // Note what is *not* done here: the failure is not worked around by
            // copying the bundle partially. A partial copy would be archived
            // under a digest describing content it no longer holds, which is
            // worse than not archiving it at all.
            if let Err(error) = archive.store(&digest, &bundle_path) {
                skipped.push(Skipped {
                    repo: repo.slug(),
                    reason: format!("bundle {root} could not be archived: {error}"),
                });
                continue;
            }
            entry.bundles.insert(root.clone(), digest.clone());

            if !seen_digests.insert(digest.clone()) {
                continue;
            }
            records.push(IndexRecord {
                digest,
                repo: repo.slug(),
                commit: repo.commit.clone(),
                bundle_root: root,
                provenance: repo.provenance,
                stars: repo.stars,
                measurements,
            });
        }

        ledger.entries.insert(key, entry);
        let _ = std::fs::remove_dir_all(&checkout);

        // Persist after every repository. `docs/01-corpus-scan.md` says a re-run
        // must not re-fetch; writing the ledger only at the end made that true
        // only for runs that completed, and a 200-repository harvest is long
        // enough that interrupting one is the normal case, not the exception.
        archive.write_ledger(&ledger)?;
    }

    archive.write_ledger(&ledger)?;

    records.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    skipped.sort_by(|a, b| (a.repo.as_str(), a.reason.as_str()).cmp(&(&b.repo, &b.reason)));

    Ok(Index {
        snapshot: snapshot.to_owned(),
        sources,
        records,
        skipped,
    })
}

/// Every directory under `root` holding a `SKILL.md`, sorted.
fn find_bundles(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut work = vec![root.to_path_buf()];

    while let Some(dir) = work.pop() {
        if dir.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if dir.join("SKILL.md").is_file() {
            found.push(dir.clone());
            // Do not descend further: a bundle's own reference files are part of
            // that bundle, not separate bundles.
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                work.push(entry.path());
            }
        }
    }

    found.sort();
    found
}

/// A forward-slashed path relative to `base`.
fn relative_slash(base: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(base).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    })
}

/// Parse and measure a bundle directory.
fn measure_bundle(path: &Path) -> Result<(String, Measurements), Error> {
    let manifest =
        skillmap_parse::parse_path(path, &ClaudeCode, &Limits::default()).map_err(|error| {
            Error::Parse {
                context: path.display().to_string(),
                message: error.to_string(),
            }
        })?;

    let walk = inventory::walk(path, &Limits::default()).map_err(|error| Error::Parse {
        context: path.display().to_string(),
        message: error.to_string(),
    })?;

    let entry_text = walk
        .files
        .iter()
        .find(|file| file.path == skillmap_parse::ENTRY_FILE)
        .and_then(|file| file.text.as_deref());

    let (parsed, extra_keys, has_version) = match entry_text.map(frontmatter::parse) {
        Some(Ok(front)) => {
            let extra: Vec<String> = front
                .entries
                .keys()
                .filter(|key| !CORE_KEYS.contains(&key.as_str()))
                .cloned()
                .collect();
            let versioned = front
                .entries
                .keys()
                .any(|key| VERSION_KEYS.contains(&key.as_str()));
            (true, extra, versioned)
        }
        _ => (false, Vec::new(), false),
    };

    let measurements = measure::measure(&manifest, &walk.files, parsed, extra_keys, has_version);
    Ok((manifest.target.content_digest.to_wire(), measurements))
}

/// Re-measure an already-archived bundle, without touching the network.
fn measure_archived(path: &Path) -> Result<Measurements, Error> {
    measure_bundle(path).map(|(_, measurements)| measurements)
}

/// Render `index` as canonical JSON: sorted keys, two-space indent, LF, trailing
/// newline — the same framing the manifest uses, for the same reason.
///
/// # Errors
///
/// [`Error::Parse`] if serialization fails, which it cannot for this type.
pub fn index_json(index: &Index) -> Result<String, Error> {
    let value = serde_json::to_value(index).map_err(|error| Error::Parse {
        context: "index.json".to_owned(),
        message: error.to_string(),
    })?;
    let mut out = serde_json::to_string_pretty(&value).map_err(|error| Error::Parse {
        context: "index.json".to_owned(),
        message: error.to_string(),
    })?;
    out.push('\n');
    Ok(out)
}

/// Render a count as `n/d (p%)`, with the denominator always present.
///
/// A function rather than a formatting convention, because "state the
/// denominator every time" is the one rule that makes the difference between a
/// publishable base rate and a number somebody screenshots out of context. It is
/// not possible to print a percentage from this module without one.
///
/// Percentages are computed in integer tenths; the corpus has no floats anywhere,
/// and a rate printed to one decimal place is already more precision than a
/// sample of this size supports.
#[must_use]
pub fn percent(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        return format!("{numerator}/0 (n/a)");
    }
    let tenths = numerator.saturating_mul(1000) / denominator;
    format!(
        "{numerator}/{denominator} ({}.{}%)",
        tenths / 10,
        tenths % 10
    )
}

/// Build `report.md`.
#[must_use]
pub fn report(index: &Index) -> String {
    let mut out = String::new();
    let total = index.records.len() as u64;
    let head: Vec<&IndexRecord> = index
        .records
        .iter()
        .filter(|record| record.provenance.is_head())
        .collect();
    let tail: Vec<&IndexRecord> = index
        .records
        .iter()
        .filter(|record| !record.provenance.is_head())
        .collect();

    let _ = writeln!(out, "# Corpus scan — snapshot `{}`\n", index.snapshot);

    // ---- Method and bias, before any finding. Not a footnote. ----
    let _ = writeln!(
        out,
        "## Method, and what these numbers do not mean\n\n\
         Read this before the tables. Every number below is a base rate over a \
         **sample**, and the sample is not the ecosystem.\n"
    );
    let _ = writeln!(
        out,
        "- **{total} distinct bundles**, deduplicated by content digest. A bundle \
         vendored into several repositories counts once, so popular templates do \
         not inflate the rates.\n\
         - **Head vs tail is reported separately.** {} bundles came from sources a \
         human curated (the `anthropics/skills` baseline, awesome-lists, \
         operator-named repositories) and {} from GitHub code search. Curated \
         sources measure what people chose to write about; only code search \
         reaches the tail. Pooling them would describe neither population.\n\
         - **Code search has a hard ceiling.** GitHub returns at most 10 pages of \
         100 results, so the tail sample is bounded by the API, not by what \
         exists. Treat tail counts as a floor.\n\
         - **Only public repositories** are reachable, and only those whose \
         `SKILL.md` is indexed for code search.\n\
         - **Star counts come from the API**, never from secondary sources. \
         Published star figures for this ecosystem disagree with each other and \
         with the API.",
        head.len(),
        tail.len()
    );
    // What each source actually yielded. A source that returned nothing is a fact
    // about the harvest, not about the ecosystem, and the two are
    // indistinguishable in a bundle count.
    if !index.sources.is_empty() {
        let _ = writeln!(out, "\n### What each source yielded\n");
        let _ = writeln!(out, "| Source | Query | Repositories |");
        let _ = writeln!(out, "|---|---|---|");
        for source in &index.sources {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} |",
                source.provenance.as_str(),
                source.query,
                source.repositories
            );
        }

        for source in index.sources.iter().filter(|s| s.repositories == 0) {
            let _ = writeln!(
                out,
                "\n> **WARNING - the `{}` source returned zero repositories.** Every number below therefore describes only the sources that did return something. This is a fact about the harvest, not about the ecosystem: \"we could not look\" and \"we looked and found nothing\" are different statements, and a bundle count cannot tell them apart.{}",
                source.provenance.as_str(),
                if source.provenance.is_head() {
                    ""
                } else {
                    " Because this is the only source that reaches the tail, the corpus is entirely curated head, and nothing below can be read as an ecosystem rate."
                }
            );
        }
    }

    // Concentration. Deduplication by digest stops one bundle counting twice; it
    // does nothing about one repository contributing hundreds of *distinct*
    // generated bundles from a single template, which skews every rate here.
    if let Some((repo, count)) = dominant_repository(&index.records) {
        if total > 0 && count.saturating_mul(2) > total {
            let _ = writeln!(
                out,
                "\n> **WARNING - {} of the sample comes from one repository** (`{repo}`). Deduplication by content digest does not help here: these are distinct bundles, usually generated from one template, so every rate below describes that template more than it describes the ecosystem. Treat the frontmatter-key and language tables in particular as statements about a single publisher.",
                percent(count, total)
            );
        }
    }

    let _ = writeln!(
        out,
        "\n### Structural versus lexical\n\n\
         The **structural** tables are exact: they come from the same parser the \
         scanner uses, and say precisely what is in the bundle.\n\n\
         The **lexical** table is not. It counts bundles whose text *contains* a \
         marker substring — a credential path, an `eval(`, a URL. It does not \
         parse, does not establish reachability, and does not distinguish a live \
         call from the same string inside a comment, a docstring, or a warning not \
         to do the thing. **Every lexical number is an upper bound.** They are here \
         because they size the problem cheaply and tell the rule engine which \
         languages are worth grammars; they are not findings, they carry no \
         provenance, and none of them appears in any manifest this project emits.\n"
    );

    // ---- Structure ----
    let _ = writeln!(out, "## Structure (exact)\n");
    let _ = writeln!(out, "| Measure | Head | Tail | All |");
    let _ = writeln!(out, "|---|---|---|---|");
    structural_row(
        &mut out,
        "Ships executable scripts",
        &head,
        &tail,
        &index.records,
        |m| m.structure.has_scripts,
    );
    structural_row(
        &mut out,
        "Has unreferenced files",
        &head,
        &tail,
        &index.records,
        |m| m.structure.has_unreferenced,
    );
    structural_row(
        &mut out,
        "Frontmatter parsed",
        &head,
        &tail,
        &index.records,
        |m| m.governance.frontmatter_parsed,
    );
    structural_row(
        &mut out,
        "Declares a version",
        &head,
        &tail,
        &index.records,
        |m| m.governance.has_version,
    );
    structural_row(
        &mut out,
        "Ships a LICENSE",
        &head,
        &tail,
        &index.records,
        |m| m.governance.has_license,
    );

    // ---- The progressive-disclosure gap ----
    let _ = writeln!(
        out,
        "\n## The progressive-disclosure gap (exact)\n\n\
         The share of a bundle's bytes that an agent sees at session start. This \
         is the asymmetry the project exists to measure: everything outside the \
         description enters context later, on trigger, unobserved."
    );
    let shares: Vec<u64> = index
        .records
        .iter()
        .filter_map(|record| record.measurements.structure.description_share_ppm())
        .collect();
    if shares.is_empty() {
        let _ = writeln!(out, "\nNo bundle had any content to measure.");
    } else {
        let median = median_ppm(&shares);
        let _ = writeln!(
            out,
            "\n- Median description share: **{}.{:02}%** of bundle bytes (n={})\n\
             - Bundles where the description is under 1% of total bytes: {}",
            median / 10_000,
            (median % 10_000) / 100,
            shares.len(),
            percent(
                shares.iter().filter(|share| **share < 10_000).count() as u64,
                shares.len() as u64
            )
        );
    }

    let unreferenced_bytes: u64 = index
        .records
        .iter()
        .map(|record| record.measurements.structure.unreferenced_bytes)
        .sum();
    let total_bytes: u64 = index
        .records
        .iter()
        .map(|record| record.measurements.structure.total_bytes)
        .sum();
    let _ = writeln!(
        out,
        "- Bytes in files nothing points at: {} of {total_bytes} total",
        unreferenced_bytes
    );

    // ---- Languages ----
    let _ = writeln!(
        out,
        "\n## Languages present (exact)\n\n\
         The input to T4's grammar priorities: write rules for what the corpus \
         actually contains, in that order.\n"
    );
    let mut languages: BTreeMap<&str, u64> = BTreeMap::new();
    for record in &index.records {
        for language in record.measurements.structure.languages.keys() {
            *languages.entry(language.as_str()).or_default() += 1;
        }
    }
    let _ = writeln!(out, "| Language | Bundles containing it |");
    let _ = writeln!(out, "|---|---|");
    let mut ranked: Vec<(&&str, &u64)> = languages.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (language, count) in ranked {
        let _ = writeln!(out, "| `{language}` | {} |", percent(*count, total));
    }

    // ---- Lexical ----
    let _ = writeln!(
        out,
        "\n## Capability surface (lexical — upper bounds, not findings)\n\n\
         Bundles whose text contains the marker. See the method note above: these \
         are substring matches, not analysis.\n"
    );
    let _ = writeln!(
        out,
        "| Marker | Head | Tail | All | Of which, only in unreferenced files |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|");
    for name in marker_names() {
        let only_unreferenced = index
            .records
            .iter()
            .filter(|record| {
                record
                    .measurements
                    .lexical
                    .only_in_unreferenced
                    .iter()
                    .any(|marker| marker == name)
            })
            .count() as u64;
        let _ = writeln!(
            out,
            "| `{name}` | {} | {} | {} | {} |",
            percent(count_lexical(&head, name), head.len() as u64),
            percent(count_lexical(&tail, name), tail.len() as u64),
            percent(
                count_lexical(&index.records.iter().collect::<Vec<_>>(), name),
                total
            ),
            percent(only_unreferenced, total)
        );
    }
    let _ = writeln!(
        out,
        "\nThe last column is the shape worth looking at: machinery present in a \
         bundle but only in files no documented path reaches. It is a lead for the \
         labelling pass, not a conclusion about any bundle."
    );

    // ---- Format spread ----
    let _ = writeln!(
        out,
        "\n## Format spread (exact)\n\n\
         Whether the ecosystem uses frontmatter a single parser cannot absorb, and \
         the input to the ≥5% resolver-scope rule in `docs/01-corpus-scan.md`.\n"
    );
    let mut extra_keys: BTreeMap<&str, u64> = BTreeMap::new();
    for record in &index.records {
        for key in &record.measurements.governance.extra_frontmatter_keys {
            *extra_keys.entry(key.as_str()).or_default() += 1;
        }
    }
    if extra_keys.is_empty() {
        let _ = writeln!(
            out,
            "No bundle used a frontmatter key beyond `name` and `description`."
        );
    } else {
        let _ = writeln!(out, "| Frontmatter key | Bundles | Above 5% threshold |");
        let _ = writeln!(out, "|---|---|---|");
        let mut ranked: Vec<(&&str, &u64)> = extra_keys.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (key, count) in ranked {
            let above = total > 0 && count.saturating_mul(100) / total >= 5;
            let _ = writeln!(
                out,
                "| `{key}` | {} | {} |",
                percent(*count, total),
                if above { "yes" } else { "no" }
            );
        }
    }

    let unparsed = index
        .records
        .iter()
        .filter(|record| !record.measurements.governance.frontmatter_parsed)
        .count() as u64;
    let _ = writeln!(
        out,
        "\n**Frontmatter this project's strict parser refused: {}.** This is the \
         number that decides whether refusing non-subset YAML is tenable. If it is \
         not small, the parser widens — see `docs/00-tasks.md`.",
        percent(unparsed, total)
    );

    // ---- Provenance ----
    let _ = writeln!(out, "\n## Where the sample came from (exact)\n");
    let _ = writeln!(out, "| Provenance | Bundles | Population |");
    let _ = writeln!(out, "|---|---|---|");
    for provenance in Provenance::ALL {
        let count = index
            .records
            .iter()
            .filter(|record| record.provenance == *provenance)
            .count() as u64;
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            provenance.as_str(),
            percent(count, total),
            if provenance.is_head() { "head" } else { "tail" }
        );
    }

    if !index.skipped.is_empty() {
        let _ = writeln!(
            out,
            "\n## Reached but not measured ({})\n\n\
             Recorded so that \"we found nothing there\" stays distinguishable from \
             \"we never looked\".\n",
            index.skipped.len()
        );
        // Bucketed by class, never listed per bundle. Two reasons, both binding:
        //
        // `docs/01-corpus-scan.md` says to name no maintainer as a suspect and to
        // describe patterns rather than people. A list of several hundred named
        // third-party bundles under the phrase "contains a virus" is exactly that
        // prohibition — and it would be reporting one machine's antivirus
        // heuristic as though it were a finding this project had established.
        //
        // The raw reasons also carry local filesystem paths, which have no place
        // in a document meant to be read on a different machine than it was made.
        let mut reasons: BTreeMap<&str, u64> = BTreeMap::new();
        for entry in &index.skipped {
            *reasons.entry(skip_class(&entry.reason)).or_default() += 1;
        }
        let mut ranked: Vec<(&&str, &u64)> = reasons.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (reason, count) in ranked {
            let _ = writeln!(out, "- {count} × {reason}");
        }
        let _ = writeln!(
            out,
            "
Bundle names are deliberately omitted. These are facts about this              harvest — a clone that failed, a host antivirus that blocked a file —              not findings about the bundles, and `docs/01-corpus-scan.md` is              explicit that this report describes patterns rather than people."
        );
    }

    let _ = writeln!(
        out,
        "\n---\n\n\
         ## The decision this report exists to inform\n\n\
         `docs/01-corpus-scan.md` calls this the kill gate. If the base rates above \
         are boring — few bundles ship scripts, hardly any touch credentials, the \
         disclosure gap is small — then the risk this project addresses is \
         theoretical, and the honest outcome is to publish that and stop. A \
         negative result reported carefully is worth more than a scanner nobody \
         needs.\n\n\
         No maintainer is named anywhere above, and no bundle is characterised as \
         malicious. These are patterns, not accusations; see `SECURITY.md` for the \
         disclosure process if something here looks live."
    );

    out
}

/// Bucket a skip reason into a class safe to publish.
///
/// The raw reasons embed bundle paths and local filesystem paths. Neither belongs
/// in a published report: one names third parties, the other names this machine.
fn skip_class(reason: &str) -> &'static str {
    let lowered = reason.to_lowercase();
    if lowered.contains("virus") || lowered.contains("unwanted software") {
        "blocked by the host's antivirus while archiving"
    } else if lowered.contains("could not be archived") {
        "could not be written to the archive"
    } else if lowered.contains("fetch failed") {
        "could not be cloned"
    } else if lowered.contains("no skill.md") {
        "repository contained no SKILL.md"
    } else if lowered.contains("could not be parsed") {
        "bundle could not be parsed"
    } else if lowered.contains("could not be measured") {
        "archived bundle could not be re-measured"
    } else {
        "other"
    }
}

/// The repository contributing the most bundles, and how many.
fn dominant_repository(records: &[IndexRecord]) -> Option<(String, u64)> {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for record in records {
        *counts.entry(record.repo.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
        .map(|(repo, count)| (repo.to_owned(), count))
}

/// Median of a sorted-on-the-fly slice of parts-per-million values.
fn median_ppm(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted.get(middle).copied().unwrap_or(0)
    } else {
        let low = sorted.get(middle.saturating_sub(1)).copied().unwrap_or(0);
        let high = sorted.get(middle).copied().unwrap_or(0);
        low.saturating_add(high) / 2
    }
}

/// How many records set a named lexical marker.
fn count_lexical(records: &[&IndexRecord], name: &str) -> u64 {
    records
        .iter()
        .filter(|record| lexical_hit(&record.measurements.lexical, name))
        .count() as u64
}

/// Emit one head/tail/all row for a boolean structural measure.
fn structural_row(
    out: &mut String,
    label: &str,
    head: &[&IndexRecord],
    tail: &[&IndexRecord],
    all: &[IndexRecord],
    predicate: impl Fn(&Measurements) -> bool,
) {
    let count = |records: &[&IndexRecord]| {
        records
            .iter()
            .filter(|record| predicate(&record.measurements))
            .count() as u64
    };
    let all_refs: Vec<&IndexRecord> = all.iter().collect();
    let _ = writeln!(
        out,
        "| {label} | {} | {} | {} |",
        percent(count(head), head.len() as u64),
        percent(count(tail), tail.len() as u64),
        percent(count(&all_refs), all.len() as u64)
    );
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn a_percentage_can_never_be_printed_without_its_denominator() {
        assert_eq!(percent(1, 3), "1/3 (33.3%)");
        assert_eq!(percent(0, 10), "0/10 (0.0%)");
        assert_eq!(percent(10, 10), "10/10 (100.0%)");
        // The one case that would otherwise divide by zero.
        assert_eq!(percent(0, 0), "0/0 (n/a)");
    }

    #[test]
    fn median_handles_both_parities_and_the_empty_case() {
        assert_eq!(median_ppm(&[10, 20, 30]), 20);
        assert_eq!(median_ppm(&[10, 20, 30, 40]), 25);
        assert_eq!(median_ppm(&[]), 0);
    }

    #[test]
    fn an_empty_corpus_still_produces_an_honest_report() {
        // The kill-gate case: the harvest found nothing. The report must say so
        // rather than panicking or printing a table of NaNs.
        let index = Index {
            sources: Vec::new(),
            snapshot: "empty".to_owned(),
            records: Vec::new(),
            skipped: Vec::new(),
        };
        let text = report(&index);
        assert!(text.contains("0 distinct bundles"));
        assert!(
            text.contains("n/a"),
            "zero denominators must render honestly"
        );
        assert!(text.contains("kill gate"));
        assert!(!text.contains("NaN"));
    }

    #[test]
    fn the_report_states_bias_before_any_finding() {
        let index = Index {
            sources: Vec::new(),
            snapshot: "s".to_owned(),
            records: Vec::new(),
            skipped: Vec::new(),
        };
        let text = report(&index);
        let bias = text.find("what these numbers do not mean").unwrap();
        let findings = text.find("## Structure").unwrap();
        assert!(
            bias < findings,
            "sampling bias must precede the findings, not sit in a footnote"
        );
    }

    #[test]
    fn a_source_that_returned_nothing_is_reported_loudly() {
        // The defect the first real harvest exposed. Code search returned zero,
        // the tail column rendered `0/0 (n/a)`, and every base rate silently
        // described the curated head alone. A bundle count cannot distinguish
        // "we could not look" from "we looked and found nothing" — so the source
        // has to say which it was.
        let index = Index {
            sources: vec![
                crate::SourceReport {
                    provenance: Provenance::CuratedList,
                    query: "acme/list".to_owned(),
                    repositories: 1,
                },
                crate::SourceReport {
                    provenance: Provenance::CodeSearch,
                    query: "filename:SKILL.md".to_owned(),
                    repositories: 0,
                },
            ],
            snapshot: "s".to_owned(),
            records: Vec::new(),
            skipped: Vec::new(),
        };
        let text = report(&index);

        assert!(text.contains("returned zero repositories"));
        assert!(
            text.contains("only source that reaches the tail"),
            "a zero-yield tail source must say the corpus is head-only"
        );
        // And the query itself is printed, so the sampling method is reproducible.
        assert!(text.contains("filename:SKILL.md"));
    }

    #[test]
    fn one_repository_dominating_the_sample_is_reported_loudly() {
        // Deduplication by digest does not help when one repository contributes
        // hundreds of *distinct* bundles generated from a single template: every
        // rate then describes that template, not the ecosystem.
        let mut records = Vec::new();
        for index in 0..9 {
            records.push(record("vendor/catalog", &format!("skills/{index}")));
        }
        records.push(record("someone/else", "skills/one"));

        let index = Index {
            sources: Vec::new(),
            snapshot: "s".to_owned(),
            records,
            skipped: Vec::new(),
        };
        let text = report(&index);
        assert!(text.contains("comes from one repository"));
        assert!(text.contains("vendor/catalog"));
        assert!(
            text.contains("9/10"),
            "the share must carry its denominator"
        );
    }

    #[test]
    fn a_balanced_sample_gets_no_concentration_warning() {
        let index = Index {
            sources: Vec::new(),
            snapshot: "s".to_owned(),
            records: vec![
                record("a/one", "skills/x"),
                record("b/two", "skills/y"),
                record("c/three", "skills/z"),
            ],
            skipped: Vec::new(),
        };
        assert!(!report(&index).contains("comes from one repository"));
    }

    /// A minimal index record for report tests.
    fn record(repo: &str, root: &str) -> IndexRecord {
        IndexRecord {
            digest: format!("sha256:{repo}{root}"),
            repo: repo.to_owned(),
            commit: "0".repeat(40),
            bundle_root: root.to_owned(),
            provenance: Provenance::CuratedList,
            stars: None,
            measurements: Measurements {
                structure: measure::Structure {
                    files: 1,
                    total_bytes: 100,
                    reference_bytes: 0,
                    unreferenced_bytes: 0,
                    description_bytes: 10,
                    has_unreferenced: false,
                    has_scripts: false,
                    languages: BTreeMap::new(),
                    unresolved: BTreeMap::new(),
                },
                lexical: measure::Lexical::default(),
                governance: measure::Governance {
                    has_license: false,
                    has_version: false,
                    extra_frontmatter_keys: Vec::new(),
                    frontmatter_parsed: true,
                },
            },
        }
    }

    #[test]
    fn lexical_numbers_are_labelled_as_upper_bounds() {
        let index = Index {
            sources: Vec::new(),
            snapshot: "s".to_owned(),
            records: Vec::new(),
            skipped: Vec::new(),
        };
        let text = report(&index);
        assert!(text.contains("upper bound"));
        assert!(text.contains("not findings"));
        assert!(
            text.contains("no provenance"),
            "the report must say why these can never be manifest findings"
        );
    }
}
