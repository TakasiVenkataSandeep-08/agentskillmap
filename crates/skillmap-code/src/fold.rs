//! Resolving computed path expressions to literals, where that is honest.
//!
//! Every credential read in the labelled corpus reaches its path by
//! **computation**. Not one uses a string literal. The rules were written
//! against literals, which is why recall measured 38.9% and why every miss was a
//! computed target. `docs/00-tasks.md` recorded that as the highest-value open
//! question for detection; this is the answer to it.
//!
//! # What this is not
//!
//! Not an interpreter, and not a dataflow analysis. It folds a small, closed set
//! of shapes that appear in real bundles:
//!
//! | Shape | Example |
//! |---|---|
//! | string literal | `".env"` |
//! | path join | `base / ".env"`, `os.path.join(a, b)`, `path.join(a, b)` |
//! | home directory | `Path.home()`, `os.homedir()`, `expanduser("~")` |
//! | constructor | `Path(x)`, `pathlib.Path(x)` |
//! | single-assignment constant | `BASE = Path.home() / ".foo"` then `BASE / ".env"` |
//!
//! Anything else is unknown, and unknown stays unknown.
//!
//! # Partial knowledge is the common case, and is reported as such
//!
//! `path.join(process.cwd(), ".baoyu-skills", ".env")` cannot be resolved — the
//! working directory is not knowable from source. But the *tail* is: whatever
//! this is, it ends in `.env`. [`Folded::Tail`] carries that, and rules match it
//! through `[match] path_suffixes` rather than `path_prefixes`.
//!
//! That distinction is load-bearing. A prefix pattern answers "is this the file
//! at this location"; a suffix pattern answers "is this a file with this name,
//! wherever it lives". The corpus needs the second, and conflating them would
//! make `.env` in `path_prefixes` match only a path that *starts* with `.env`,
//! which is nothing anyone writes.
//!
//! # Invariant 7 is intact
//!
//! Folding is engine capability: it decides what a path expression *is*. Which
//! paths are interesting remains entirely in `rules/*.toml`. Nothing here knows
//! that `.env` is a credential — that word appears in this file only in doc
//! comments.

use std::collections::BTreeMap;
use tree_sitter::Node;

/// How deep a path expression may nest before folding gives up.
///
/// Hostile input is the normal case (invariant 10), and an unbounded walk over
/// an attacker-supplied tree is a denial of service on somebody's CI. Ten is far
/// past anything in the corpus; the deepest real expression found was four.
const MAX_DEPTH: usize = 10;

/// The marker standing in for an unresolvable path component.
///
/// Never matched against; it exists so a [`Folded::Tail`] can be rendered for a
/// human without implying the head was empty.
pub const UNKNOWN: &str = "<computed>";

/// What a path expression could be resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folded {
    /// Every component resolved. As good as a literal, and matched the same way.
    Exact(String),
    /// The tail resolved and the head did not. Matched against `path_suffixes`.
    Tail(String),
    /// Nothing usable. The caller reports `unresolved: computed_target`.
    Unknown,
}

/// Single-assignment constants in one file.
///
/// Only identifiers assigned **exactly once** are usable. A name assigned twice
/// has no single value at the read site without control-flow analysis, and
/// guessing which assignment wins is how a resolver starts reporting paths that
/// were never opened.
#[derive(Debug, Default)]
pub struct Scope<'tree> {
    bindings: BTreeMap<String, Node<'tree>>,
}

impl<'tree> Scope<'tree> {
    /// Collect single-assignment bindings from a whole file.
    #[must_use]
    pub fn collect(root: Node<'tree>, source: &str) -> Self {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut values: BTreeMap<String, Node<'tree>> = BTreeMap::new();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            // Python `x = expr`, JavaScript/TypeScript `const x = expr`.
            let binding = match node.kind() {
                "assignment" => node
                    .child_by_field_name("left")
                    .zip(node.child_by_field_name("right")),
                "variable_declarator" => node
                    .child_by_field_name("name")
                    .zip(node.child_by_field_name("value")),
                // Shell `NAME=value`, including `local NAME=value`.
                //
                // Absent until measurement said so: 13 of the 25 corpus misses
                // for the outside_bundle terms were shell, and every one of them
                // reached its path through a variable this folder could not see.
                // The subsystem simply did not speak the language.
                "variable_assignment" => node
                    .child_by_field_name("name")
                    .zip(node.child_by_field_name("value")),
                _ => None,
            };

            if let Some((name, value)) = binding {
                if matches!(name.kind(), "identifier" | "variable_name") {
                    if let Some(text) = source.get(name.start_byte()..name.end_byte()) {
                        *counts.entry(text.to_owned()).or_insert(0) += 1;
                        values.insert(text.to_owned(), value);
                    }
                }
            }

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }

