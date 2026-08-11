; Reachability primitives for shell.
;
; Same four roles the engine knows everywhere: @def.name, @def.span, @call.name,
; @call.dynamic. Nothing about shell leaks into Rust.

(function_definition
  name: (word) @def.name) @def.span

; A command invocation is a call. In shell the callee is usually the command
; name itself, which is also how a script calls one of its own functions.
(command
  name: (command_name (word) @call.name))

; Computed callees: `$CMD args`, `eval "$x"`, `. "$f"`. The analysis cannot
; follow any of them, and a script containing one could reach anything in itself.
(command
  name: (command_name
    [(simple_expansion) (expansion) (string)] @call.dynamic))
