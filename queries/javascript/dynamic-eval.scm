; Turns a string into code, in javascript. Shared by javascript and typescript.
;
; Bare `eval` only, for the same reason as python: `.eval(` is a method on
; plenty of objects that are not the interpreter. See the python query.

; eval(userInput)
(call_expression
  function: (identifier) @_fn
  (#eq? @_fn "eval")) @site

; new Function("return " + expr)
(new_expression
  constructor: (identifier) @_ctor
  (#eq? @_ctor "Function")) @site

; vm.runInNewContext(src) / vm.runInThisContext(src)
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  (#eq? @_mod "vm")
  (#match? @_fn "^(runInNewContext|runInThisContext|runInContext|compileFunction)$")) @site

; setTimeout("doThing()", 100) — a STRING first argument is evaluated as code.
;
; The string is the whole signal. `setTimeout(fn, 100)` with a function is the
; overwhelmingly common form and reaches no evaluator, so matching the callee
; name alone would fire on essentially every asynchronous skill in the corpus.
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (string))
  (#match? @_fn "^(setTimeout|setInterval)$")) @site
