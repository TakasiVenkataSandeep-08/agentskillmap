; Fetched content reaches an interpreter, in one expression.
;
; Scoped exactly as `obfuscation.scm` is, and for the same reason: the engine
; has no taint analysis. "Fetched on line 4, executed on line 9" is not detected
; and this rule does not claim it. What IS detectable is the single construct
; where both happen at once — and in shell that construct is the whole idiom.

; curl -sS https://... | bash
;
; The left side names the protocol and the right side is an interpreter reading
; its script from the pipe. Anchored so the interpreter is the last named child
; of its command and therefore takes no script argument, which is what separates
; `| bash` from `| python3 render.py`, where stdin is data.
(pipeline
  (command
    name: (command_name (word) @_get))
  (command
    name: (command_name (word) @_sh) .)
  (#match? @_get "^(curl|wget|fetch)$")
  (#match? @_sh "^(bash|sh|zsh|dash|ksh|python|python3|node|ruby|perl)$")) @site
