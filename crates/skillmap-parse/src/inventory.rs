//! Walking a bundle: hashing, size limits, symlink containment, binary detection.
//!
//! Every file that exists in the bundle appears in the inventory, and anything
//! the walk could not fully handle also emits an `unresolved` entry. Those two
//! statements together are invariant 3: a scanner that reports nothing because it
//! understood nothing has to look different from one that reports nothing because
//! there was nothing there.

use crate::{Limits, ParseError};
use sha2::{Digest as _, Sha256};
use skillmap_core::{Digest, InventoryEntry, LoadPhase, ParseStatus, Unresolved, UnresolvedReason};
use skillmap_resolve::relative_slash_path;
use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// How many leading bytes are examined for a NUL when deciding text vs binary.
///
/// Matches what `git` does. A NUL in the first 8 KiB is the standard heuristic;
/// looking at the whole file would be slower and no more accurate in practice.
const BINARY_SNIFF_BYTES: usize = 8192;

/// Line feed and carriage return, named rather than written inline: this file
/// is entirely about newline handling, and a bare byte literal in the middle of
/// it is the one place the escape is easiest to misread.
const LF: u8 = b'\n';

/// Carriage return.
const CR: u8 = b'\r';

/// One walked file, before load-phase classification.
pub struct WalkedFile {
    /// Forward-slash path relative to the bundle root.
    pub path: String,
    /// Number of bytes that went into [`WalkedFile::sha256`] — that is, the
    /// LF-normalized length for text and the raw length for binary content.
    ///
    /// Deliberately **not** the on-disk length. A CRLF checkout of the same
    /// commit has more bytes on disk than an LF one, so reporting `stat` size
    /// would make the manifest differ across platforms even though the digest
    /// did not — invariant 2's exact failure mode, and a `size` that did not
    /// describe the hashed bytes would be confusing regardless.
    pub size: u64,
    /// SHA-256 of the content, LF-normalized for text.
    pub sha256: Digest,
    /// What the file was recognised as.
    pub parsed_as: &'static str,
    /// Whether the content could be read and understood.
    pub parse_status: ParseStatus,
    /// Decoded text, for files small enough and not binary. `None` otherwise, so
    /// the reference graph knows there is nothing to scan.
    pub text: Option<String>,
}

/// The result of walking a bundle.
pub struct Walk {
    /// Every file found, sorted by path.
    pub files: Vec<WalkedFile>,
    /// Everything the walk could not fully handle.
    pub unresolved: Vec<Unresolved>,
}

/// Walk `root`, hashing every file and recording what could not be handled.
///
/// # Errors
///
/// [`ParseError`] only for failures that make the whole bundle unreadable — the
/// root missing, or a directory that cannot be listed. A problem with a single
/// *file* is never an error: it becomes an `unresolved` entry, because one
/// unreadable file must not take down the scan of everything around it
/// (invariant 10).
pub fn walk(root: &Path, limits: &Limits) -> Result<Walk, ParseError> {
    let canonical_root = root.canonicalize().map_err(|source| ParseError::Io {
        path: root.to_path_buf(),
        source,
    })?;

    let mut files = Vec::new();
    let mut unresolved = Vec::new();
    // Canonical directory paths already descended into, so a symlink cycle that
    // stays inside the bundle terminates instead of looping forever.
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();
    visited.insert(canonical_root.clone());

    // An explicit worklist rather than recursion. Directory nesting is attacker
    // controlled — a bundle can ship a tree thousands of levels deep, and a
    // recursive walk would exhaust the stack on it. A stack overflow aborts the
    // process outright: it cannot be caught, so it is not merely a panic but an
    // unrecoverable one, and invariant 10 exists precisely because a crash on
    // malformed input is a denial of service on somebody's CI. Heap-allocated
    // worklist, no depth limit needed, no new `unresolved` reason code needed.
    let mut worklist: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = worklist.pop() {
        let children = descend(
            &dir,
            root,
            &canonical_root,
            limits,
            &mut visited,
            &mut files,
            &mut unresolved,
        )?;
        worklist.extend(children);
    }

    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    Ok(Walk { files, unresolved })
}

