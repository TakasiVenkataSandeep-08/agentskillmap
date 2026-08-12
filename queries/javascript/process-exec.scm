; Spawns a process whose program is statically known from source.
;
; Structurally disjoint from `process-exec-dynamic.scm`; see that pair's test in
; crates/skillmap-code/tests/fixtures.rs. Shared by javascript and typescript.
;
; BOTH IMPORT FORMS ARE WRITTEN HERE, at the same time rather than one after the
; other. `child_process.execSync(...)` and a destructured `const { execSync } =
; require("child_process")` are the same call, and shipping only the member form
; is the omission this project has made three separate times — most recently in
; its own labelling tooling. Member and bare are written as a pair for every
; shape below.
;
; THE MEMBER FORM CONSTRAINS ITS OBJECT, and that is not decoration. `.exec(` is
; how every regular expression in JavaScript is run — `pattern.exec(text)` — and
; it is a method on database clients besides. An unconstrained property match
; fired on both in the negative fixture, which is how this was caught. The bare
; form needs no such guard: a destructured `execSync` came from somewhere, and a
; local function called `execSync` that does not spawn is not a thing people
; write.

; child_process.execSync("npm install") — member form, literal program
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (string))
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")
  (#match? @_mod "^(child_process|childProcess|cp|proc|cproc)$")) @site

; execSync("npm install") — bare form, from a destructured import
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (string))
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")) @site

; A template literal with no substitution is still a literal. The `#not-match?`
; keeps these disjoint from the dynamic query, which matches the substitution
; structurally.
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (template_string) @_prog)
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")
  (#match? @_mod "^(child_process|childProcess|cp|proc|cproc)$")
  (#not-match? @_prog "\\$\\{")) @site

(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (template_string) @_prog)
  (#match? @_fn "^(exec|execSync|execFile|execFileSync|spawn|spawnSync|fork)$")
  (#not-match? @_prog "\\$\\{")) @site
