; instruction.exfil
;
; Prose directing bundle-adjacent data to an external destination. The signal is
; the pairing: something sensitive, and somewhere it is being sent.
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
    "(?i)(send|post|upload|transmit|forward|exfiltrate) [^.\n]{0,60}(credential|token|secret|api[ _-]?key|password|\.env|ssh key|private key)[^.\n]{0,60} (to|at|via) "))

(((inline) @site)
  (#match? @site
    "(?i)(send|post|upload|transmit|forward) [^.\n]{0,60} to (https?://|our |their )[^.\n]{0,40}(server|endpoint|api|webhook|collector|bucket)"))
