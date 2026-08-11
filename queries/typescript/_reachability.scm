; Reachability primitives for TypeScript.
;
; Four roles, no engine changes. Function *declarations* and functions bound to a
; name are both definitions, because both are things a later call can name.

(function_declaration
  name: (identifier) @def.name) @def.span

(variable_declarator
  name: (identifier) @def.name
  value: [(function_expression) (arrow_function)]) @def.span

(method_definition
  name: (property_identifier) @def.name) @def.span

; Statically-known callees: a bare name, or the final property of a path.
(call_expression
  function: (identifier) @call.name)

(call_expression
  function: (member_expression
    property: (property_identifier) @call.name))

; Computed callees: `fns[name]()`, `(getFn())()`. Unfollowable.
(call_expression
  function: [(subscript_expression) (call_expression)] @call.dynamic)
