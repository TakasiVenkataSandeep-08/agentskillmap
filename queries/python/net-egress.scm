; Sends data to, or fetches data from, a network endpoint.
;
; The shape of the CALL is the whole of the evidence, which is why the rule
; declares no [match] block: there is no path to filter, and a host list would be
; an allowlist of destinations rather than a description of what the code does.
; Same reasoning as the dotenv rules.
;
; Every pattern here matches a call that actually issues a request. Constructing
; a client does not — `requests.Session()` and `httpx.Client()` are deliberately
; absent, because a session nobody calls a verb on sends nothing, and the
; negative fixture pins that.

; requests.get(url) / httpx.post(url, json=...) / requests.request("GET", url)
;
; The module is matched as an identifier rather than by suffix, so a local object
; that happens to expose `.get` — a dict, a cache, a config — cannot match. That
; is the trap this pattern exists to avoid: `.get(` is one of the most common
; method names in any language.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_verb)
  (#match? @_mod "^(requests|httpx)$")
  (#match? @_verb "^(get|post|put|patch|delete|head|options|request|stream)$")) @site

; urllib.request.urlopen(req)
(call
  function: (attribute
    attribute: (identifier) @_fn)
  (#match? @_fn "^(urlopen|urlretrieve)$")) @site

; urlopen(req), from `from urllib.request import urlopen`.
;
; The destructured-import branch, written at the same time as the one above and
; not after it. This project has now shipped that omission three separate times —
; in the javascript credential query, in the dotenv rules, and in the labelling
; pass's own mechanism filter, where a bundle showed two network sites that were
; both the import line while the real call three lines down matched nothing.
(call
  function: (identifier) @_fn
  (#match? @_fn "^(urlopen|urlretrieve)$")) @site

; socket.create_connection((host, port)) — an explicit outbound connection.
;
; `socket.socket(...)` is absent on purpose: it allocates a socket and connects
; nothing. The connect call is the egress.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  (#eq? @_mod "socket")
  (#eq? @_fn "create_connection")) @site

; http.client.HTTPSConnection(host) / HTTPConnection(host)
(call
  function: (attribute
    attribute: (identifier) @_ctor)
  (#match? @_ctor "^(HTTPConnection|HTTPSConnection)$")) @site
