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

; fs.mkdirSync(dir) / fs.rmSync(p) / fs.unlinkSync(p)
;
; The dominant missed shape, same as python. Matched through the `fs` object so
; `.rename(` and `.rm(` cannot pick up unrelated methods — both are ordinary
; names on ORMs and collections.
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (string) @path)
  (#eq? @_mod "fs")
  (#match? @_fn "^(mkdir|mkdirSync|rm|rmSync|rmdir|rmdirSync|unlink|unlinkSync|truncate|truncateSync)$")) @site

(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . [(identifier) (member_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#eq? @_mod "fs")
  (#match? @_fn "^(mkdir|mkdirSync|rm|rmSync|rmdir|rmdirSync|unlink|unlinkSync|truncate|truncateSync)$")) @site

; mkdirSync(dir) — destructured import, written as a pair with the member form.
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . (string) @path)
  (#match? @_fn "^(mkdirSync|rmSync|rmdirSync|unlinkSync)$")) @site

(call_expression
  function: (identifier) @_fn
  arguments: (arguments . [(identifier) (member_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#match? @_fn "^(mkdirSync|rmSync|rmdirSync|unlinkSync)$")) @site

; fs.copyFileSync(src, dst) / fs.renameSync(from, to) — the SECOND argument is
; written; matching the first would report the source as a write.
(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (_) (string) @path)
  (#eq? @_mod "fs")
  (#match? @_fn "^(copyFile|copyFileSync|cp|cpSync|rename|renameSync)$")) @site

(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_fn)
  arguments: (arguments . (_) [(identifier) (member_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#eq? @_mod "fs")
  (#match? @_fn "^(copyFile|copyFileSync|cp|cpSync|rename|renameSync)$")) @site
