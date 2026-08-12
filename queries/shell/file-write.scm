; Writes a file, in shell.
;
; The redirect direction is matched explicitly. tree-sitter-bash parses `<`, `>`
; and `>>` to one `file_redirect` node, and this repository has already shipped
; the consequence of not saying which it wanted: `sh.credential-read.dotfile`
; reported `cat > .env` — a write — as a credential read. Here the wanted
; directions are the output ones, and the input one is excluded by naming them.
(file_redirect
  ">"
  destination: [(word) (string)] @path) @site

(file_redirect
  ">>"
  destination: [(word) (string)] @path) @site

; tee /etc/thing
(command
  name: (command_name (word) @_cmd)
  argument: [(word) (string)] @path
  (#eq? @_cmd "tee")) @site
