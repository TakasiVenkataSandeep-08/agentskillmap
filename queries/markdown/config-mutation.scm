; instruction.config_mutation
;
; Prose directing edits to agent configuration. A skill that rewrites the agent's
; own config changes what every later session does, which outlives the skill's own
; trigger and is not something a reviewer reading SKILL.md would expect.
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
    "(?i)(add|append|write|edit|modify|update|insert) [^.\n]{0,60}(to|in|into) [^.\n]{0,30}(CLAUDE\.md|AGENTS\.md|settings\.json|settings\.local\.json|\.mcp\.json)"))

(((inline) @site)
  (#match? @site
    "(?i)(register|install|configure|set up|add) (a |an |the )?(new )?(hook|statusline|mcp server|subagent) "))
