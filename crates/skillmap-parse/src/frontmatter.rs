//! A strict parser for `SKILL.md` frontmatter.
//!
//! # Why this is not a YAML library
//!
//! Frontmatter is the first untrusted bytes this tool touches, in a program
//! whose entire pitch is supply-chain caution. Pulling in a general engine would
//! mean defending it in `SECURITY.md` forever, and `serde_yaml`, the obvious
//! pick, was archived by its author in 2024.
//!
//! # What the corpus changed
//!
//! T2 shipped a much narrower subset and deferred the question *"is refusing
//! non-subset YAML tenable?"* to T3's harvest. The answer came back **28%
//! refused** across 34,284 real bundles, and a survey of the failures
//! (`examples/frontmatter-survey.rs`) put the blame overwhelmingly on **nested
//! block mappings** — `metadata:` with sub-keys, `compatibility:`, `env:` — with
//! flow mappings and sequences-of-mappings behind them. On the curated head
//! alone the refusal rate had looked like 0.1%, which is exactly the sampling
//! trap `docs/01-corpus-scan.md` warns about.
//!
//! So the subset widened, along a line that keeps the original argument intact:
//!
//! **Nesting is data. Anchors and aliases are indirection.** A nested map is
//! ordinary structure with a size bounded by the file; an alias is a reference
//! that expands, and repeated expansion is the billion-laughs denial-of-service
//! shape. Depth is capped at [`MAX_DEPTH`] so the first half of that stays true
//! no matter what a bundle contains.
//!
//! Supported:
//!
//! - `key: scalar`, quoted or bare
//! - nested block mappings, to [`MAX_DEPTH`]
//! - block sequences, at either indentation style, of scalars or of mappings
//! - flow sequences `[a, b]` and flow mappings `{a: b}`
//! - block scalars `|` and `>`
//! - `# comments` and blank lines
//!
//! Refused, with a line number:
//!
//! - anchors `&a`, aliases `*a`, merge keys `<<:` — the DoS shapes
//! - `%YAML` directives and `...` document-end markers
//! - tab indentation, which YAML forbids and which silently changes structure
//! - duplicate keys, where engines disagree about which value wins
//! - nesting deeper than [`MAX_DEPTH`]
//!
//! Refusing loudly stays a first-class outcome (invariant 3): the caller turns an
//! [`Error`] into an `unresolved` entry with reason `parse_error`, and the bundle
//! is still inventoried and hashed.

use std::collections::BTreeMap;

/// The delimiter opening and closing a frontmatter block.
const FENCE: &str = "---";

/// How deep nesting may go before the parser refuses.
///
/// Bounded so "this cannot be made to expand without limit" remains true. Nothing
/// in the corpus came close: the deepest real frontmatter observed was three
/// levels, and this leaves room for structures nobody has written yet without
/// leaving the door open to a pathological one.
pub const MAX_DEPTH: usize = 8;

/// A parsed frontmatter block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frontmatter {
    /// Every top-level key, sorted.
    pub entries: BTreeMap<String, Value>,
    /// Byte offset just past the closing fence, where the body begins.
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
    /// `allowed-tools: Bash` and `allowed-tools: [Bash]` interchangeably. Nested
    /// maps inside a list contribute nothing: callers of this method want tool
    /// names, and flattening a map into one would invent a string the author
    /// never wrote.
    pub fn list(&self, key: &str) -> Vec<&str> {
        match self.entries.get(key) {
            Some(Value::Scalar(value)) => vec![value.as_str()],
            Some(Value::List(values)) => values
                .iter()
                .filter_map(|value| match value {
                    Value::Scalar(scalar) => Some(scalar.as_str()),
                    Value::List(_) | Value::Map(_) => None,
                })
                .collect(),
            Some(Value::Map(_)) | None => Vec::new(),
        }
    }
}

/// A frontmatter value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A single scalar.
    Scalar(String),
    /// A sequence.
    List(Vec<Value>),
    /// A nested mapping.
    Map(BTreeMap<String, Value>),
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

