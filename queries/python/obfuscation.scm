; An encoding/decoding chain feeding a sink.
;
; SINGLE-EXPRESSION CHAINS ONLY. The engine has no taint analysis and `fold` is
; not dataflow, so the honest scope of this rule is "the decode and the sink are
; written in one expression". A payload decoded on one line and evaluated three
; lines later is not detected, and pretending otherwise would be claiming an
; analysis this tool does not perform.
;
; Decoding alone is not the signal — every skill that touches an image or a JWT
; calls b64decode. What makes it worth reporting is decode AND execute in the
; same breath, which is not something a legitimate program has reason to write.

; eval(base64.b64decode(PAYLOAD).decode())
; exec(base64.b64decode(PAYLOAD))
;
; One pattern covers both depths: in the first the matched call is the outer
; `.decode()`, in the second it is `.b64decode(...)`. Either way the sink's
; argument is a call whose function is an attribute with a decoding name.
(call
  function: (identifier) @_sink
  arguments: (argument_list
    (call
      function: (attribute
        attribute: (identifier) @_dec)))
  (#match? @_sink "^(eval|exec|compile)$")
  (#match? @_dec "^(b64decode|b64encode|a85decode|b32decode|b16decode|decodebytes|decodestring|unhexlify|decompress|decode|fromhex)$")) @site

; eval(codecs.decode(payload, "rot13")) — the decoder reached as a bare name
; through `from base64 import b64decode`. Written as a pair with the branch
; above, which is the omission this project has shipped three times.
(call
  function: (identifier) @_sink
  arguments: (argument_list
    (call
      function: (identifier) @_dec))
  (#match? @_sink "^(eval|exec|compile)$")
  (#match? @_dec "^(b64decode|a85decode|b32decode|b16decode|decodebytes|unhexlify|decompress|fromhex)$")) @site
