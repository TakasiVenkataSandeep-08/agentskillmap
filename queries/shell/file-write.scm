; Writes a file, in shell.
;
; The redirect direction is matched explicitly. tree-sitter-bash parses `<`, `>`
; and `>>` to one `file_redirect` node, and this repository has already shipped
; the consequence of not saying which it wanted: `sh.credential-read.dotfile`
; reported `cat > .env` — a write — as a credential read. Here the wanted
; directions are the output ones, and the input one is excluded by naming them.

; `>/dev/null` is discarding output, not writing a file, and it is one of the
; most common constructs in shell. It was 8 of the 13 false positives the corpus
; produced for `fs.write.outside_bundle` on this rule's first measured run —
; `/dev/null` is an absolute path, so the outside-bundle filter matched it
; perfectly correctly and the claim was still wrong.
;
; Excluded here rather than in `[match]` because the match modes are all
; positive: they say which paths are interesting, never which are not. A
; `#not-match?` in the query is the existing way to say "not this", and it keeps
; the exclusion next to the shape it qualifies.
(file_redirect
  ">"
  destination: [(word) (string)] @path
  (#not-match? @path "^/dev/")) @site

(file_redirect
  ">>"
  destination: [(word) (string)] @path
  (#not-match? @path "^/dev/")) @site

; tee /etc/thing
(command
  name: (command_name (word) @_cmd)
  argument: [(word) (string)] @path
  (#eq? @_cmd "tee")) @site