/// One physical line of the block.
struct Row {
    number: u64,
    indent: usize,
    /// Trimmed content. Empty for blank lines.
    content: String,
    /// The line with its indentation intact, for block scalars.
    raw: String,
}

/// A cursor over the block's lines.
struct Rows {
    rows: Vec<Row>,
    at: usize,
}

impl Rows {
    /// The next row that carries content, skipping blanks and comments.
    fn peek_significant(&self) -> Option<&Row> {
        self.rows
            .get(self.at..)?
            .iter()
            .find(|row| !row.content.is_empty() && !row.content.starts_with('#'))
    }

    /// Advance past blanks and comments.
    fn skip_insignificant(&mut self) {
        while let Some(row) = self.rows.get(self.at) {
            if row.content.is_empty() || row.content.starts_with('#') {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    /// The line number to blame when input ran out.
    fn last_line(&self) -> u64 {
        self.rows.last().map_or(1, |row| row.number)
    }
}

/// Parse the frontmatter block at the start of `text`.
///
/// # Errors
///
/// [`Error`] if there is no opening fence, no closing fence, or any construct
/// outside the supported subset documented on this module.
pub fn parse(text: &str) -> Result<Frontmatter, Error> {
    let (rows, body_offset) = collect_block(text)?;
    let mut rows = Rows { rows, at: 0 };
    let entries = parse_map(&mut rows, 0, 1)?;

    // Anything left is content the map parser declined to consume, which means it
    // sat at an indentation that does not belong to any key.
    rows.skip_insignificant();
    if let Some(row) = rows.rows.get(rows.at) {
        return Err(Error {
            line: row.number,
            message: format!(
                "unexpected indentation; {:?} belongs to no key",
                row.content
            ),
        });
    }

    Ok(Frontmatter {
        entries,
        body_offset,
    })
}

/// Split the frontmatter block off the front of `text`.
fn collect_block(text: &str) -> Result<(Vec<Row>, usize), Error> {
    let mut rows = Vec::new();
    let mut offset = 0usize;
    let mut number = 0u64;
    let mut opened = false;

    for line in text.split_inclusive('\n') {
        number += 1;
        offset += line.len();
        let stripped = line.strip_suffix('\n').unwrap_or(line);
        let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);

        if !opened {
            if stripped.trim_end() != FENCE {
                return Err(Error {
                    line: number,
                    message: format!(
                        "expected `{FENCE}` on the first line to open the frontmatter block"
                    ),
                });
            }
            opened = true;
            continue;
        }

        if stripped.trim_end() == FENCE {
            return Ok((rows, offset));
        }

        let indent = stripped.len() - stripped.trim_start_matches(' ').len();
        if stripped.starts_with('\t') || stripped[..indent].contains('\t') {
            return Err(Error {
                line: number,
                message: "tab indentation is not valid YAML and silently changes structure"
                    .to_owned(),
            });
        }

        rows.push(Row {
            number,
            indent,
            content: stripped.trim().to_owned(),
            raw: stripped.to_owned(),
        });
    }

    Err(Error {
        line: if opened { number } else { 1 },
        message: if opened {
            format!("frontmatter block is never closed by `{FENCE}`")
        } else {
            "file is empty; expected a `---` frontmatter block".to_owned()
        },
    })
}

/// Parse a mapping whose keys sit at `indent`.
fn parse_map(
    rows: &mut Rows,
    indent: usize,
    depth: usize,
) -> Result<BTreeMap<String, Value>, Error> {
    check_depth(depth, rows.peek_significant().map_or(1, |row| row.number))?;
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();

    loop {
        rows.skip_insignificant();
        let Some(row) = rows.rows.get(rows.at) else {
            break;
        };
        if row.indent < indent {
            break;
        }
        if row.indent > indent {
            return Err(Error {
                line: row.number,
                message: format!(
                    "unexpected indentation; {:?} belongs to no key",
                    row.content
                ),
            });
        }

        let (number, content) = (row.number, row.content.clone());
        reject_unsupported(&content, number)?;

        if content.starts_with("- ") || content == "-" {
            return Err(Error {
                line: number,
                message: "list item `- ...` does not follow a `key:` line".to_owned(),
            });
        }

        let Some((key, rest)) = split_key(&content) else {
            return Err(Error {
                line: number,
                message: format!("expected `key: value`, `key:`, or `{FENCE}`, found {content:?}"),
            });
        };
        if key.is_empty() {
            return Err(Error {
                line: number,
                message: "empty key".to_owned(),
            });
        }
        let key = unquote(key);

        rows.at += 1;
        let value = parse_value(rows, indent, depth, rest.trim(), number)?;

        if entries.insert(key.clone(), value).is_some() {
            return Err(Error {
                line: number,
                message: format!("duplicate key `{key}`: which value wins is engine-dependent"),
            });
        }
    }

    Ok(entries)
}

/// Parse whatever follows `key:` on its line, and any block beneath it.
fn parse_value(
    rows: &mut Rows,
    indent: usize,
    depth: usize,
    inline: &str,
    number: u64,
) -> Result<Value, Error> {
    // Indirection can appear as a value as well as at the start of a line —
    // `name: *alias`, `base: &anchor` — and only the line-start form was caught
    // before. A refusal that depends on where the construct sits is not a refusal.
    if inline.starts_with('*') || inline.starts_with('&') {
        return Err(Error {
            line: number,
            message: "YAML anchors and aliases are not supported: an alias exists to                       be expanded, and repeated expansion is the denial-of-service                       shape this parser refuses"
                .to_owned(),
        });
    }

    match inline {
        // A block scalar: everything more indented, joined.
        "|" | ">" | "|-" | ">-" | "|+" | ">+" => Ok(Value::Scalar(read_block_scalar(
            rows,
            indent,
            inline.starts_with('>'),
        ))),

        // Nothing on the line: the value is a nested block, or empty.
        "" => {
            let Some(next) = rows.peek_significant() else {
                return Ok(Value::Scalar(String::new()));
            };
            // A sequence may sit at the key's own indent or deeper; a mapping must
            // be deeper. Both spellings are common and mean the same thing.
            if next.content.starts_with("- ") || next.content == "-" {
                if next.indent >= indent {
                    let at = next.indent;
                    return Ok(Value::List(parse_sequence(rows, at, depth + 1)?));
                }
            } else if next.indent > indent {
                // More-indented lines are a nested mapping if they are keys, and a
                // multi-line plain scalar if they are prose.
                if split_key(&next.content).is_some_and(|(key, _)| !key.is_empty()) {
                    let at = next.indent;
                    return Ok(Value::Map(parse_map(rows, at, depth + 1)?));
                }
                return Ok(Value::Scalar(read_continuation(rows, indent).join(" ")));
            }
            Ok(Value::Scalar(String::new()))
        }

        _ if inline.starts_with('[') => Ok(Value::List(parse_flow_sequence(inline, number)?)),
        _ if inline.starts_with('{') => Ok(Value::Map(parse_flow_map(inline, number, depth)?)),

        // A plain scalar may continue on more-indented lines. This is how almost
        // every long `description:` in the corpus is written, and refusing it
        // accounted for the largest remaining class of failures after nesting was
        // supported — the parser was reading a wrapped sentence as a structural
        // error.
        //
        // A continuation is any more-indented line that is not itself a `key:` and
        // not a list item. That distinction is what keeps a genuine nested mapping
        // from being swallowed into the sentence above it.
        _ => {
            let mut parts = vec![unquote(strip_comment(inline))];
            parts.extend(read_continuation(rows, indent));
            Ok(Value::Scalar(parts.join(" ").trim().to_owned()))
        }
    }
}

/// Consume the more-indented lines continuing a plain scalar.
fn read_continuation(rows: &mut Rows, indent: usize) -> Vec<String> {
    let mut parts = Vec::new();
    while let Some(row) = rows.rows.get(rows.at) {
        if row.content.is_empty() || row.content.starts_with('#') || row.indent <= indent {
            break;
        }
        // A key or a list item is structure, not prose.
        if row.content.starts_with("- ")
            || row.content == "-"
            || split_key(&row.content).is_some_and(|(key, _)| !key.is_empty())
        {
            break;
        }
        parts.push(row.content.clone());
        rows.at += 1;
    }
    parts
}

/// Parse a block sequence whose `-` markers sit at `indent`.
fn parse_sequence(rows: &mut Rows, indent: usize, depth: usize) -> Result<Vec<Value>, Error> {
    check_depth(depth, rows.peek_significant().map_or(1, |row| row.number))?;
    let mut items = Vec::new();

    loop {
        rows.skip_insignificant();
        let Some(row) = rows.rows.get(rows.at) else {
            break;
        };
        if row.indent != indent || !(row.content.starts_with("- ") || row.content == "-") {
            break;
        }

        let number = row.number;
        let rest = row
            .content
            .strip_prefix("- ")
            .unwrap_or("")
            .trim()
            .to_owned();
        reject_unsupported(&rest, number)?;
        rows.at += 1;

        // `- key: value` opens a mapping that continues on more-indented lines.
        if let Some((key, tail)) = split_key(&rest) {
            if !key.is_empty() {
                let mut map: BTreeMap<String, Value> = BTreeMap::new();
                let key = unquote(key);
                let value = parse_value(rows, indent + 2, depth, tail.trim(), number)?;
                map.insert(key, value);

                // A list item's mapping continues on lines indented past the `-`.
                rows.skip_insignificant();
                let continues = rows
                    .rows
                    .get(rows.at)
                    .is_some_and(|next| next.indent > indent && !next.content.starts_with("- "));
                if continues {
                    let at = rows
                        .rows
                        .get(rows.at)
                        .map_or(indent + 2, |next| next.indent);
                    let line = next_number(rows);
                    for (key, value) in parse_map(rows, at, depth + 1)? {
                        if map.insert(key.clone(), value).is_some() {
                            return Err(Error {
                                line,
                                message: format!("duplicate key `{key}` in list item"),
                            });
                        }
                    }
                }
                items.push(Value::Map(map));
                continue;
            }
        }

        if rest.is_empty() {
            // A bare `-` introduces a nested block.
            rows.skip_insignificant();
            if let Some(next) = rows.rows.get(rows.at) {
                if next.indent > indent {
                    let at = next.indent;
                    items.push(Value::Map(parse_map(rows, at, depth + 1)?));
                    continue;
                }
            }
            items.push(Value::Scalar(String::new()));
            continue;
        }

        if rest.starts_with('[') {
            items.push(Value::List(parse_flow_sequence(&rest, number)?));
        } else if rest.starts_with('{') {
            items.push(Value::Map(parse_flow_map(&rest, number, depth)?));
        } else {
            items.push(Value::Scalar(unquote(strip_comment(&rest))));
        }
    }

    Ok(items)
}

/// The line number of the row under the cursor, for error messages.
fn next_number(rows: &Rows) -> u64 {
    rows.rows
        .get(rows.at)
        .map_or_else(|| rows.last_line(), |row| row.number)
}

/// Refuse the constructs this parser deliberately does not implement.
fn reject_unsupported(content: &str, line: u64) -> Result<(), Error> {
    let reject = |message: String| Err(Error { line, message });

    if content.starts_with('&') {
        return reject(
            "YAML anchors (`&name`) are not supported: an anchor exists to be \
             referenced, and reference expansion is the shape this parser refuses"
                .to_owned(),
        );
    }
    if content.starts_with('*') {
        return reject(
            "YAML aliases (`*name`) are not supported: alias expansion is a \
             denial-of-service shape and no SKILL.md needs it"
                .to_owned(),
        );
    }
    if content.starts_with("<<:") {
        return reject("YAML merge keys (`<<:`) are not supported".to_owned());
    }
    if content.starts_with('%') {
        return reject("YAML directives (`%YAML`, `%TAG`) are not supported".to_owned());
    }
    if content == "..." {
        return reject("YAML document end markers are not supported".to_owned());
    }
    Ok(())
}

/// Refuse nesting past the cap.
fn check_depth(depth: usize, line: u64) -> Result<(), Error> {
    if depth > MAX_DEPTH {
        return Err(Error {
            line,
            message: format!(
                "frontmatter nests deeper than {MAX_DEPTH} levels; the cap is what \
                 keeps \"this cannot expand without limit\" true"
            ),
        });
    }
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
                let rest = rest.get(1..)?;
                // Requiring a space or end-of-line after the colon is what keeps
                // `homepage: https://x` from splitting at the URL's own colon.
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

/// Split a flow collection's body on commas that are not inside quotes or nested
/// brackets.
fn split_flow(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut nesting = 0usize;

    for ch in inner.chars() {
        match (quote, ch) {
            (None, '"' | '\'') => {
                quote = Some(ch);
                current.push(ch);
            }
            (Some(open), ch) if ch == open => {
                quote = None;
                current.push(ch);
            }
            (None, '[' | '{') => {
                nesting += 1;
                current.push(ch);
            }
            (None, ']' | '}') => {
                nesting = nesting.saturating_sub(1);
                current.push(ch);
            }
            (None, ',') if nesting == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Parse `[a, b, "c"]`.
fn parse_flow_sequence(text: &str, line: u64) -> Result<Vec<Value>, Error> {
    let inner = text
        .strip_prefix('[')
        .and_then(|rest| rest.trim_end().strip_suffix(']'))
        .ok_or_else(|| Error {
            line,
            message: "flow sequence is not closed on the same line by `]`".to_owned(),
        })?;

    Ok(split_flow(inner)
        .into_iter()
        .map(|item| Value::Scalar(unquote(item.trim())))
        .collect())
}

/// Parse `{a: 1, b: two}`.
fn parse_flow_map(text: &str, line: u64, depth: usize) -> Result<BTreeMap<String, Value>, Error> {
    check_depth(depth + 1, line)?;
    let inner = text
        .strip_prefix('{')
        .and_then(|rest| rest.trim_end().strip_suffix('}'))
        .ok_or_else(|| Error {
            line,
            message: "flow mapping is not closed on the same line by `}`".to_owned(),
        })?;

    let mut map = BTreeMap::new();
    for pair in split_flow(inner) {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once(':').ok_or_else(|| Error {
            line,
            message: format!("flow mapping entry {pair:?} is not `key: value`"),
        })?;
        map.insert(unquote(key.trim()), Value::Scalar(unquote(value.trim())));
    }
    Ok(map)
}

/// Read the indented body of a `|` or `>` block scalar.
fn read_block_scalar(rows: &mut Rows, indent: usize, folded: bool) -> String {
    let mut collected: Vec<String> = Vec::new();
    while let Some(row) = rows.rows.get(rows.at) {
        if !row.content.is_empty() && row.indent <= indent {
            break;
        }
        collected.push(row.raw.trim().to_owned());
        rows.at += 1;
    }

    let joined = if folded {
        collected.join(" ")
    } else {
        collected.join("\n")
    };
    joined.trim().to_owned()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed unwrap, panic, or index in a test is the test failing.               Invariant 10 bans these in library code, where hostile input is the               normal case; asserting on a parsed shape is not that."
)]
mod tests {
    use super::*;

    fn scalar(value: &str) -> Value {
        Value::Scalar(value.to_owned())
    }

    #[test]
    fn parses_the_ordinary_shape() {
        let text = "---\nname: pdf-tools\ndescription: Fills in PDF forms.\n---\n# Body\n";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("name"), Some("pdf-tools"));
        assert_eq!(front.scalar("description"), Some("Fills in PDF forms."));
        assert_eq!(text.get(front.body_offset..), Some("# Body\n"));
    }

    #[test]
    fn parses_nested_block_mappings() {
        // The construct that caused 28% of real bundles to be refused.
        let text = "---\nname: demo\nmetadata:\n  author: someone\n  version: 2\n---\n";
        let front = parse(text).unwrap();
        let Some(Value::Map(metadata)) = front.entries.get("metadata") else {
            panic!(
                "expected a nested map, got {:?}",
                front.entries.get("metadata")
            );
        };
        assert_eq!(metadata.get("author"), Some(&scalar("someone")));
        assert_eq!(metadata.get("version"), Some(&scalar("2")));
    }

    #[test]
    fn parses_deeply_nested_mappings() {
        let text = "---\na:\n  b:\n    c:\n      d: deep\n---\n";
        let front = parse(text).unwrap();
        let mut cursor = front.entries.get("a");
        for key in ["b", "c"] {
            let Some(Value::Map(map)) = cursor else {
                panic!("expected a map at {key}");
            };
            cursor = map.get(key);
        }
        let Some(Value::Map(inner)) = cursor else {
            panic!("expected the innermost map");
        };
        assert_eq!(inner.get("d"), Some(&scalar("deep")));
    }

    #[test]
    fn parses_sequences_at_either_indentation() {
        let flush = parse("---\ntools:\n- Bash\n- Read\n---\n").unwrap();
        let indented = parse("---\ntools:\n  - Bash\n  - Read\n---\n").unwrap();
        assert_eq!(flush.list("tools"), vec!["Bash", "Read"]);
        assert_eq!(
            indented.list("tools"),
            vec!["Bash", "Read"],
            "both spellings are common and mean the same thing"
        );
    }

    #[test]
    fn parses_a_sequence_of_mappings() {
        let text = "---\nexamples:\n  - name: first\n    run: a.py\n  - name: second\n---\n";
        let front = parse(text).unwrap();
        let Some(Value::List(items)) = front.entries.get("examples") else {
            panic!("expected a list");
        };
        assert_eq!(items.len(), 2);
        let Value::Map(first) = &items[0] else {
            panic!("expected a mapping item");
        };
        assert_eq!(first.get("name"), Some(&scalar("first")));
        assert_eq!(first.get("run"), Some(&scalar("a.py")));
    }

    #[test]
    fn parses_flow_mappings_and_sequences() {
        let text = "---\nmeta: {a: 1, b: two}\ntags: [x, \"y\"]\n---\n";
        let front = parse(text).unwrap();
        let Some(Value::Map(meta)) = front.entries.get("meta") else {
            panic!("expected a flow map");
        };
        assert_eq!(meta.get("a"), Some(&scalar("1")));
        assert_eq!(meta.get("b"), Some(&scalar("two")));
        assert_eq!(front.list("tags"), vec!["x", "y"]);
    }

    #[test]
    fn a_flow_sequence_of_maps_does_not_split_inside_braces() {
        let front = parse("---\nitems: [{a: 1, b: 2}, plain]\n---\n").unwrap();
        let Some(Value::List(items)) = front.entries.get("items") else {
            panic!("expected a list");
        };
        assert_eq!(
            items.len(),
            2,
            "the comma inside the braces is not a separator"
        );
    }

    #[test]
    fn parses_quotes_comments_and_block_scalars() {
        let text = "---\n# leading comment\nname: \"quoted name\"\nother: 'single'\n\
                    note: plain # trailing\ndescription: |\n  First line.\n  Second line.\n---\n";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("name"), Some("quoted name"));
        assert_eq!(front.scalar("other"), Some("single"));
        assert_eq!(front.scalar("note"), Some("plain"));
        assert_eq!(
            front.scalar("description"),
            Some("First line.\nSecond line.")
        );
    }

    #[test]
    fn a_plain_scalar_continues_across_indented_lines() {
        // The commonest long `description:` shape in the corpus, and the largest
        // remaining failure class once nesting was supported.
        let text = "---
name: demo
description: Manages tasks by
                      organizing them into checklists. Use when asked.
other: x
---
";
        let front = parse(text).unwrap();
        assert_eq!(
            front.scalar("description"),
            Some("Manages tasks by organizing them into checklists. Use when asked.")
        );
        assert_eq!(
            front.scalar("other"),
            Some("x"),
            "the next key still parses"
        );
    }

    #[test]
    fn a_continuation_does_not_swallow_a_nested_mapping() {
        // The risk of supporting continuations: prose and structure both sit at a
        // deeper indent, and only the `key:` shape tells them apart.
        let text = "---
description: short
metadata:
  author: someone
---
";
        let front = parse(text).unwrap();
        assert_eq!(front.scalar("description"), Some("short"));
        let Some(Value::Map(metadata)) = front.entries.get("metadata") else {
            panic!("metadata must stay a mapping, not become part of the sentence");
        };
        assert_eq!(metadata.get("author"), Some(&scalar("someone")));
    }

    #[test]
    fn folded_block_scalars_join_with_spaces() {
        let text = "---\ndescription: >\n  One\n  Two\n---\n";
        assert_eq!(parse(text).unwrap().scalar("description"), Some("One Two"));
    }

    #[test]
    fn a_scalar_reads_as_a_one_element_list() {
        assert_eq!(
            parse("---\nallowed-tools: Bash\n---\n")
                .unwrap()
                .list("allowed-tools"),
            vec!["Bash"]
        );
    }

    #[test]
    fn a_colon_inside_a_value_is_not_a_key_separator() {
        let front = parse("---\nhomepage: \"https://example.com/x\"\nbare: https://x.dev/y\n---\n")
            .unwrap();
        assert_eq!(front.scalar("homepage"), Some("https://example.com/x"));
        assert_eq!(front.scalar("bare"), Some("https://x.dev/y"));
    }

    #[test]
    fn crlf_frontmatter_parses_identically_to_lf() {
        let lf = parse("---\nname: demo\nmeta:\n  a: 1\n---\nbody\n").unwrap();
        let crlf = parse("---\r\nname: demo\r\nmeta:\r\n  a: 1\r\n---\r\nbody\r\n").unwrap();
        assert_eq!(lf.entries, crlf.entries);
    }

    #[test]
    fn still_refuses_the_indirection_shapes() {
        // Nesting widened; indirection did not. An alias exists to be expanded,
        // and repeated expansion is the denial-of-service shape.
        for (label, text) in [
            ("anchor", "---\nbase: &a\n  x: 1\n---\n"),
            ("alias", "---\nname: *a\n---\n"),
            ("merge key", "---\n<<: *defaults\n---\n"),
            ("directive", "---\n%YAML 1.2\n---\n"),
            ("document end", "---\nname: a\n...\n"),
            ("tab indentation", "---\nmeta:\n\ta: 1\n---\n"),
            ("no fence", "name: demo\n"),
            ("unclosed block", "---\nname: demo\n"),
            ("duplicate key", "---\nname: a\nname: b\n---\n"),
            ("orphan list item", "---\n- Bash\n---\n"),
            ("unclosed flow sequence", "---\ntools: [Bash\n---\n"),
            ("empty key", "---\n: value\n---\n"),
            ("bare text", "---\njust some prose\n---\n"),
        ] {
            assert!(parse(text).is_err(), "{label}: should have been refused");
        }
    }

    #[test]
    fn refuses_nesting_past_the_cap() {
        let mut text = String::from("---\n");
        for level in 0..=MAX_DEPTH + 2 {
            text.push_str(&" ".repeat(level * 2));
            text.push_str(&format!("k{level}:\n"));
        }
        text.push_str("---\n");
        let error = parse(&text).unwrap_err();
        assert!(
            error.message.contains("nests deeper"),
            "expected a depth refusal, got {error}"
        );
    }

    #[test]
    fn errors_carry_a_line_number() {
        let error = parse("---\nname: a\nname: b\n---\n").unwrap_err();
        assert_eq!(error.line, 3);
        assert!(error.to_string().contains("duplicate key"));
    }

    #[test]
    fn detects_a_frontmatter_fence() {
        assert!(has_frontmatter("---\nname: x\n---\n"));
        assert!(has_frontmatter("---\r\nname: x\r\n---\r\n"));
        assert!(!has_frontmatter("# Just a heading\n"));
        assert!(!has_frontmatter(""));
    }
}
