; Reads a path conventionally holding credentials, in shell.
;
; Structural, not textual: matches the shape of a command with a reading verb and
; a literal argument. Path filtering lives in [match] so extending coverage never
; requires touching tree-sitter.

; cat ~/.aws/credentials   /   source ~/.env   /   head -1 ~/.netrc
(command
  name: (command_name (word) @_cmd)
  argument: [(word) (string) (raw_string) (concatenation)] @path
  (#match? @_cmd "^(cat|less|more|head|tail|source|\.|read|grep|awk|sed|cp|mv)$")) @site

; Redirected input: `while read -r line; do ... done < ~/.aws/credentials`
;
; The `"<"` is load-bearing and was missing. tree-sitter-bash uses one
; `file_redirect` node for every direction — `<`, `>`, `>>` all parse to the same
; shape with the same `destination` field — so an unconstrained pattern here made
; a rule called `credential-read` fire on `cat > .env`, which *writes* one. The
; T3 labelling pass found it on a real bundle: a setup script generating a .env
; from user input, which is one of the most common things a skill does.
;
; The operator is an anonymous node, so it is matched as a literal.
(file_redirect
  "<"
  destination: [(word) (string)] @path) @site

; Computed target — captured deliberately so the engine emits
; `unresolved: computed_target` rather than staying silent. Invariant 3.
(command
  name: (command_name (word) @_cmd)
  argument: [(simple_expansion) (expansion) (command_substitution)] @dynamic
  (#match? @_cmd "^(cat|less|more|head|tail|source|\.|read)$")) @site
