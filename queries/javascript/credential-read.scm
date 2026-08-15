; Reads a path conventionally holding credentials, in JavaScript.
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
  arguments: (arguments . [(identifier) (member_expression) (subscript_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#match? @_fn "^(readFile|readFileSync|createReadStream|openSync)$")) @site

; The same, after a destructuring import: `import { readFileSync } from 'fs'`.
;
; This branch was missing, and its absence was silent in the worst way. The
; literal form above had both a member-expression and a bare-identifier variant;
; the computed form had only the member-expression one. So
; `fs.readFileSync(CONFIG_FILE)` produced `unresolved: computed_target` and
; `readFileSync(CONFIG_FILE)` produced nothing at all — for the same read, in the
; same language, differing only by import style. The destructured form is the
; modern idiom.
;
; Found by the T3 labelling pass, on a wallet skill reading an API key out of a
; config file under ~/.config. Python has had both branches since the reference
; rule; JavaScript and TypeScript did not.
(call_expression
  function: (identifier) @_fn
  arguments: (arguments . [(identifier) (member_expression) (subscript_expression) (call_expression) (template_string) (binary_expression)] @dynamic)
  (#match? @_fn "^(readFile|readFileSync|createReadStream|openSync)$")) @site
