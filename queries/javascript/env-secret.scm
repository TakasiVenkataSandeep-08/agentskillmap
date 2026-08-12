; Reads an environment variable whose NAME indicates a secret, in javascript.
;
; Shared by javascript and typescript. The name set lives in a `#match?` rather
; than in `[match]` because `path_suffixes` matches at a `/` boundary and secret
; names are suffix-shaped; see the python query for the full reasoning and for
; how the regex was tuned against the labelled corpus.
;
; EVERY PATTERN ANCHORS ON A READ CONTEXT, and that is the write guard.
;
; `process.env.OPENAI_API_KEY` is the dominant idiom in this corpus, so unlike
; python it cannot be skipped — but `process.env.OPENAI_API_KEY = "x"` is the
; same node in an assignment's `left` field, and tree-sitter has no negation to
; exclude it with. So instead of matching the access anywhere and hoping, each
; pattern below names a parent in which the access is unambiguously a value
; being consumed.
;
; This repository has already shipped the other mistake once: the shell rule
; reported `cat > .env` — a WRITE — as a credential read, because the grammar
; uses one node for every redirect direction and the query never said which it
; wanted. The README lists it as one of three defects the labelling pass found.
; Reporting a hand-rolled dotenv loader as a credential *reader* would be the
; same error with the same shape, and two bundles in the corpus write the
; environment exactly that way.
;
; The cost is that a read in a context not listed here is missed. The list covers
; every shape the labelled corpus actually uses; it is an enumeration, and an
; enumeration is incomplete by construction.

; const key = process.env.OPENAI_API_KEY
(variable_declarator
  value: (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; { apiKey: process.env.OPENAI_API_KEY }
(pair
  value: (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; f(process.env.OPENAI_API_KEY)
(arguments
  (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; process.env.OPENAI_API_KEY || fallback
(binary_expression
  (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; return process.env.OPENAI_API_KEY
(return_statement
  (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; !process.env.OPENAI_API_KEY
(unary_expression
  (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; `${process.env.OPENAI_API_KEY}`
(template_substitution
  (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))

; x = process.env.OPENAI_API_KEY
(assignment_expression
  right: (member_expression
    object: (member_expression
      object: (identifier) @_p
      property: (property_identifier) @_e)
    property: (property_identifier) @_name) @site
  (#eq? @_p "process")
  (#eq? @_e "env")
  (#match? @_name "(?i)(API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)$"))