        values.retain(|name, _| counts.get(name).copied() == Some(1));
        Self { bindings: values }
    }

    fn get(&self, name: &str) -> Option<Node<'tree>> {
        self.bindings.get(name).copied()
    }
}

/// Fold a path expression to whatever is knowable from source alone.
#[must_use]
pub fn fold(node: Node<'_>, source: &str, scope: &Scope<'_>) -> Folded {
    fold_at(node, source, scope, 0)
}

fn fold_at(node: Node<'_>, source: &str, scope: &Scope<'_>, depth: usize) -> Folded {
    if depth > MAX_DEPTH {
        return Folded::Unknown;
    }
    let text = |n: Node<'_>| source.get(n.start_byte()..n.end_byte()).unwrap_or_default();

    match node.kind() {
        "string" | "template_string" | "raw_string" => {
            // A shell string is a sequence of parts: `"${HOME}/.local/share"` is
            // an expansion followed by literal text. Folding it part by part is
            // what makes a shell variable usable at all, and it is why this arm
            // checks for expansion children before falling back.
            // Any named child means the string is not a literal: an expansion,
            // a command substitution, an arithmetic expansion. Each has to go
            // through `fold_parts`, which knows how to say "unknown".
            //
            // Checking only for expansions here let `"/tmp/x_$(date +%s).wav"`
            // fall through to raw extraction, and the command substitution was
            // spliced into the path as if it were text — a fabricated path in
            // `detail.paths`, which is the same confident wrong answer the
            // literal extractor was producing for unquoted shell words. Caught
            // by reading a corpus finding rather than by a test.
            let mut cursor = node.walk();
            let has_parts = node.named_children(&mut cursor).any(|child| {
                !matches!(
                    child.kind(),
                    "string_content" | "string_fragment" | "string_start" | "string_end"
                )
            });
            if has_parts {
                return fold_parts(node, source, scope, depth);
            }

            let value = crate::literal(node, source);
            // A template string with an interpolation is not a literal. Its
            // static prefix is knowable but its tail is not, which is the wrong
            // way round for suffix matching, so it is simply unknown.
            if value.contains("${") {
                Folded::Unknown
            } else {
                Folded::Exact(value)
            }
        }

        // `base / "name"` in Python, and string `+` in either language.
        "binary_operator" | "binary_expression" => {
            let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) else {
                return Folded::Unknown;
            };
            let operator = node
                .child_by_field_name("operator")
                .map(|op| text(op).to_owned())
                .unwrap_or_default();
            if operator != "/" && operator != "+" {
                return Folded::Unknown;
            }
            join(
                fold_at(left, source, scope, depth + 1),
                fold_at(right, source, scope, depth + 1),
                operator == "/",
            )
        }

        "identifier" | "variable_name" => scope.get(text(node)).map_or(Folded::Unknown, |value| {
            fold_at(value, source, scope, depth + 1)
        }),

        // Shell `$VAR` and `${VAR}`.
        //
        // `$HOME` is the shell spelling of the home directory, and resolves the
        // same way `Path.home()` and `os.homedir()` already do — `~`, which is
        // how the rule data spells it.
        //
        // `${VAR:-default}` folds to UNKNOWN on purpose. Both branches are
        // reachable and which one runs depends on the environment, so picking
        // either would be asserting something source cannot establish. That
        // costs real detections — `${XDG_STATE_HOME:-$HOME/.local/state}` is in
        // the corpus — and the alternative is a confident guess.
        "simple_expansion" | "expansion" => {
            let mut cursor = node.walk();
            let named: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
            let Some(name) = named.first().filter(|n| n.kind() == "variable_name") else {
                return Folded::Unknown;
            };
            // More than the variable itself means a default or an operator.
            if named.len() > 1 {
                return Folded::Unknown;
            }
            match text(*name) {
                "HOME" => Folded::Exact("~".to_owned()),
                other => scope.get(other).map_or(Folded::Unknown, |value| {
                    fold_at(value, source, scope, depth + 1)
                }),
            }
        }

        // Shell `"$DIR/graph.json"` and unquoted `$DIR/graph.json`: a sequence of
        // literal and expansion parts, folded left to right.
        "concatenation" => fold_parts(node, source, scope, depth),

        "call" | "call_expression" => fold_call(node, source, scope, depth),

        // `(expr)` and TypeScript's `expr as T` are transparent.
        "parenthesized_expression" | "as_expression" | "non_null_expression" => {
            node.named_child(0).map_or(Folded::Unknown, |inner| {
                fold_at(inner, source, scope, depth + 1)
            })
        }

        _ => Folded::Unknown,
    }
}

