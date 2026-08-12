; Spawns a process whose program is NOT statically known from source.
;
; The structural counterpart to `process-exec.scm`. Both import forms as a pair,
; and the same object constraint on the member form — see that file for why
; `pattern.exec(text)` makes it necessary.

; child_process.execSync(cmd) — member form, computed program
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . [
    (member_expression)
    (call_expression)
    (subscript_expression)
    (binary_expression)
  ])
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")
  (#match? @_mod "^(child_process|childProcess|cp|proc|cproc)$")) @site

; execSync(cmd) — bare form, computed program
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . [
    (member_expression)
    (call_expression)
    (subscript_expression)
    (binary_expression)
  ])
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")) @site

; execSync(`npm install ${pkg}`) — an interpolated command line, matched on the
; substitution rather than by guessing at the text. This is what the static query
; excludes by predicate, and the two together put a template literal in exactly
; one term.
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (template_string (template_substitution)))
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")
  (#match? @_mod "^(child_process|childProcess|cp|proc|cproc)$")) @site

(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (template_string (template_substitution)))
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")) @site
