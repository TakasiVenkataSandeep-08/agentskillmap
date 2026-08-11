//! A strict parser for `SKILL.md` frontmatter.
//!
//! # Why this is not a YAML library
//!
//! Frontmatter is the first untrusted bytes this tool touches, in a program
//! whose entire pitch is supply-chain caution. A general YAML engine accepts
//! anchors, aliases, merge keys, tags, and recursive structures — none of which a
//! `SKILL.md` has any reason to contain, and some of which (alias expansion) are
//! a known denial-of-service shape. Pulling one in would mean defending it in
//! `SECURITY.md` forever, and `serde_yaml`, the obvious pick, was archived by its
//! author in 2024.
//!
//! So this parses the documented shape and **refuses everything else out loud**:
//!
//! - `key: scalar`
//! - `key: "quoted"` / `key: 'quoted'`
//! - `key: [a, b]` — flow sequences of scalars
//! - block sequences of scalars under a key
//! - `key: |` and `key: >` — literal and folded block scalars
//! - `# comments` and blank lines
//!
//! Nesting, anchors, aliases, tags, multi-document markers, and anything else
//! produce an [`Error`], which the caller turns into an `unresolved` entry with
//! reason `parse_error`. Invariant 3: refusing loudly is a first-class outcome,
//! and it is strictly better than guessing at a construct we did not expect.
//!
//! If real bundles turn out to need more of YAML, `docs/01-corpus-scan.md` (T3)
//! is what will say so, with a denominator. Widening this on speculation would be
//! the wrong order.

use std::collections::BTreeMap;

/// The delimiter opening and closing a frontmatter block.
const FENCE: &str = "---";

/// A parsed frontmatter block.
///
/// Keys map to [`Value`]; ordering is by key, since a `BTreeMap` is the only
/// map that can feed a byte-identical artifact without an explicit sort at every
/// use site (invariant 2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frontmatter {
    /// Every key in the block, sorted.
    pub entries: BTreeMap<String, Value>,
    /// Byte offset just past the closing fence, where the body begins.
    ///
    /// The instruction plane (T5) scans prose for `instruction.*` signals and
    /// must start here: frontmatter is structured metadata, and lexical patterns
    /// matched against it would be findings about a data block rather than about
    /// anything the agent is told to do.
    pub body_offset: usize,
}

impl Frontmatter {
    /// The value of `key` as a single string, if it is a scalar.
    pub fn scalar(&self, key: &str) -> Option<&str> {
        match self.entries.get(key) {
            Some(Value::Scalar(value)) => Some(value),
            _ => None,
        }
    }

    /// The value of `key` as a list of strings.
    ///
    /// A scalar counts as a one-element list, because agents accept
    /// `allowed-tools: Bash` and `allowed-tools: [Bash]` interchangeably and the
    /// distinction carries no meaning worth preserving downstream.
    pub fn list(&self, key: &str) -> Vec<&str> {
        match self.entries.get(key) {
            Some(Value::Scalar(value)) => vec![value.as_str()],
            Some(Value::List(values)) => values.iter().map(String::as_str).collect(),
            None => Vec::new(),
        }
    }
}

/// A frontmatter value. Deliberately only two shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A single scalar.
    Scalar(String),
    /// A sequence of scalars.
    List(Vec<String>),
}

/// Why a frontmatter block could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// 1-indexed line within the file where the problem is.
    pub line: u64,
    /// What went wrong, phrased for a bundle author reading a CI failure.
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

/// Whether `text` opens with a frontmatter fence.
pub fn has_frontmatter(text: &str) -> bool {
    text.lines()
        .next()
        .is_some_and(|line| line.trim_end() == FENCE)
}

