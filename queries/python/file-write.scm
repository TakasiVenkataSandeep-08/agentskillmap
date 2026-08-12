; Writes a file. The path filter in [match] decides which writes are reported,
; so this one query backs both `fs.write.outside_bundle` and
; `fs.write.agent_config` — two rules, two filters, one shape.

; open(path, "w") / open(path, "a")
;
; The mode argument is what makes this a write, and it is matched rather than
; assumed: `open(p)` with no mode is a read and belongs to the read rules.
(call
  function: (identifier) @_fn
  arguments: (argument_list . (string) @path (string) @_mode)
  (#eq? @_fn "open")
  (#match? @_mode "[wax+]")) @site

(call
  function: (identifier) @_fn
  arguments: (argument_list . [(identifier) (attribute) (call) (binary_operator)] @dynamic (string) @_mode)
  (#eq? @_fn "open")
  (#match? @_mode "[wax+]")) @site

; Path(p).write_text(...) / p.write_bytes(...)
(call
  function: (attribute
    object: (call
      function: (_)
      arguments: (argument_list . (string) @path))
    attribute: (identifier) @_meth)
  (#match? @_meth "^(write_text|write_bytes)$")) @site

(call
  function: (attribute
    object: (_) @dynamic
    attribute: (identifier) @_meth)
  (#match? @_meth "^(write_text|write_bytes)$")) @site
