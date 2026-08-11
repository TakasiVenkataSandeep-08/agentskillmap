; instruction.fetch_as_instruction
;
; Prose telling the agent to treat fetched content as instructions. This is the
; textbook indirect-prompt-injection setup: the bundle itself is benign, and the
; real instructions arrive later from somewhere the reviewer never looked.
;
; NOTE ON SYNTAX: each pattern is wrapped in an extra pair of parentheses so the
; `#match?` predicate is grouped with the node it constrains. Without them the
; predicate attaches to nothing, the pattern degenerates to "every inline node",
; and the rule fires on all prose. The negative fixtures are what caught that.
;
; Character classes exclude the newline as well as the period, so a match stays
; inside one sentence on one line: a negated class matches newlines by default,
; which would let a single pattern span two unrelated paragraphs.

(((inline) @site)
  (#match? @site
    "(?i)(fetch|download|retrieve|curl|wget) [^.\n]{0,80}(and|then) [^.\n]{0,40}(follow|obey|execute|run) "))

(((inline) @site)
  (#match? @site
    "(?i)(treat|use) (the |any |all )?(content|response|output|text|result)s? [^.\n]{0,50} as (your |the )?(instruction|command|directive|prompt)"))

(((inline) @site)
  (#match? @site
    "(?i)(follow|obey|execute|carry out) (the |any |whatever )?(instruction|command|directive|step)s? (in|at|from|found (in|at)) (this |that |the )?(url|link|page|endpoint|address|https?://)"))