/// Parse the frontmatter block at the start of `text`.
///
/// # Errors
///
/// [`Error`] if there is no opening fence, no closing fence, or any construct
/// outside the supported subset documented on this module.
pub fn parse(text: &str) -> Result<Frontmatter, Error> {
    let mut lines = NumberedLines::new(text);

    let Some((first_no, first)) = lines.next() else {
        return Err(Error {
            line: 1,
            message: "file is empty; expected a `---` frontmatter block".to_owned(),
        });
    };
    if first.trim_end() != FENCE {
        return Err(Error {
            line: first_no,
            message: format!("expected `{FENCE}` on the first line to open the frontmatter block"),
        });
    }

    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    let mut pending_list: Option<(String, Vec<String>)> = None;

    while let Some((line_no, line)) = lines.next() {
        if line.trim_end() == FENCE {
            if let Some((key, values)) = pending_list.take() {
                insert(&mut entries, key, Value::List(values), line_no)?;
            }
            return Ok(Frontmatter {
                entries,
                body_offset: lines.consumed(),
            });
        }

        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        reject_unsupported(trimmed, line_no)?;

        // A block-sequence item continues whatever key opened the sequence.
        if let Some(item) = trimmed.strip_prefix("- ").or_else(|| {
            // A bare `-` is an empty item, which is legal YAML and meaningless here.
            (trimmed == "-").then_some("")
        }) {
            let Some((_, values)) = pending_list.as_mut() else {
                return Err(Error {
                    line: line_no,
                    message: "list item `- ...` does not follow a `key:` line".to_owned(),
                });
            };
            values.push(unquote(item.trim()));
            continue;
        }

        if let Some((key, values)) = pending_list.take() {
            insert(&mut entries, key, Value::List(values), line_no)?;
        }

        let Some((key, rest)) = split_key(trimmed) else {
            return Err(Error {
                line: line_no,
                message: format!(
                    "expected `key: value`, a `- item`, or `{FENCE}`, found {trimmed:?}"
                ),
            });
        };

        if key.is_empty() {
            return Err(Error {
                line: line_no,
                message: "empty key".to_owned(),
            });
        }

        let rest = rest.trim();
        match rest {
            // `key:` with nothing after it opens a block sequence, or is an empty
            // value if no `- item` follows.
            "" => pending_list = Some((key.to_owned(), Vec::new())),
            "|" | ">" | "|-" | ">-" | "|+" | ">+" => {
                let (block, ended) = read_block_scalar(&mut lines, rest.starts_with('>'));
                if !ended {
                    return Err(Error {
                        line: line_no,
                        message: format!("block scalar for `{key}` is not closed by `{FENCE}`"),
                    });
                }
                insert(&mut entries, key.to_owned(), Value::Scalar(block), line_no)?;
            }
            _ if rest.starts_with('[') => {
                let list = parse_flow_sequence(rest, line_no)?;
                insert(&mut entries, key.to_owned(), Value::List(list), line_no)?;
            }
            _ if rest.starts_with('{') => {
                return Err(Error {
                    line: line_no,
                    message: format!(
                        "`{key}` uses a flow mapping; nested structures are not supported \
                         in SKILL.md frontmatter"
                    ),
                })
            }
            _ => insert(
                &mut entries,
                key.to_owned(),
                Value::Scalar(unquote(strip_comment(rest))),
                line_no,
            )?,
        }
    }

    Err(Error {
        line: 1,
        message: format!("frontmatter block is never closed by `{FENCE}`"),
    })
}

/// Reject YAML features this parser deliberately does not implement.
///
/// Named separately so the refusal list is readable as a list, and so each entry
/// can say *why* rather than just failing.
fn reject_unsupported(trimmed: &str, line: u64) -> Result<(), Error> {
    let reject = |message: String| Err(Error { line, message });

    if trimmed.starts_with('&') {
        return reject("YAML anchors (`&name`) are not supported".to_owned());
    }
    if trimmed.starts_with('*') {
        return reject(
            "YAML aliases (`*name`) are not supported: alias expansion is a \
             denial-of-service shape and no SKILL.md needs it"
                .to_owned(),
        );
    }
    if trimmed.starts_with("<<:") {
        return reject("YAML merge keys (`<<:`) are not supported".to_owned());
    }
    if trimmed.starts_with("%") {
        return reject("YAML directives (`%YAML`, `%TAG`) are not supported".to_owned());
    }
    if trimmed == "..." {
        return reject("YAML document end markers are not supported".to_owned());
    }
    Ok(())
}

/// Insert a key, rejecting duplicates.
///
/// A duplicate key is ambiguous — YAML engines disagree on whether first or last
/// wins — and ambiguity in the frontmatter of a security artifact is exactly the
/// kind of thing worth refusing rather than silently resolving.
fn insert(
    entries: &mut BTreeMap<String, Value>,
    key: String,
    value: Value,
    line: u64,
) -> Result<(), Error> {
    if entries.contains_key(&key) {
        return Err(Error {
            line,
            message: format!("duplicate key `{key}`: which value wins is engine-dependent"),
        });
    }
    entries.insert(key, value);
    Ok(())
}

