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

; path.mkdir(parents=True) / export_dir.mkdir(exist_ok=True)
;
; The dominant missed shape. Creating a directory is a write to the filesystem,
; and every `outside_bundle` miss that turned out to be a state directory under
; the home folder reached it this way.
;
; Attribute form on any receiver, which is safe for `mkdir` in a way it is NOT
; for the neighbouring verbs: `.rename(` is a pandas DataFrame method and
; `.replace(` is a string method, so both are matched only through `os.` below.
; That distinction is the same one `.get(`, `.fetch(` and `.exec(` already
; forced elsewhere in this rule set.
(call
  function: (attribute
    object: (_) @dynamic
    attribute: (identifier) @_meth)
  (#match? @_meth "^(mkdir|unlink|touch)$")) @site

; os.makedirs(path) / os.remove(path) / shutil.rmtree(path)
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string) @path)
  (#match? @_mod "^(os|shutil)$")
  (#match? @_fn "^(makedirs|mkdir|remove|unlink|rename|replace|rmtree|rmdir)$")) @site

(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . [(identifier) (attribute) (call) (binary_operator) (subscript)] @dynamic)
  (#match? @_mod "^(os|shutil)$")
  (#match? @_fn "^(makedirs|mkdir|remove|unlink|rename|replace|rmtree|rmdir)$")) @site

; shutil.copy(src, dst) / shutil.move(src, dst) — the SECOND argument is written.
; Matching the first would report the source as a write, the same direction error
; the shell redirect rule exists to avoid.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (_) (string) @path)
  (#eq? @_mod "shutil")
  (#match? @_fn "^(copy|copy2|copyfile|copytree|move)$")) @site

(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (_) [(identifier) (attribute) (call) (binary_operator)] @dynamic)
  (#eq? @_mod "shutil")
  (#match? @_fn "^(copy|copy2|copyfile|copytree|move)$")) @site
