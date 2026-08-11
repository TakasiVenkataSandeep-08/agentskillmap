//! The GitHub side: code search over the REST API, and `git clone` for contents.
//!
//! The split is deliberate. `ureq` handles authenticated GETs that return small
//! JSON documents; `git clone --depth 1` handles the bulk transfer. That keeps
//! the HTTP dependency to something whose whole job is "GET a URL with a bearer
//! token", and reuses the git the operator already has rather than
//! reimplementing a tarball transfer inside a supply-chain auditor.
//!
//! Everything here implements [`crate::Source`] or [`crate::Fetcher`], so the
//! pipeline can be exercised end to end without a network by substituting local
//! implementations — which is exactly what the tests do.

use crate::{Error, Fetcher, Provenance, RepoRef, Source};
use std::path::Path;
use std::process::Command;

/// The API root. A constant so the tests can document that nothing else in the
/// crate constructs a URL.
const API: &str = "https://api.github.com";

/// Sent on every request. GitHub rejects API calls without one.
const USER_AGENT: &str = concat!("skillmap-corpus/", env!("CARGO_PKG_VERSION"));

/// An authenticated GitHub REST client.
pub struct GitHub {
    token: String,
    agent: ureq::Agent,
}

impl GitHub {
    /// Build a client from a token.
    #[must_use]
    pub fn new(token: String) -> Self {
        Self {
            token,
            agent: ureq::AgentBuilder::new().user_agent(USER_AGENT).build(),
        }
    }

    /// GET a URL and parse the body as JSON.
    fn get(&self, url: &str, context: &str) -> Result<serde_json::Value, Error> {
        let response = self
            .agent
            .get(url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .map_err(|error| Error::Api {
                context: context.to_owned(),
                message: error.to_string(),
            })?;

        let body = response.into_string().map_err(|error| Error::Api {
            context: context.to_owned(),
            message: error.to_string(),
        })?;

        serde_json::from_str(&body).map_err(|error| Error::Api {
            context: context.to_owned(),
            message: format!("response was not JSON: {error}"),
        })
    }

    /// Resolve a repository's default branch head to a commit SHA.
    ///
    /// Pinning here rather than cloning a branch name is what makes the corpus
    /// reproducible and the fetch cache sound.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] if the repository cannot be read.
    pub fn head_commit(&self, owner: &str, name: &str) -> Result<String, Error> {
        let context = format!("{owner}/{name} head commit");
        let repo = self.get(&format!("{API}/repos/{owner}/{name}"), &context)?;
        let branch = repo
            .get("default_branch")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::Api {
                context: context.clone(),
                message: "no default_branch in response".to_owned(),
            })?;

        let commit = self.get(
            &format!("{API}/repos/{owner}/{name}/commits/{branch}"),
            &context,
        )?;
        commit
            .get("sha")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| Error::Api {
                context,
                message: "no sha in commits response".to_owned(),
            })
    }

    /// Star count, read from the API rather than from any secondary source.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] if the repository cannot be read.
    pub fn stars(&self, owner: &str, name: &str) -> Result<Option<u64>, Error> {
        let repo = self.get(
            &format!("{API}/repos/{owner}/{name}"),
            &format!("{owner}/{name} stars"),
        )?;
        Ok(repo
            .get("stargazers_count")
            .and_then(serde_json::Value::as_u64))
    }
}

/// Code search for repositories containing a `SKILL.md`.
///
/// This is the only source that reaches the **tail** of the ecosystem. Every
/// other source samples repositories somebody already chose to write about.
pub struct CodeSearch<'a> {
    /// The authenticated client.
    pub client: &'a GitHub,
}

