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
(file_redirect
  destination: [(word) (string)] @path) @site

; Computed target — captured deliberately so the engine emits
; `unresolved: computed_target` rather than staying silent. Invariant 3.
(command
  name: (command_name (word) @_cmd)
  argument: [(simple_expansion) (expansion) (command_substitution)] @dynamic
  (#match? @_cmd "^(cat|less|more|head|tail|source|\.|read)$")) @site