/// Split `key: rest` at the first colon that is not inside quotes.
fn split_key(line: &str) -> Option<(&str, &str)> {
    let mut quote: Option<char> = None;
    for (index, ch) in line.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(open), ch) if ch == open => quote = None,
            (None, ':') => {
                let (key, rest) = line.split_at(index);
                // `rest` starts with the colon.
                let rest = rest.get(1..)?;
                // `key:value` without a space is not `key: value` in YAML unless
                // the value is quoted or the colon ends the line, but every real
                // SKILL.md writes the space. Accept both; the ambiguity YAML
                // worries about (URLs like `http://x`) is handled by requiring
                // the colon to be followed by a space or end-of-line.
                if rest.is_empty() || rest.starts_with(' ') {
                    return Some((key.trim(), rest));
                }
                return None;
            }
            _ => {}
        }
    }
    None
}

/// Strip a trailing ` # comment` from an unquoted scalar.
fn strip_comment(value: &str) -> &str {
    if value.starts_with('"') || value.starts_with('\'') {
        return value;
    }
    match value.find(" #") {
        Some(index) => value.get(..index).unwrap_or(value).trim_end(),
        None => value,
    }
}

/// Remove one matching pair of surrounding quotes, if present.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes.first().copied(), bytes.last().copied());
        if (first == Some(b'"') && last == Some(b'"'))
            || (first == Some(b'\'') && last == Some(b'\''))
        {
            if let Some(inner) = value.get(1..value.len() - 1) {
                return inner.to_owned();
            }
        }
    }
    value.to_owned()
}

/// Parse `[a, b, "c"]` into its elements.
fn parse_flow_sequence(text: &str, line: u64) -> Result<Vec<String>, Error> {
    let inner = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| Error {
            line,
            message: "flow sequence is not closed on the same line by `]`".to_owned(),
        })?;

    if inner.contains('[') || inner.contains('{') {
        return Err(Error {
            line,
            message: "nested flow collections are not supported in SKILL.md frontmatter".to_owned(),
        });
    }

    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(inner.split(',').map(|item| unquote(item.trim())).collect())
}

/// Read the indented body of a `|` or `>` block scalar.
///
/// Returns the joined text and whether the closing fence was reached. Lines are
/// joined with `\n` for literal blocks and with a space for folded ones, then
/// trailing whitespace is trimmed — the `-`/`+` chomping indicators are accepted
/// but not distinguished, because nothing downstream depends on a trailing
/// newline in a description.
fn read_block_scalar(lines: &mut NumberedLines<'_>, folded: bool) -> (String, bool) {
    let mut collected: Vec<&str> = Vec::new();
    while let Some((_, line)) = lines.peek() {
        let trimmed = line.trim();
        if trimmed == FENCE {
            let joined = if folded {
                collected.join(" ")
            } else {
                collected.join("\n")
            };
            return (joined.trim().to_owned(), true);
        }
        // Any indented line, or a blank one, belongs to the block.
        if !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        lines.next();
        collected.push(trimmed);
    }

    let joined = if folded {
        collected.join(" ")
    } else {
        collected.join("\n")
    };
    // Not terminated by a fence; the caller reports it.
    (joined.trim().to_owned(), false)
}

/// A line iterator that tracks 1-indexed line numbers and bytes consumed.
///
/// Hand-rolled rather than `str::lines` so `body_offset` can be exact: the caller
/// needs to know where the body starts in **bytes**, and `lines()` discards the
/// terminator lengths that would let anyone reconstruct that.
struct NumberedLines<'a> {
    text: &'a str,
    offset: usize,
    line: u64,
}

impl<'a> NumberedLines<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            offset: 0,
            line: 0,
        }
    }

    /// Bytes consumed so far, i.e. the offset just past the last line returned.
    fn consumed(&self) -> usize {
        self.offset
    }

    fn peek(&self) -> Option<(u64, &'a str)> {
        let rest = self.text.get(self.offset..)?;
        if rest.is_empty() {
            return None;
        }
        let end = rest.find('\n').unwrap_or(rest.len());
        let line = rest.get(..end)?;
        Some((self.line + 1, line.strip_suffix('\r').unwrap_or(line)))
    }
}

