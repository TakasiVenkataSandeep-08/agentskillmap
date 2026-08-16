; instruction.directs_outside_write
;
; Prose directing the agent to run a command that writes to, copies into, or
; makes executable a path outside the bundle.
;
; WHY THIS SHAPE. In a bundle that ships no parseable file at all — 89.8% of the
; corpus — the code that matters lives in fenced blocks, and the dominant genre
; there is reference material rather than instruction. Three earlier candidate
; signals (directing egress, credential access, subprocess spawning) were
; defined and withdrawn: a network call inside a code sample is documentation,
; and at 23-26% base rates with no contextual separator they were noise
; generators. Requiring an operative heading was measured as a rescue and
; failed, at 30% of the CONTROL stratum.
;
; What survives is a shape carrying its own intent. Reference material
; demonstrates logic; it never mutates the reader's machine as an illustration.
; Nobody teaches programming by appending to a shell profile.
;
; NOTE ON SYNTAX: each pattern is wrapped in an extra pair of parentheses so the
; predicates group with the node they constrain. Without them the predicate
; attaches to nothing and the pattern degenerates to "every fence".
;
; EVERY PATTERN STAYS ON ONE LINE. `[^\n]` throughout, and `[ \t]` rather than
; `\s` after a redirect. Three of the six false candidates in the labelled
; stratum came from a probe whose whitespace class crossed a newline, matching a
; trailing `>` on one line against a home path opening the next — once across
; two unrelated fences. A shell redirect cannot span a newline; the pattern
; must not either.

; Form 1 — redirect into a path outside the bundle.
;
; The gap between `>` and the path is `[ \t]*` and an optional quote, nothing
; more. A wider gap matched `grep -ri "<keyword>" ~/clawd-*/memory/`, where the
; `>` closing a placeholder was read as a redirect, and `> mkdir -p ~/.memoria`,
; where a quote marker was. Both are in the labelled stratum as rejections.
(((fenced_code_block
    (info_string) @_lang
    (code_fence_content) @site))
  (#match? @_lang "(?i)^(bash|sh|shell|zsh|console)$")
  (#match? @site
    ">>?[ \t]*[\"']?(~/|\\$HOME/|/etc/|/usr/local/|/opt/)"))

; Form 2 — copy, move, link or install with a destination outside the bundle.
;
; `mkdir` is deliberately absent: creating an empty directory is preparation,
; not a write, and on its own it was 3.11% of the population with a worst case
; of an empty folder.
(((fenced_code_block
    (info_string) @_lang
    (code_fence_content) @site))
  (#match? @_lang "(?i)^(bash|sh|shell|zsh|console)$")
  (#match? @site
    "(^|\n|[ \t;&|])[ \t]*(sudo[ \t]+)?(cp|mv|ln|install)[ \t][^\n]{0,120}[ \t](~/|\\$HOME/|/etc/|/usr/local/|/opt/)"))

; Form 3 — making a path outside the bundle executable.
;
; The moment fetched bytes become runnable. Rare on its own (0.44%) and it
; travels with a download in the labelled stratum, but it is the step that turns
; an inert file into code.
(((fenced_code_block
    (info_string) @_lang
    (code_fence_content) @site))
  (#match? @_lang "(?i)^(bash|sh|shell|zsh|console)$")
  (#match? @site
    "chmod[ \t]+(-[A-Za-z]+[ \t]+)*\\+x[ \t]+[\"']?(~/|\\$HOME/|/etc/|/usr/local/|/opt/)"))