/// Process one directory, returning the subdirectories still to be walked.
#[allow(
    clippy::too_many_arguments,
    reason = "threading accumulators explicitly keeps the walk a plain function \
              rather than a struct whose fields would have to be public to test"
)]
fn descend(
    dir: &Path,
    root: &Path,
    canonical_root: &Path,
    limits: &Limits,
    visited: &mut BTreeSet<PathBuf>,
    files: &mut Vec<WalkedFile>,
    unresolved: &mut Vec<Unresolved>,
) -> Result<Vec<PathBuf>, ParseError> {
    let mut pending: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|source| ParseError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    // Sort before descending. Filesystems enumerate in arbitrary order, and the
    // walk order decides nothing about output ordering (everything is sorted
    // later) except which `unresolved` note wins a tie — but relying on that
    // would be exactly the kind of latent nondeterminism invariant 2 forbids.
    let mut children: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ParseError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        children.push(entry.path());
    }
    children.sort();

    for child in children {
        let Some(relative) = relative_slash_path(root, &child) else {
            // Not representable as a contained, UTF-8, forward-slash path. It
            // cannot go in the manifest, so it is reported rather than dropped.
            unresolved.push(Unresolved {
                reason: UnresolvedReason::SymlinkEscape,
                file: child.to_string_lossy().into_owned(),
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: Some(
                    "path is not a UTF-8 descendant of the bundle root and cannot be \
                     represented in a manifest"
                        .to_owned(),
                ),
            });
            continue;
        };

        let metadata = match std::fs::symlink_metadata(&child) {
            Ok(metadata) => metadata,
            Err(source) => {
                unresolved.push(Unresolved {
                    reason: UnresolvedReason::ParseError,
                    file: relative,
                    start_byte: None,
                    end_byte: None,
                    start_line: None,
                    note: Some(format!("cannot stat: {source}")),
                });
                continue;
            }
        };

        if metadata.is_symlink() {
            // Resolve and check containment. A symlink pointing outside the
            // bundle is content the bundle does not actually ship, so following
            // it would hash somebody else's bytes into this bundle's identity.
            match child.canonicalize() {
                Ok(target) if target.starts_with(canonical_root) => {
                    // Contained: fall through and treat it as the file or
                    // directory it points at.
                }
                Ok(_) | Err(_) => {
                    unresolved.push(Unresolved {
                        reason: UnresolvedReason::SymlinkEscape,
                        file: relative,
                        start_byte: None,
                        end_byte: None,
                        start_line: None,
                        note: Some(
                            "symlink resolves outside the bundle root, or is broken; \
                             its target is not part of this bundle"
                                .to_owned(),
                        ),
                    });
                    continue;
                }
            }
        }

        let is_dir = child.is_dir();
        if is_dir {
            // Canonicalize before recording as visited, so two paths reaching the
            // same directory through a symlink are recognised as one.
            let canonical = child.canonicalize().unwrap_or_else(|_| child.clone());
            if !visited.insert(canonical) {
                unresolved.push(Unresolved {
                    reason: UnresolvedReason::SymlinkEscape,
                    file: relative,
                    start_byte: None,
                    end_byte: None,
                    start_line: None,
                    note: Some(
                        "directory was already walked through another path; not \
                         descended again"
                            .to_owned(),
                    ),
                });
                continue;
            }
            pending.push(child);
            continue;
        }

        if !child.is_file() {
            // A socket, FIFO, or device node. Not content, but not silence either.
            unresolved.push(Unresolved {
                reason: UnresolvedReason::ParseError,
                file: relative,
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: Some("not a regular file".to_owned()),
            });
            continue;
        }

        ingest_file(&child, relative, metadata.len(), limits, files, unresolved);
    }

    Ok(pending)
}

/// Hash and classify a single regular file.
fn ingest_file(
    path: &Path,
    relative: String,
    size: u64,
    limits: &Limits,
    files: &mut Vec<WalkedFile>,
    unresolved: &mut Vec<Unresolved>,
) {
    // Over the limit: still hashed and still inventoried, because
    // `content_digest` means "these bytes" and omitting a file would change the
    // bundle's identity. What the limit buys is that the content is never held in
    // memory or analyzed.
    if size > limits.max_file_bytes {
        match hash_streaming(path) {
            Ok(hashed) => {
                unresolved.push(Unresolved {
                    reason: UnresolvedReason::SizeLimit,
                    file: relative.clone(),
                    start_byte: None,
                    end_byte: None,
                    start_line: None,
                    note: Some(format!(
                        "{size} bytes on disk exceeds the {} byte limit; hashed but not analyzed",
                        limits.max_file_bytes
                    )),
                });
                files.push(WalkedFile {
                    path: relative,
                    size: hashed.length,
                    sha256: hashed.digest,
                    parsed_as: "unknown",
                    parse_status: ParseStatus::Unsupported,
                    text: None,
                });
            }
            Err(source) => push_read_error(&relative, &source, files, unresolved),
        }
        return;
    }

    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) => {
            push_read_error(&relative, &source, files, unresolved);
            return;
        }
    };

    if is_binary(&bytes) {
        unresolved.push(Unresolved {
            reason: UnresolvedReason::BinaryFile,
            file: relative.clone(),
            start_byte: None,
            end_byte: None,
            start_line: None,
            note: Some("contains a NUL byte; not analyzed as text".to_owned()),
        });
        files.push(WalkedFile {
            path: relative,
            size: bytes.len() as u64,
            // Binary content is hashed raw: LF-normalizing it would corrupt it,
            // and its bytes are what they are on every platform anyway.
            sha256: Digest::of(&bytes),
            parsed_as: "binary",
            parse_status: ParseStatus::Unsupported,
            text: None,
        });
        return;
    }

    let Ok(text) = String::from_utf8(bytes) else {
        // Not binary by the NUL test, but not UTF-8 either — Latin-1 prose, say.
        // Hash the raw bytes so identity is still exact, and say why it was not
        // read as text rather than pretending it was empty.
        let raw = std::fs::read(path).unwrap_or_default();
        unresolved.push(Unresolved {
            reason: UnresolvedReason::ParseError,
            file: relative.clone(),
            start_byte: None,
            end_byte: None,
            start_line: None,
            note: Some("not valid UTF-8; hashed but not analyzed as text".to_owned()),
        });
        files.push(WalkedFile {
            path: relative,
            size: raw.len() as u64,
            sha256: Digest::of(&raw),
            parsed_as: "unknown",
            parse_status: ParseStatus::Error,
            text: None,
        });
        return;
    };

    // LF-normalize before hashing. A CRLF checkout on Windows must not change a
    // file's digest, or the same bundle has two identities and every lockfile
    // diff becomes platform noise (invariant 2).
    let normalized = normalize_newlines(&text);
    files.push(WalkedFile {
        path: relative.clone(),
        size: normalized.len() as u64,
        sha256: Digest::of(normalized.as_bytes()),
        parsed_as: language_of(&relative),
        parse_status: ParseStatus::Ok,
        text: Some(normalized),
    });
}

