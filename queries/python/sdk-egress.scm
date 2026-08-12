; Egress reached through a vendor SDK, in python. See the javascript query for
; why the method chain is the evidence rather than the receiver.

; client.chat.completions.create(...)
(call
  function: (attribute
    object: (attribute
      object: (attribute
        attribute: (identifier) @_a)
      attribute: (identifier) @_b)
    attribute: (identifier) @_c)
  (#match? @_a "^(chat|beta)$")
  (#match? @_b "^(completions|messages|responses|embeddings|images)$")
  (#match? @_c "^(create|generate|stream|parse)$")) @site

; client.completions.create(...) / client.embeddings.create(...)
(call
  function: (attribute
    object: (attribute
      attribute: (identifier) @_b)
    attribute: (identifier) @_c)
  (#match? @_b "^(completions|embeddings)$")
  (#match? @_c "^(create|stream)$")) @site