/// Fold a node whose children are alternating literal text and expansions.
///
/// Shell strings and concatenations are both this shape. Parts are glued left to
/// right with plain concatenation semantics rather than join semantics, because
/// `"$DIR/graph.json"` already carries its own separator — inserting another
/// would produce `~/x//graph.json`.
///
/// Quote characters are dropped: they delimit the string and are not part of the
/// path. Anything that is neither literal text nor a foldable expansion makes the
/// whole thing unknown, which is the same rule every other arm follows.
fn fold_parts(node: Node<'_>, source: &str, scope: &Scope<'_>, depth: usize) -> Folded {
    let mut cursor = node.walk();
    let mut acc: Option<Folded> = None;

    for part in node.children(&mut cursor) {
        let text = source
            .get(part.start_byte()..part.end_byte())
            .unwrap_or_default();
        if matches!(text, "\"" | "'" | "$'") {
            continue;
        }
        // The literal pieces of a string are named nodes in some grammars and
        // anonymous in others, so both are treated as text. Everything else
        // named — an expansion, a command substitution — is folded, and folding
        // is what knows how to say "unknown" instead of splicing it in.
        let folded = match part.kind() {
            "string_start" | "string_end" => continue,
            "string_content" | "string_fragment" => Folded::Exact(text.to_owned()),
            _ if part.is_named() => fold_at(part, source, scope, depth + 1),
            _ => Folded::Exact(text.to_owned()),
        };
        acc = Some(match acc {
            None => folded,
            Some(left) => join(left, folded, false),
        });
    }
    acc.unwrap_or(Folded::Unknown)
}

/// Fold a call expression: joins, constructors, and home-directory lookups.
fn fold_call(node: Node<'_>, source: &str, scope: &Scope<'_>, depth: usize) -> Folded {
    let text = |n: Node<'_>| source.get(n.start_byte()..n.end_byte()).unwrap_or_default();

    let Some(callee) = node.child_by_field_name("function") else {
        return Folded::Unknown;
    };
    let callee_text = text(callee);

    // The last dotted segment: `os.path.join` -> `join`, `Path.home` -> `home`.
    let tail = callee_text.rsplit('.').next().unwrap_or(callee_text);

    let arguments: Vec<Node<'_>> = node
        .child_by_field_name("arguments")
        .map(|list| {
            let mut cursor = list.walk();
            list.named_children(&mut cursor).collect()
        })
        .unwrap_or_default();

    match tail {
        // Every real home is a real path, and `~` is how the rule data spells it.
        // Producing `~` rather than an absolute path keeps a folded result
        // comparable to a literal a rule author would write.
        "home" | "homedir" => Folded::Exact("~".to_owned()),

        "expanduser" => match arguments.first() {
            Some(first) => fold_at(*first, source, scope, depth + 1),
            None => Folded::Unknown,
        },

        "join" | "resolve" => arguments
            .iter()
            .map(|argument| fold_at(*argument, source, scope, depth + 1))
            .reduce(|left, right| join(left, right, true))
            .unwrap_or(Folded::Unknown),

        // `Path(x)`, `pathlib.Path(x)` — transparent over a single argument.
        "Path" => match arguments.first() {
            Some(first) => fold_at(*first, source, scope, depth + 1),
            None => Folded::Unknown,
        },

        // `os.path.dirname(...)`, `__dirname`, `process.cwd()` and friends are
        // real directories this analysis cannot name. They are not failures —
        // they are a known-unknown head, which is exactly what makes the tail
        // worth reporting.
        "dirname" | "abspath" | "cwd" | "realpath" | "getcwd" => Folded::Tail(String::new()),

        _ => Folded::Unknown,
    }
}

