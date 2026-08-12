; Egress reached through a vendor SDK, where no protocol is named at the call
; site. Shared by javascript and typescript.
;
; THIS IS THE LARGEST MEASURED GAP IN `net.egress`, and it is invisible to every
; pattern the other rule uses. `openai.chat.completions.create(...)` reaches a
; hosted API without the word http appearing anywhere; the receiver is a local
; name bound to `new OpenAI(...)` somewhere else in the file, and the engine
; cannot follow a receiver to its constructor.
;
; So the METHOD CHAIN is the evidence, not the receiver. That works here and
; would not work for `.get(` or `.fetch(`, because these chains are shapes
; nothing else has reason to write.
;
; Two families, both drawn from the corpus rather than from a vendor list:
; four labelled bundles reach an LLM API this way and two reach an EVM node.
; `Linkedin(...)` and `enable_remote_sync(...)` are deliberately NOT here — each
; appears in exactly one bundle, and naming them would raise recall while
; lowering what the number means, which is the same call Phase 1 made about
; `.beanstalk` and `.fluxa-ai-wallet-mcp`.

; client.chat.completions.create(...) — the OpenAI-compatible shape, which groq,
; together, mistral, deepseek, ollama and openrouter all also speak.
;
; Three levels deep on purpose. `.create(` alone is an ORM method, `.completions`
; alone is not a thing, and the full chain is unambiguous.
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        property: (property_identifier) @_a)
      property: (property_identifier) @_b)
    property: (property_identifier) @_c)
  (#match? @_a "^(chat|beta)$")
  (#match? @_b "^(completions|messages|responses|embeddings|images)$")
  (#match? @_c "^(create|generate|stream|parse)$")) @site

; client.messages.create(...) / client.images.generate(...) — the two-level
; Anthropic and image shapes.
(call_expression
  function: (member_expression
    object: (member_expression
      property: (property_identifier) @_b)
    property: (property_identifier) @_c)
  (#match? @_b "^(completions|embeddings)$")
  (#match? @_c "^(create|stream)$")) @site

; viem / ethers: an EVM client talks to a node over RPC.
;
; `createPublicClient` and `createWalletClient` construct one; `readContract`,
; `writeContract` and `sendTransaction` are the calls that reach the chain.
; Unlike `requests.Session()`, the constructors are included because they take a
; transport and a chain — they name a remote endpoint at the call site, which is
; what the session did not.
(call_expression
  function: (identifier) @_fn
  (#match? @_fn "^(createPublicClient|createWalletClient)$")) @site

(call_expression
  function: (member_expression
    property: (property_identifier) @_m)
  (#match? @_m "^(readContract|writeContract|sendTransaction|estimateGas|waitForTransactionReceipt)$")) @site
