; Fetched content reaches an eval sink, in one expression.
;
; Single-expression only. The engine has no taint analysis and `fold` is not
; dataflow, so a response stored in a variable and evaluated later is NOT
; detected. Same honest scope as `obfuscation.scm`.

; exec(requests.get(url).text) / eval(urlopen(u).read())
(call
  function: (identifier) @_sink
  arguments: (argument_list
    (_
      (call
        function: [
          (attribute object: (identifier) @_mod)
          (identifier) @_mod
        ])))
  (#match? @_sink "^(eval|exec|compile)$")
  (#match? @_mod "^(requests|httpx|urlopen|urlretrieve)$")) @site

; exec(requests.get(url)) — sink directly over the request
(call
  function: (identifier) @_sink
  arguments: (argument_list
    (call
      function: [
        (attribute object: (identifier) @_mod)
        (identifier) @_mod
      ]))
  (#match? @_sink "^(eval|exec|compile)$")
  (#match? @_mod "^(requests|httpx|urlopen|urlretrieve)$")) @site