/// Join two folded components, propagating unknowns.
fn join(left: Folded, right: Folded, separator: bool) -> Folded {
    let glue = |a: &str, b: &str| {
        if !separator || a.is_empty() || a.ends_with('/') || b.starts_with('/') {
            format!("{a}{b}")
        } else {
            format!("{a}/{b}")
        }
    };

    match (left, right) {
        // An unresolvable *tail* poisons everything: the result ends in something
        // unknown, so no suffix claim can be made about it.
        (_, Folded::Unknown) => Folded::Unknown,

        (Folded::Exact(a), Folded::Exact(b)) => Folded::Exact(glue(&a, &b)),

        // An unresolvable head, under a **join**, still leaves a usable tail:
        // `os.path.join(root, ".env")` ends in `/.env` whatever `root` is,
        // because a join inserts a separator.
        //
        // Under **concatenation** it does not, and the difference is a real false
        // positive rather than a nicety: `root + ".env"` can produce
        // `production.env`, which is not a file called `.env`. This is the one
        // place the separator flag changes an answer rather than a rendering.
        (Folded::Unknown, Folded::Exact(b)) if separator => Folded::Tail(b),
        (Folded::Unknown, Folded::Tail(b)) if separator => Folded::Tail(b),
        (Folded::Unknown, _) => Folded::Unknown,

        // A known head followed by an unknown-headed tail: the head is lost,
        // because the tail's own head is unknown.
        (Folded::Tail(a), Folded::Exact(b)) => Folded::Tail(glue(&a, &b)),
        (Folded::Exact(_), Folded::Tail(b)) | (Folded::Tail(_), Folded::Tail(b)) => Folded::Tail(b),
    }
}

impl Folded {
    /// The resolved path, for matching. `None` when nothing was resolved.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Exact(value) | Self::Tail(value) => Some(value),
            Self::Unknown => None,
        }
    }

    /// A path a human can read, with the unknown head made visible.
    #[must_use]
    pub fn render(&self) -> Option<String> {
        match self {
            Self::Exact(value) => Some(value.clone()),
            Self::Tail(value) if value.is_empty() => None,
            Self::Tail(value) => Some(format!("{UNKNOWN}/{value}")),
            Self::Unknown => None,
        }
    }
}

/// Whether a folded path ends with `pattern` at a component boundary.
///
/// Component boundary, not raw string suffix: `.env` must match `a/b/.env` and
/// `.env`, and must **not** match `production.env` or `dotenv`. A raw
/// `ends_with` would match all four, and the last two are the false positives
/// that would show up first in a corpus full of configuration files.
#[must_use]
pub fn ends_with_component(value: &str, pattern: &str) -> bool {
    if value == pattern {
        return true;
    }
    value
        .strip_suffix(pattern)
        .is_some_and(|head| head.ends_with('/'))
}

