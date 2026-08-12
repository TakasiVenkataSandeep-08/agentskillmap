; Reads an environment variable whose NAME indicates a secret.
;
; The name set lives here rather than in `[match]`, and that is forced rather
; than stylistic: `path_suffixes` matches at a component boundary — it requires a
; `/` before the pattern — so `OPENAI_API_KEY` against `_API_KEY` is false, and
; `path_prefixes` is a plain `starts_with`, which is the wrong end. Secret names
; are suffix-shaped. A `#match?` regex is still data, not code; the whole
; instruction plane is built this way.
;
; THE REGEX IS TUNED FROM THE LABELS, NOT FROM IMAGINATION. Every environment
; read in the 92 labelled bundles was extracted and split by whether its bundle
; carries the term. Against that: 28 of 28 secret-bearing names match, 0 of 38
; non-secret names do — including the traps a looser pattern would catch,
; `CACHE_KEY`, `PRIMARY_KEY`, `SORT_KEY`, `MAX_TOKENS`, `TOKENIZER`, `CLIENT_ID`
; and `TENANT_ID`. That check is what the labels are FOR: they were made by
; judging names, before this regex existed, so it can be audited against them
; rather than the other way round.
;
; Anchored at the end, and at `^` or `_` at the front. `_KEY$` alone is absent on
; purpose — every cache and every database row has one.

; os.environ.get("OPENAI_API_KEY") / os.getenv("STRIPE_SECRET")
;
; Call forms only, and that is the write guard. `os.environ["X"] = v` is an
; assignment to a subscript; there is no way to write through `.get()`, so these
; two patterns cannot match a write no matter what the surrounding statement
; does. The subscript read form is deliberately absent — see the rule's
; false_positive_notes for what that costs.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string) @_name)
  (#eq? @_mod "os")
  (#eq? @_fn "getenv")
  (#match? @_name "(?i)['\"_](API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)['\"]$")) @site

(call
  function: (attribute
    object: (attribute
      object: (identifier) @_mod
      attribute: (identifier) @_env)
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string) @_name)
  (#eq? @_mod "os")
  (#eq? @_env "environ")
  (#eq? @_fn "get")
  (#match? @_name "(?i)['\"_](API_?KEY|APIKEY|ACCESS_?KEY|SECRET|PRIVATE_?KEY|TOKEN|JWT|PASSWORD|PASSWD|PASS|CREDENTIALS?|JSESSIONID)['\"]$")) @site
