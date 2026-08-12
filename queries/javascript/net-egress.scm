; Sends data to, or fetches data from, a network endpoint, in javascript.
;
; Shared by the javascript and typescript rules — the node kinds line up, which
; is the same arrangement the dotenv rules already use.
;
; No [match] block and no host capture; see the rule TOML for why.

; fetch(url) — the global, as a bare identifier.
;
; Bare ONLY. `cache.fetch()`, `repo.fetch()`, `db.fetch()` and `imap.fetch()` are
; all real method names on non-network objects, and one of them cost a label in
; the corpus pass: a mail skill's only apparent network site was an IMAP method
; coincidentally called `fetch`, while the actual socket open matched nothing.
; A member-expression pattern on the property name alone would fire on every one.
(call_expression
  function: (identifier) @_fn
  (#eq? @_fn "fetch")) @site

; axios(config) / axios.get(url) / axios.post(url, body)
;
; The object is matched as an identifier, so only a binding actually named
; `axios` qualifies. Same discipline as the python module patterns.
(call_expression
  function: (identifier) @_fn
  (#eq? @_fn "axios")) @site

(call_expression
  function: (member_expression
    object: (identifier) @_mod
    property: (property_identifier) @_verb)
  (#match? @_mod "^(axios|http|https)$")
  (#match? @_verb "^(get|post|put|patch|delete|head|options|request)$")) @site

; new WebSocket(url) — a persistent connection is egress like any other.
(new_expression
  constructor: (identifier) @_ctor
  (#eq? @_ctor "WebSocket")) @site
