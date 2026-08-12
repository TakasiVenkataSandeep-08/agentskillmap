; Turns a string into code.
;
; BARE IDENTIFIERS ONLY, and that single decision is what makes this rule
; shippable. `.eval(` is PyTorch's mode switch — `model.eval()` appears in a
; large fraction of ML-adjacent skills and means "stop training", not "run this
; string" — and `.exec(` is a method on database cursors and compiled regular
; expressions. A member-expression pattern would fire on all of them.
;
; This project has now met that trap three times, once per method name: `.get(`
; in the egress rules, `.fetch(` in the same, and `.exec(` in the process rules,
; where an unconstrained match fired on `pattern.exec(text)` before a negative
; fixture caught it. Here the answer is not a constrained object list but no
; member form at all: unlike `requests` or `child_process`, there is no module a
; legitimate `eval` is reached through.
;
; Disjoint from `process-exec.scm` by construction: that rule requires the call
; to be `subprocess.<verb>` or `os.<verb>`, so python's `exec()` builtin lands
; here and `os.system()` lands there.

; eval(user_input) / exec(payload) / compile(src, "<s>", "exec")
(call
  function: (identifier) @_fn
  (#match? @_fn "^(eval|exec|compile)$")) @site
