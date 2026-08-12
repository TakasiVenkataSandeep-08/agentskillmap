; Turns a string, or fetched bytes, into code — in shell.
;
; Three shapes, and each is deliberately narrower than the obvious version.
;
; DISJOINT FROM `process-exec-dynamic.scm`, which owns a computed command WORD
; (`$CMD args`). Here the command word is perfectly well known — `bash`, `eval`,
; `source` — and what is computed is the CODE it runs. The two never overlap.

; eval "$cmd"
;
; Only with an expansion argument. `eval` on a literal is a shell idiom for
; quoting control and evaluates nothing the reader cannot already see.
(command
  name: (command_name (word) @_cmd)
  argument: [(simple_expansion) (expansion) (string) (command_substitution)]
  (#eq? @_cmd "eval")) @site

; curl -s https://... | bash
;
; The pipeline is the whole signal: an interpreter reading its script from
; standard input runs whatever the left-hand side produced. The interpreter is
; anchored as the LAST named child of its command, so it has no arguments — that
; is what distinguishes `| bash` from `| python3 script.py`, which reads stdin as
; data rather than as code. The corpus's one demonstration payload opens with
; exactly this shape.
(pipeline
  (command
    name: (command_name (word) @_sh) .)
  (#match? @_sh "^(bash|sh|zsh|dash|ksh|python|python3|node|ruby|perl)$")) @site

; source "$CONFIG"  /  . "$x"
;
; Only when the argument is a WHOLE expansion. `source "$SCRIPT_DIR/lib.sh"` has
; a literal tail naming a specific file inside the bundle, which is an import and
; not an evaluation — the same distinction the labelling pass made when it
; declined to call a bundle-local helper `code.dynamic_eval`, and the same
; anchoring the shell exec rule uses to tell `"$@"` from `"$DIR/script.sh"`.
; The taxonomy row says `source` of computed content, and this is what computed
; means.
(command
  name: (command_name (word) @_cmd)
  argument: [(simple_expansion) (string . (simple_expansion) .)]
  (#match? @_cmd "^(source|\.)$")) @site
