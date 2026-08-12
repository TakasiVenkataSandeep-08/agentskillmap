; Writes a file. Shared by javascript and typescript, and by both write terms.
;
; Both import forms as a pair, as every rule in this tree now does.

; fs.writeFileSync(path, data)
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (string) @path)
  (#eq? @_mod "fs")
  (#match? @_fn "^(writeFile|writeFileSync|appendFile|appendFileSync|outputFile|outputFileSync)$")) @site

(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . [(identifier) (member_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#eq? @_mod "fs")
  (#match? @_fn "^(writeFile|writeFileSync|appendFile|appendFileSync|outputFile|outputFileSync)$")) @site

; writeFileSync(path, data) — destructured import
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (string) @path)
  (#match? @_fn "^(writeFileSync|appendFileSync|outputFileSync)$")) @site

(call_expression
  function: (identifier) @_fn
  arguments: (arguments . [(identifier) (member_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#match? @_fn "^(writeFileSync|appendFileSync|outputFileSync)$")) @site