impl<'a> Iterator for NumberedLines<'a> {
    type Item = (u64, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.text.get(self.offset..)?;
        if rest.is_empty() {
            return None;
        }
        let (line, advance) = match rest.find('\n') {
            Some(index) => (rest.get(..index)?, index + 1),
            None => (rest, rest.len()),
        };
        self.offset += advance;
        self.line += 1;
        Some((self.line, line.strip_suffix('\r').unwrap_or(line)))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_ordinary_shape() {
        let text = "---\nname: pdf-tools\ndescription: Fills in PDF forms.\n---\n# Body\n";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("name"), Some("pdf-tools"));
        assert_eq!(front.scalar("description"), Some("Fills in PDF forms."));
        assert_eq!(text.get(front.body_offset..), Some("# Body\n"));
    }

    #[test]
    fn parses_quotes_comments_and_flow_sequences() {
        let text = "---\n# leading comment\nname: \"quoted name\"\nother: 'single'\n\
                    allowed-tools: [Bash, \"Read\"]\nempty: []\nnote: plain # trailing\n---\n";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("name"), Some("quoted name"));
        assert_eq!(front.scalar("other"), Some("single"));
        assert_eq!(front.list("allowed-tools"), vec!["Bash", "Read"]);
        assert!(front.list("empty").is_empty());
        assert_eq!(front.scalar("note"), Some("plain"));
    }

    #[test]
    fn parses_block_sequences_and_block_scalars() {
        let text = "---\ntools:\n  - Bash\n  - Read\ndescription: |\n  First line.\n  \
                    Second line.\n---\n";
        let front = parse(text).unwrap();
        assert_eq!(front.list("tools"), vec!["Bash", "Read"]);
        assert_eq!(
            front.scalar("description"),
            Some("First line.\nSecond line.")
        );
    }

    #[test]
    fn folded_block_scalars_join_with_spaces() {
        let text = "---\ndescription: >\n  One\n  Two\n---\n";
        assert_eq!(parse(text).unwrap().scalar("description"), Some("One Two"));
    }

    #[test]
    fn a_scalar_reads_as_a_one_element_list() {
        let text = "---\nallowed-tools: Bash\n---\n";
        assert_eq!(parse(text).unwrap().list("allowed-tools"), vec!["Bash"]);
    }

    #[test]
    fn crlf_frontmatter_parses_identically_to_lf() {
        let lf = parse("---\nname: demo\n---\nbody\n").unwrap();
        let crlf = parse("---\r\nname: demo\r\n---\r\nbody\r\n").unwrap();
        assert_eq!(lf.entries, crlf.entries);
    }

    #[test]
    fn refuses_yaml_features_it_does_not_implement() {
        // Each of these is a construct a general engine would happily accept and
        // this parser must reject out loud rather than guess at.
        let cases = [
            ("no fence", "name: demo\n"),
            ("unclosed block", "---\nname: demo\n"),
            ("anchor", "---\n&anchor\nname: demo\n---\n"),
            ("alias", "---\n*alias\n---\n"),
            ("merge key", "---\n<<: *defaults\n---\n"),
            ("directive", "---\n%YAML 1.2\n---\n"),
            ("document end", "---\nname: a\n...\n"),
            ("flow mapping", "---\nmeta: {a: 1}\n---\n"),
            ("nested flow", "---\nmeta: [[a]]\n---\n"),
            ("duplicate key", "---\nname: a\nname: b\n---\n"),
            ("orphan list item", "---\n- Bash\n---\n"),
            ("unclosed flow sequence", "---\ntools: [Bash\n---\n"),
            ("empty key", "---\n: value\n---\n"),
            ("bare text", "---\njust some prose\n---\n"),
        ];
        for (label, text) in cases {
            assert!(
                parse(text).is_err(),
                "{label}: should have been refused, not guessed at"
            );
        }
    }

    #[test]
    fn errors_carry_a_line_number() {
        let err = parse("---\nname: a\nname: b\n---\n").unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.to_string().contains("duplicate key"));
    }

    #[test]
    fn detects_a_frontmatter_fence() {
        assert!(has_frontmatter("---\nname: x\n---\n"));
        assert!(has_frontmatter("---\r\nname: x\r\n---\r\n"));
        assert!(!has_frontmatter("# Just a heading\n"));
        assert!(!has_frontmatter(""));
    }

    #[test]
    fn a_colon_inside_a_value_is_not_a_key_separator() {
        let text = "---\nname: demo\nhomepage: \"https://example.com/x\"\n---\n";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("homepage"), Some("https://example.com/x"));
    }
}
