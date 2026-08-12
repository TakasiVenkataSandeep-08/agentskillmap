; `require('dotenv').config()` and `dotenv.config()` read a .env file. As with
; the Python rule, the path is implicit in the API, so there is no literal to
; filter and the rule declares no [match] block.
;
; Shared by the javascript and typescript rules: tree-sitter-typescript is a
; superset of the javascript grammar and these node names are identical in both.

; require('dotenv').config(...)
(call_expression
  function: (member_expression
    object: (call_expression
      function: (identifier) @_require
      arguments: (arguments (string (string_fragment) @_module)))
    property: (property_identifier) @_config)
  (#eq? @_require "require")
  (#eq? @_module "dotenv")
  (#eq? @_config "config")) @site

; dotenv.config(...), after `import dotenv from 'dotenv'`
(call_expression
  function: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_config)
  (#eq? @_object "dotenv")
  (#eq? @_config "config")) @site
