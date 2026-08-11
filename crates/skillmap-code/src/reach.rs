//! Reachability: is this sink on a path that actually runs?
//!
//! `AGENTS.md` and `ARCHITECTURE.md` both insist on the distinction, and the
//! reference fixture bakes it in: `fixtures/python/credential-read/positive.py`
//! reads `~/.aws/credentials` inside `collect()`, which nothing calls, and its
//! expected reachability is `present` — not `observed`.
//!
//! Three answers, and the difference between the last two matters:
//!
//! | Answer | Claim |
//! |---|---|
//! | `observed` | A path from code that runs was **established**. |
//! | `present` | The sink exists; no path was established. |
//! | `unresolved` | A computed callee **blocked** the analysis. |
//!
//! `present` and `unresolved` are not the same statement. `present` says the
//! analysis looked and found no caller; `unresolved` says the analysis could not
//! see well enough to say. Collapsing them would be exactly the silent-drop
//! failure invariant 3 exists to prevent.
//!
//! No node types appear in this file. It consumes four roles — `def.name`,
//! `def.span`, `call.name`, `call.dynamic` — that a per-language query supplies,
//! which is what keeps invariant 7 true for reachability and not just for sinks.

use skillmap_core::Reachability;
use skillmap_rules::LoadedLanguage;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Node, QueryCursor, Tree};

/// One function definition.
struct Definition {
    name: String,
    start: usize,
    end: usize,
}

/// What the reachability query found in one file.
pub struct Reach {
    definitions: Vec<Definition>,
    /// Names reachable from module level, transitively.
    reachable: BTreeSet<String>,
    /// Whether a computed callee appears anywhere the analysis would have to
    /// follow. If so, "not statically reachable" stops meaning "not called".
    blocked: bool,
    /// Whether this file executes at all when the bundle is used.
    entered: bool,
}

impl Reach {
    /// How much was established about a sink at `offset`.
    #[must_use]
    pub fn classify(&self, offset: usize) -> Reachability {
        // A file nothing documents a path to does not run on its own. Claiming
        // `observed` for it would assert a path the analysis never established.
        if !self.entered {
            return Reachability::Present;
        }

        match self.enclosing(offset) {
            // Module level: runs when the file does.
            None => Reachability::Observed,
            Some(name) if self.reachable.contains(name) => Reachability::Observed,
            Some(_) if self.blocked => Reachability::Unresolved,
            Some(_) => Reachability::Present,
        }
    }

    /// The innermost definition containing `offset`.
    ///
    /// Innermost, so a call inside a nested function is attributed to the nested
    /// function rather than to whatever encloses it — defining a function is not
    /// calling it.
    fn enclosing(&self, offset: usize) -> Option<&str> {
        self.definitions
            .iter()
            .filter(|definition| definition.start <= offset && offset < definition.end)
            .min_by_key(|definition| definition.end.saturating_sub(definition.start))
            .map(|definition| definition.name.as_str())
    }
}

/// Build reachability facts for one parsed file.
///
/// `entered` is whether the file runs when the bundle is used. It comes from the
/// load phase the parser already computed: a file the body documents a path to
/// is imported or invoked, and an unreferenced file is not.
#[must_use]
pub fn analyze(language: &LoadedLanguage, tree: &Tree, source: &str, entered: bool) -> Reach {
    // A language with no reachability query is not code — prose has no call
    // graph. Nothing is ever `observed` there, which is the honest answer: the
    // code plane established no execution path because there is none to establish.
    let Some(query) = language.reachability.as_ref() else {
        return Reach {
            definitions: Vec::new(),
            reachable: BTreeSet::new(),
            blocked: false,
            entered: false,
        };
    };
    let bytes = source.as_bytes();

    let index_of = |name: &str| query.capture_index_for_name(name);
    let (def_name_idx, def_span_idx) = (index_of("def.name"), index_of("def.span"));
    let (call_name_idx, call_dynamic_idx) = (index_of("call.name"), index_of("call.dynamic"));

    let mut definitions: Vec<Definition> = Vec::new();
    let mut calls: Vec<(String, usize)> = Vec::new();
    let mut dynamic_offsets: Vec<usize> = Vec::new();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), bytes);
    while let Some(matched) = matches.next() {
        let mut name: Option<String> = None;
        let mut span: Option<(usize, usize)> = None;

        for capture in matched.captures {
            let index = Some(capture.index);
            if index == def_name_idx {
                name = Some(text_of(capture.node, source));
            } else if index == def_span_idx {
                span = Some((capture.node.start_byte(), capture.node.end_byte()));
            } else if index == call_name_idx {
                calls.push((text_of(capture.node, source), capture.node.start_byte()));
            } else if index == call_dynamic_idx {
                dynamic_offsets.push(capture.node.start_byte());
            }
        }

        if let (Some(name), Some((start, end))) = (name, span) {
            definitions.push(Definition { name, start, end });
        }
    }

    let reach = Reach {
        definitions,
        reachable: BTreeSet::new(),
        blocked: false,
        entered,
    };

    // Attribute every call to whatever encloses it, then walk out from module
    // level. A definition is reachable only if something that runs calls it.
    let mut module_calls: Vec<&str> = Vec::new();
    let mut calls_within: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (callee, offset) in &calls {
        match reach.enclosing(*offset) {
            None => module_calls.push(callee),
            Some(owner) => calls_within.entry(owner).or_default().push(callee),
        }
    }

    let mut reachable: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<&str> = module_calls.into_iter().collect();
    while let Some(name) = queue.pop_front() {
        if !reachable.insert(name.to_owned()) {
            continue;
        }
        for callee in calls_within.get(name).into_iter().flatten() {
            queue.push_back(callee);
        }
    }

    // A computed callee only blocks the analysis if it sits somewhere that runs.
    // One buried in a function nothing calls tells us nothing about the rest.
    let blocked = dynamic_offsets.iter().any(|offset| {
        reach
            .enclosing(*offset)
            .is_none_or(|owner| reachable.contains(owner))
    });

    Reach {
        definitions: reach.definitions,
        reachable,
        blocked,
        entered,
    }
}

/// The source text a node covers.
fn text_of(node: Node<'_>, source: &str) -> String {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
        .to_owned()
}
