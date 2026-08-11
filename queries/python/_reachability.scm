; Reachability primitives for Python.
;
; The engine consumes four roles and knows nothing else about the language:
;
;   @def.name     the name a function is bound to
;   @def.span     the whole definition, so the engine can tell what is inside it
;   @call.name    a statically-known callee
;   @call.dynamic a callee the analysis cannot follow
;
; A sink outside every @def.span runs when the file does. A sink inside one runs
; only if that definition is reachable from module level. A file containing
; @call.dynamic cannot be reasoned about completely, so unreached definitions in
; it report `unresolved` rather than `present` — the analysis was blocked, which
; is a different claim from "this is not called" (invariant 3).

(function_definition
  name: (identifier) @def.name) @def.span

; Statically-known callees: a bare name, or the final attribute of a path.
(call
  function: (identifier) @call.name)

(call
  function: (attribute
    attribute: (identifier) @call.name))

; Computed callees: `globals()["f"]()`, `getattr(mod, name)()`. Captured rather
; than ignored so the engine can say the analysis was blocked.
(call
  function: [(subscript) (call)] @call.dynamic)
