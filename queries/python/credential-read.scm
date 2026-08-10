; Capture the smallest span that identifies the read site.
; Structural, not textual: matches the call shape, not the substring "open(".
; Path filtering happens in [match] so contributors can extend it without tree-sitter.

; open("~/.aws/credentials")
(call
  function: (identifier) @_fn
  arguments: (argument_list . (string) @path)
  (#eq? @_fn "open")) @site

; pathlib.Path("~/.ssh/id_rsa").read_text()
(call
  function: (attribute
    object: (call
              function: (_) @_ctor
              arguments: (argument_list . (string) @path))
    attribute: (identifier) @_meth)
  (#match? @_meth "^(read_text|read_bytes|open)$")) @site

; Computed target — captured deliberately so the engine can emit
; `unresolved: computed_target` instead of staying silent. Invariant 3.
(call
  function: (identifier) @_fn
  arguments: (argument_list . [(identifier) (binary_operator) (call)] @dynamic)
  (#eq? @_fn "open")) @site