/// Record a file that exists but could not be read at all.
///
/// It still gets an inventory entry, with the zero-length digest standing in for
/// "no bytes were obtained" — dropping it would make the bundle look smaller than
/// it is, and the accompanying `unresolved` entry is what says the digest is not
/// a claim about content.
fn push_read_error(
    relative: &str,
    source: &std::io::Error,
    files: &mut Vec<WalkedFile>,
    unresolved: &mut Vec<Unresolved>,
) {
    unresolved.push(Unresolved {
        reason: UnresolvedReason::ParseError,
        file: relative.to_owned(),
        start_byte: None,
        end_byte: None,
        start_line: None,
        note: Some(format!("cannot read: {source}")),
    });
    files.push(WalkedFile {
        path: relative.to_owned(),
        // Zero bytes were obtained, so zero is what `size` reports: it describes
        // the hashed content, and the digest here is the empty one. The
        // accompanying `unresolved` entry is what says the file is not actually
        // empty, only unreadable.
        size: 0,
        sha256: Digest::of(&[]),
        parsed_as: "unknown",
        parse_status: ParseStatus::Error,
        text: None,
    });
}

/// A digest and the number of bytes that produced it.
pub struct Hashed {
    /// The digest.
    pub digest: Digest,
    /// How many bytes were fed into it, after any normalization.
    pub length: u64,
}

/// SHA-256 a file without holding it in memory, normalizing line endings.
///
/// Normalization matters here for the same reason it does on the in-memory path:
/// an oversized *text* file hashed raw would give a CRLF checkout a different
/// bundle identity than an LF one. Whether to normalize is decided by the same
/// NUL sniff used everywhere else, applied to the first chunk, so a file's
/// classification does not depend on how the reads happened to be chunked.
fn hash_streaming(path: &Path) -> Result<Hashed, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut normalized = Vec::with_capacity(buffer.len() + 1);

    let mut length: u64 = 0;
    let mut binary: Option<bool> = None;
    let mut pending_cr = false;

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let Some(chunk) = buffer.get(..read) else {
            break;
        };
        let is_binary = *binary.get_or_insert_with(|| is_binary(chunk));

        if is_binary {
            hasher.update(chunk);
            length += read as u64;
            continue;
        }

        normalized.clear();
        normalize_into(chunk, &mut pending_cr, &mut normalized);
        hasher.update(&normalized);
        length += normalized.len() as u64;
    }

    // A trailing CR at end of file becomes a final LF, matching the in-memory
    // normalizer's treatment of a lone CR.
    if pending_cr {
        hasher.update([LF]);
        length += 1;
    }

    Ok(Hashed {
        digest: Digest::from_raw(hasher.finalize().into()),
        length,
    })
}

/// Append `chunk` to `out`, converting CRLF and lone CR to LF.
///
/// `pending_cr` carries a CR that ended the previous chunk, so the conversion is
/// identical no matter where the read boundaries fell.
fn normalize_into(chunk: &[u8], pending_cr: &mut bool, out: &mut Vec<u8>) {
    for &byte in chunk {
        if *pending_cr {
            *pending_cr = false;
            out.push(LF);
            if byte == LF {
                continue;
            }
        }
        if byte == CR {
            *pending_cr = true;
            continue;
        }
        out.push(byte);
    }
}