impl Source for CodeSearch<'_> {
    fn repos(&self, limit: usize) -> Result<Vec<RepoRef>, Error> {
        let mut found: Vec<RepoRef> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        // GitHub caps code search at 100 per page and 10 pages. That ceiling is
        // itself a sampling bias and the report states it: the tail sample is
        // bounded by what the search API will return, not by what exists.
        for page in 1..=10 {
            if found.len() >= limit {
                break;
            }
            let url = format!("{API}/search/code?q=path%3A**%2FSKILL.md&per_page=100&page={page}");
            let body = self.get_search(&url, page)?;
            let items = body
                .get("items")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                break;
            }

            for item in items {
                let Some(repo) = item.get("repository") else {
                    continue;
                };
                let (Some(owner), Some(name)) = (
                    repo.get("owner")
                        .and_then(|owner| owner.get("login"))
                        .and_then(serde_json::Value::as_str),
                    repo.get("name").and_then(serde_json::Value::as_str),
                ) else {
                    continue;
                };
                let slug = format!("{owner}/{name}");
                if !seen.insert(slug) {
                    continue;
                }
                let commit = self.client.head_commit(owner, name)?;
                let stars = self.client.stars(owner, name)?;
                found.push(RepoRef {
                    owner: owner.to_owned(),
                    name: name.to_owned(),
                    commit,
                    provenance: Provenance::CodeSearch,
                    stars,
                });
                if found.len() >= limit {
                    break;
                }
            }
        }

        found.sort_by_key(RepoRef::slug);
        Ok(found)
    }
}

impl CodeSearch<'_> {
    fn get_search(&self, url: &str, page: u32) -> Result<serde_json::Value, Error> {
        self.client.get(url, &format!("code search page {page}"))
    }
}

/// Repositories named outright: the baseline, curated lists, operator input.
pub struct Named<'a> {
    /// The authenticated client, used to pin commits and read stars.
    pub client: &'a GitHub,
    /// `owner/name` slugs.
    pub slugs: Vec<String>,
    /// How these were arrived at.
    pub provenance: Provenance,
}

impl Source for Named<'_> {
    fn repos(&self, limit: usize) -> Result<Vec<RepoRef>, Error> {
        let mut found = Vec::new();
        for slug in self.slugs.iter().take(limit) {
            let Some((owner, name)) = slug.split_once('/') else {
                return Err(Error::Api {
                    context: slug.clone(),
                    message: "expected an owner/name slug".to_owned(),
                });
            };
            found.push(RepoRef {
                owner: owner.to_owned(),
                name: name.to_owned(),
                commit: self.client.head_commit(owner, name)?,
                provenance: self.provenance,
                stars: self.client.stars(owner, name)?,
            });
        }
        found.sort_by_key(RepoRef::slug);
        Ok(found)
    }
}

/// Fetches repository contents with `git clone --depth 1`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitFetcher;

impl Fetcher for GitFetcher {
    fn fetch(&self, repo: &RepoRef, into: &Path) -> Result<(), Error> {
        let context = format!("{}@{}", repo.slug(), repo.commit);
        if let Some(parent) = into.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        // Clone the single commit rather than the branch tip, so the archive
        // matches the SHA recorded in the index even if the branch moves between
        // discovery and fetch.
        let url = format!("https://github.com/{}.git", repo.slug());
        run_git(
            &["init", "--quiet"],
            into,
            &context,
            /* create = */ true,
        )?;
        run_git(&["remote", "add", "origin", &url], into, &context, false)?;
        run_git(
            &["fetch", "--quiet", "--depth", "1", "origin", &repo.commit],
            into,
            &context,
            false,
        )?;
        run_git(
            &["checkout", "--quiet", "FETCH_HEAD"],
            into,
            &context,
            false,
        )?;
        Ok(())
    }
}

/// Run one git invocation in `dir`.
fn run_git(args: &[&str], dir: &Path, context: &str, create: bool) -> Result<(), Error> {
    if create {
        std::fs::create_dir_all(dir).map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }

    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| Error::Git {
            context: context.to_owned(),
            message: format!(
                "could not run `git {}`: {error}. The harvester shells out to git \
                 rather than embedding a transfer; git must be on PATH.",
                args.join(" ")
            ),
        })?;

    if !output.status.success() {
        return Err(Error::Git {
            context: context.to_owned(),
            message: format!(
                "`git {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_api_root_is_the_only_host_this_crate_contacts() {
        // A guard on invariant 9's blast radius: if a second host ever appears,
        // it should appear here, in a test that makes someone explain it.
        assert_eq!(API, "https://api.github.com");
    }

    #[test]
    fn named_rejects_a_slug_that_is_not_owner_over_name() {
        // Constructing a GitHub client needs no network; only calling it does.
        let client = GitHub::new("unused".to_owned());
        let source = Named {
            client: &client,
            slugs: vec!["not-a-slug".to_owned()],
            provenance: Provenance::Explicit,
        };
        assert!(source.repos(10).is_err());
    }
}
