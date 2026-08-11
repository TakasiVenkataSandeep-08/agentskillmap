; Reads a path conventionally holding credentials, in TypeScript.
;
; Matches the call shape, never the substring "readFile". Path filtering lives in
; [match] so a contributor can extend coverage without tree-sitter.

; fs.readFileSync("~/.aws/credentials")  /  await fs.promises.readFile(...)
(call_expression
  function: (member_expression
    property: (property_identifier) @_fn)
  arguments: (arguments . (string) @path)
  (#match? @_fn "^(readFile|readFileSync|createReadStream|openSync|open)$")) @site

; readFile("~/.ssh/id_rsa") after a destructuring import
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (string) @path)
  (#match? @_fn "^(readFile|readFileSync|createReadStream)$")) @site

; Computed target — reported as unresolved, never dropped (invariant 3).
(call_expression
  function: (member_expression
    property: (property_identifier) @_fn)
  arguments: (arguments . [(identifier) (template_string) (binary_expression) (call_expression)] @dynamic)
  (#match? @_fn "^(readFile|readFileSync|createReadStream|openSync)$")) @site
