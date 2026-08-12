; Spawns a process whose program is NOT statically known from source.
;
; The structural counterpart to `process-exec.scm`. See that file for why the
; split is a matter of query shape rather than of folding, and for the test that
; pins the two as disjoint.
;
; This is the more interesting of the two terms: a caller who can influence
; argv[0] can run anything, so "some process" is a weaker claim than "ffmpeg"
; in a way a reader should be able to see at a glance.

; subprocess.run(cmd) — the program is a name, an attribute, or a call result.
;
; `(attribute)` is what catches `subprocess.run([sys.executable, ...])` further
; down, and it is a real case from the labelled corpus rather than a
; hypothetical: sys.executable resolves to a python interpreter at runtime and
; to nothing at all from source.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . [
    (attribute)
    (call)
    (subscript)
    (binary_operator)
  ])
  (#eq? @_mod "subprocess")
  (#match? @_fn "^(run|call|check_call|check_output|Popen)$")) @site

; subprocess.run([sys.executable, str(runner)]) — list whose FIRST element is
; not a literal. The rest of the list may be anything; argv[0] is the question.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (list . [
    (identifier)
    (attribute)
    (call)
    (subscript)
    (binary_operator)
  ]))
  (#eq? @_mod "subprocess")
  (#match? @_fn "^(run|call|check_call|check_output|Popen)$")) @site

; subprocess.run(f"rm {path}", shell=True) — an interpolated command line.
;
; Matched structurally on the interpolation, which is exactly what the static
; query excludes by predicate. The two together are what make an f-string land
; in one term and only one.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string (interpolation)))
  (#eq? @_mod "subprocess")
  (#match? @_fn "^(run|call|check_call|check_output|Popen)$")) @site

; os.system(cmd) / os.system(f"rm {path}")
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . [
    (attribute)
    (call)
    (subscript)
    (binary_operator)
    (string (interpolation))
  ])
  (#eq? @_mod "os")
  (#match? @_fn "^(system|popen)$")) @site