/// Whether `pattern` appears in `value` as whole path component(s).
///
/// The question `ends_with_component` cannot ask. A credential directory is
/// identified by its own name — `~/.clawdbot/credentials/homebridge.json` — and
/// the filename inside it is per-integration, so matching by name would require
/// enumerating files nobody can enumerate.
///
/// Both ends are bounded, so `credentials` matches `a/credentials/b` but not
/// `a/my-credentials/b` or `a/credentialsx/b`. Trimming the separators off both
/// sides before framing them means a pattern written as `credentials`,
/// `/credentials` or `credentials/` behaves identically, and a multi-component
/// pattern like `.claude/skills` is bounded as one unit rather than matching the
/// two components separately.
#[must_use]
pub fn contains_component(value: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_matches('/');
    if pattern.is_empty() {
        return false;
    }
    format!("/{}/", value.trim_matches('/')).contains(&format!("/{pattern}/"))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn a_component_suffix_does_not_match_mid_name() {
        // The false positives a raw ends_with would produce, in a corpus that is
        // mostly configuration files.
        assert!(ends_with_component(".env", ".env"));
        assert!(ends_with_component("~/.clawdbot/x/.env", ".env"));
        assert!(ends_with_component(
            "a/b/credentials.json",
            "credentials.json"
        ));

        assert!(!ends_with_component("production.env", ".env"));
        assert!(!ends_with_component("dotenv", ".env"));
        assert!(!ends_with_component(
            "my-credentials.json",
            "credentials.json"
        ));
    }

    #[test]
    fn a_contained_component_is_bounded_at_both_ends() {
        // The shape this exists for: the directory is the signal and the
        // filename inside it is per-integration.
        assert!(contains_component(
            "~/.clawdbot/credentials/homebridge.json",
            "credentials"
        ));
        assert!(contains_component("a/credentials/b", "credentials"));

        // A trailing component counts as contained — `ends_with_component`
        // would agree, and disagreeing would be a surprise with no upside.
        assert!(contains_component("a/credentials", "credentials"));
        assert!(contains_component("credentials", "credentials"));

        // Both ends bounded. Left-unbounded is the interesting one: a raw
        // `contains` would match every one of these.
        assert!(!contains_component("a/my-credentials/b", "credentials"));
        assert!(!contains_component("a/credentialsx/b", "credentials"));
        assert!(!contains_component("a/xcredentials/b", "credentials"));

        // Written with or without separators, the pattern behaves the same.
        assert!(contains_component("a/credentials/b", "/credentials"));
        assert!(contains_component("a/credentials/b", "credentials/"));

        // Multi-component patterns bind as one unit, so the two components have
        // to be adjacent and in order.
        assert!(contains_component("~/.claude/skills/x", ".claude/skills"));
        assert!(!contains_component(
            "~/.claude/other/skills/x",
            ".claude/skills"
        ));

        // An empty pattern would otherwise match everything, which is how a
        // typo in a TOML list turns into a rule that fires on every path.
        assert!(!contains_component("a/b", ""));
        assert!(!contains_component("a/b", "/"));
    }

    #[test]
    fn an_unresolvable_tail_poisons_but_an_unresolvable_head_does_not() {
        // The result ends in something unknown: no suffix claim is possible.
        assert_eq!(
            join(Folded::Exact("x".to_owned()), Folded::Unknown, true),
            Folded::Unknown
        );
        // But a join with an unknown head still ends where it says it does.
        assert_eq!(
            join(Folded::Unknown, Folded::Exact(".env".to_owned()), true),
            Folded::Tail(".env".to_owned())
        );
    }

    #[test]
    fn concatenation_with_an_unknown_head_claims_nothing() {
        // `root + ".env"` can be `production.env`. A join cannot, because it
        // inserts a separator. This distinction is the difference between a
        // detection and a false positive, and it is the only place the separator
        // flag changes an answer rather than a rendering.
        assert_eq!(
            join(Folded::Unknown, Folded::Exact(".env".to_owned()), false),
            Folded::Unknown
        );
    }

    #[test]
    fn an_unknown_head_leaves_a_usable_tail() {
        assert_eq!(
            join(
                Folded::Tail(String::new()),
                Folded::Exact(".env".to_owned()),
                true
            ),
            Folded::Tail(".env".to_owned())
        );
    }

    #[test]
    fn a_tail_renders_with_its_unknown_head_visible() {
        assert_eq!(
            Folded::Tail(".env".to_owned()).render().as_deref(),
            Some("<computed>/.env")
        );
        assert_eq!(
            Folded::Exact("~/.netrc".to_owned()).render().as_deref(),
            Some("~/.netrc")
        );
        assert_eq!(Folded::Unknown.render(), None);
    }
}
