; `load_dotenv()` reads a .env file. The path is implicit in the API, which is
; why this rule declares no [match] block: there is no literal to filter, and the
; function name is the whole of the evidence.
;
; Found by the T3 labelling pass. Of the four real credential reads in the first
; sixteen labelled bundles, three were dotenv-shaped and the scanner detected
; none of them — `open()` with a literal path, the only shape covered until now,
; turns out not to be how this ecosystem reads credentials.

; load_dotenv()  /  load_dotenv(".env.local")  /  load_dotenv(dotenv_path=p)
(call
  function: (identifier) @_fn
  (#eq? @_fn "load_dotenv")) @site

; dotenv.load_dotenv()
(call
  function: (attribute attribute: (identifier) @_fn)
  (#eq? @_fn "load_dotenv")) @site