/// Whether the content looks binary: a NUL byte near the start.
fn is_binary(bytes: &[u8]) -> bool {
    let window = bytes.len().min(BINARY_SNIFF_BYTES);
    bytes.get(..window).is_some_and(|head| head.contains(&0))
}

/// Convert CRLF and lone CR to LF.
fn normalize_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            // Swallow the LF of a CRLF pair; a lone CR becomes LF on its own.
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Map a path to the language name recorded in `parsed_as`.
///
/// This is *recognition*, not support: T2 ships no grammars, so nothing here
/// implies a file can be analyzed for capabilities. `unresolved` entries with
/// reason `unsupported_language` belong to T4, where the rule engine knows which
/// grammars actually exist — emitting them now would attach a reason code to
/// every file in every bundle and mean nothing.
fn language_of(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "md" | "markdown" => "markdown",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" | "jsx" => "javascript",
        "sh" | "bash" | "zsh" => "shell",
        "rb" => "ruby",
        "rs" => "rust",
        "go" => "go",
        "json" => "json",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "txt" | "text" => "text",
        "" => match name {
            "Dockerfile" => "dockerfile",
            "Makefile" => "make",
            _ => "unknown",
        },
        _ => "unknown",
    }
}

/// Assemble inventory entries once load phases are known.
pub fn to_entries(
    files: Vec<WalkedFile>,
    phase_of: impl Fn(&str) -> LoadPhase,
) -> Vec<InventoryEntry> {
    files
        .into_iter()
        .map(|file| InventoryEntry {
            load_phase: phase_of(&file.path),
            path: file.path,
            size: file.size,
            sha256: file.sha256,
            parsed_as: file.parsed_as.to_owned(),
            parse_status: file.parse_status,
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    reason = "slicing a fixed test input at a known offset; a bad index is the test failing. Invariant 10 bans this in library code only."
)]
mod tests {
    use super::*;

    /// Run the streaming normalizer over `input` split at every possible point,
    /// so a CRLF straddling a read boundary is covered rather than assumed.
    fn stream_normalize(input: &[u8]) -> Vec<Vec<u8>> {
        (0..=input.len())
            .map(|split| {
                let mut out = Vec::new();
                let mut pending = false;
                let mut chunk = Vec::new();
                for part in [&input[..split], &input[split..]] {
                    chunk.clear();
                    normalize_into(part, &mut pending, &mut chunk);
                    out.extend_from_slice(&chunk);
                }
                if pending {
                    out.push(LF);
                }
                out
            })
            .collect()
    }

    #[test]
    fn streaming_normalization_matches_the_in_memory_normalizer() {
        // The streaming path only runs for oversized files, so nothing else in
        // the suite exercises it — and an oversized *text* file hashed with its
        // CRLFs intact would give a Windows checkout a different bundle identity.
        for input in [
            "a\r\nb\r\n",
            "a\rb",
            "\r\n",
            "\r",
            "no newlines at all",
            "trailing\r",
            "mixed\r\nand\rlone\n",
        ] {
            let expected = normalize_newlines(input).into_bytes();
            for (split, actual) in stream_normalize(input.as_bytes()).into_iter().enumerate() {
                assert_eq!(
                    actual, expected,
                    "input {input:?} split at byte {split} normalized differently"
                );
            }
        }
    }

    #[test]
    fn normalizes_crlf_and_lone_cr() {
        assert_eq!(normalize_newlines("a\r\nb"), "a\nb");
        assert_eq!(normalize_newlines("a\rb"), "a\nb");
        assert_eq!(normalize_newlines("a\nb"), "a\nb");
        assert_eq!(normalize_newlines("a\r\n\r\nb"), "a\n\nb");
    }

    #[test]
    fn crlf_and_lf_content_hash_identically() {
        let lf = Digest::of(normalize_newlines("line one\nline two\n").as_bytes());
        let crlf = Digest::of(normalize_newlines("line one\r\nline two\r\n").as_bytes());
        assert_eq!(
            lf, crlf,
            "a Windows checkout must not change a file's digest"
        );
    }

    #[test]
    fn detects_binary_by_leading_nul() {
        assert!(is_binary(b"\x7fELF\0\0\0"));
        assert!(!is_binary(b"#!/bin/sh\necho hi\n"));
        assert!(!is_binary(b""));
    }

    #[test]
    fn recognises_languages_by_extension() {
        assert_eq!(language_of("SKILL.md"), "markdown");
        assert_eq!(language_of("scripts/collect.py"), "python");
        assert_eq!(language_of("a/b/run.sh"), "shell");
        assert_eq!(language_of("Dockerfile"), "dockerfile");
        assert_eq!(language_of("vendor/blob.bin"), "unknown");
        assert_eq!(language_of("noextension"), "unknown");
    }
}
