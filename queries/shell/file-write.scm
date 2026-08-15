; Writes a file, in shell.
;
; The redirect direction is matched explicitly. tree-sitter-bash parses `<`, `>`
; and `>>` to one `file_redirect` node, and this repository has already shipped
; the consequence of not saying which it wanted: `sh.credential-read.dotfile`
; reported `cat > .env` — a write — as a credential read.
;
; `>/dev/null` is discarding output, not writing a file, and it was 8 of the 13
; false positives the corpus produced for `fs.write.outside_bundle` on this
; rule's first measured run. Excluded with `#not-match?` rather than in `[match]`,
; because every match mode is positive — they say which paths are interesting,
; never which are not.

; > path  /  >> path — a literal destination.
(file_redirect
  [">" ">>"]
  destination: [(word) (string)] @path
  (#not-match? @path "^/dev/")) @site

; > "$GRAPH_FILE"  /  >> "$DIR/log" — a destination reached through a variable.
;
; Captured as `dynamic` rather than `path`, which is the whole point: the `path`
; role goes through literal extraction and would yield the text `"$GRAPH_FILE"`,
; matching nothing. The `dynamic` role folds first. Shell folding landed at the
; same time as this branch and neither is useful without the other — the corpus
; misses reached their paths through exactly this shape.
(file_redirect
  [">" ">>"]
  destination: [
    (simple_expansion)
    (expansion)
    (command_substitution)
    (concatenation)
    (string (simple_expansion))
    (string (expansion))
  ] @dynamic) @site

; Commands that create or copy files. `mkdir` and `cp` are how shell scripts
; write most of what they write, and neither was a sink until the corpus said so.
(command
  name: (command_name (word) @_cmd)
  argument: [(word) (string) (raw_string)] @path
  (#match? @_cmd "^(mkdir|touch|tee|install)$")
  (#not-match? @path "^-")) @site

(command
  name: (command_name (word) @_cmd)
  argument: [
    (simple_expansion)
    (expansion)
    (command_substitution)
    (concatenation)
    (string (simple_expansion))
    (string (expansion))
  ] @dynamic
  (#match? @_cmd "^(mkdir|touch|tee|install)$")) @site

; cp / mv / ln write their LAST argument. Matching every argument would report
; the source as a write, which is the same direction error the redirect rule
; exists to avoid, so the destination is anchored as the final child.
(command
  name: (command_name (word) @_cmd)
  argument: [(word) (string) (raw_string)] @path .
  (#match? @_cmd "^(cp|mv|ln|rsync)$")) @site

(command
  name: (command_name (word) @_cmd)
  argument: [
    (simple_expansion)
    (expansion)
    (command_substitution)
    (concatenation)
    (string (simple_expansion))
    (string (expansion))
  ] @dynamic .
  (#match? @_cmd "^(cp|mv|ln|rsync)$")) @site
