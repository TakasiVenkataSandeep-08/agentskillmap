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

; import "dotenv/config"  — a side-effect import that loads .env on evaluation.
(import_statement
  source: (string (string_fragment) @_module)
  (#eq? @_module "dotenv/config")) @site

; import { config } from "dotenv"  — the destructured form.
;
; The *import* is the site here, not the call, and that is a deliberate weakening
; worth being explicit about. `config()` after a destructuring import is a bare
; call to a very common name; matching every call named `config` would be a large
; false-positive source, and a tree-sitter query cannot tell that this particular
; `config` is the one bound three lines above.
;
; So the binding is the evidence. A file that names `config` out of `dotenv` and
; never calls it is dead code, which is the only way this over-reports.
;
; This is the second time the destructured form was the missing one — the
; credential-read queries had exactly the same hole, found in the previous batch
; of the labelling pass. Writing the member-expression form and forgetting the
; destructured one is evidently a habit, and the corpus caught it twice.
(import_statement
  (import_clause
    (named_imports
      (import_specifier name: (identifier) @_name)))
  source: (string (string_fragment) @_module)
  (#eq? @_name "config")
  (#eq? @_module "dotenv")) @site
