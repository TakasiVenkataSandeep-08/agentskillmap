; instruction.exec_directive
;
; Prose directing the agent to run a command that fetches remote content and
; executes it. The documented marketplace-poisoning shape delivers exactly this
; way — a Prerequisites section whose fence holds the command — and every other
; plane is blind to it, because fence bodies are never extracted as code.
;
; WHY THIS MATCHES A FENCE AND NOT PROSE. `instruction.fetch_as_instruction`
; matches `(inline)` nodes, so it structurally cannot see inside a
; `fenced_code_block`, and its patterns need the fetch and the execution in one
; sentence while this shape splits them: the prose says *run the setup script*
; and the fence holds the pipe. Widening those patterns would never reach the
; fence.
;
; WHY @site IS THE CONTENT AND NOT THE BLOCK. `code_fence_content` is the
; smallest node available — the grammar gives no per-line node inside a fence —
; and it excludes the ``` delimiters and the info string, which are not the
; finding.
;
; NOTE ON SYNTAX: each pattern is wrapped in an extra pair of parentheses so the
; predicates are grouped with the node they constrain. Without them the
; predicate attaches to nothing and the pattern degenerates to "every fence".
; The negative fixture is what catches that.
;
; The two patterns below are the two forms the corpus actually contains, and
; both require an `https?://` URL. That requirement is doing real work: it is
; what excludes a bundled script whose *filename* contains `curl` and a `.sh`
; suffix, and what excludes a security skill grepping for `curl` as a thing to
; look for. Both are real bundles from the T10 draw.

; Form 1 — fetch piped straight into a shell.
;
; `[^\n|]` on both sides keeps the match inside one line and stops the pipe from
; being found in a later command. `[ \t]*` rather than `\s*` for the same reason:
; `\s` matches a newline, which would let the shell be borrowed from the line below.
(((fenced_code_block
    (info_string) @_lang
    (code_fence_content) @site))
  (#match? @_lang "(?i)^(bash|sh|shell|zsh|console)$")
  (#match? @site
    "(?i)(curl|wget)[^\n|]{0,200}https?://[^\n|]{0,200}\\|[ \t]*(sudo[ \t]+)?(ba|z)?sh\\b"))

; Form 2 — fetch of a script file, run on a later line.
;
; The extension must be preceded by `/` (a path segment, not a bare host) and
; must NOT be followed by `/`. That second condition is the whole defence
; against the `.sh` top-level domain: `https://api.example.sh/register` is an
; API call and `https://example.com/install.sh` is a script. Three bundles in
; the T10 draw were drawn on the former and carry no directive.
(((fenced_code_block
    (info_string) @_lang
    (code_fence_content) @site))
  (#match? @_lang "(?i)^(bash|sh|shell|zsh|console)$")
  (#match? @site
    "(?i)(curl|wget)[^\n]{0,200}https?://[^\n]{0,200}/[^\n/ \"']{1,80}\\.(sh|py)([^A-Za-z0-9/]|$)"))
