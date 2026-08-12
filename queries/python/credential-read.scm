; Capture the smallest span that identifies the read site.
; Structural, not textual: matches the call shape, not the substring "open(".
; Path filtering happens in [match] so contributors can extend it without tree-sitter.

; open("~/.aws/credentials")
(call
  function: (identifier) @_fn
  arguments: (argument_list . (string) @path)
  (#eq? @_fn "open")) @site

; pathlib.Path("~/.ssh/id_rsa").read_text()
(call
  function: (attribute
    object: (call
              function: (_) @_ctor
              arguments: (argument_list . (string) @path))
    attribute: (identifier) @_meth)
  (#match? @_meth "^(read_text|read_bytes|open)$")) @site

; Computed target — captured deliberately so the engine can emit
; `unresolved: computed_target` instead of staying silent. Invariant 3.
(call
  function: (identifier) @_fn
  arguments: (argument_list . [(identifier) (binary_operator) (call)] @dynamic)
  (#eq? @_fn "open")) @site

; `p.read_text()` where `p` is a variable, not `Path("literal").read_text()`.
;
; The literal form above has a pattern; the computed form did not, so
; `env_file = BASE_DIR / '.env'` followed by `env_file.read_text()` produced
; **nothing at all** — no capability and no unresolved entry. Two bundles in the
; T3 labelling pass read credentials exactly that way, and both were silent.
;
; This is the third time in this labelling pass that a query had a literal branch
; and no matching computed branch: the same hole appeared in the JavaScript and
; TypeScript credential-read queries (destructured imports) and again in the
; dotenv rules. Writing the shape you are thinking of and forgetting its
; variable-valued twin is evidently the recurring mistake in this rule set.
;
; The object is `(_)` rather than a list of node kinds: `(BASE / '.env').read_text()`
; wraps its receiver in a parenthesized expression, and enumerating shapes is how
; the gaps this pass keeps finding get made. Folding decides what the receiver is;
; the query's job is only to find the call.
;
; It over-captures: any object with a `read_text` method matches. The cost is a
; noisier `unresolved` list, never a false capability — the engine turns a
; `dynamic` capture into `computed_target`, and invariant 3 prefers a visible
; "could not resolve" to silence.
(call
  function: (attribute
    object: (_) @dynamic
    attribute: (identifier) @_meth)
  (#match? @_meth "^(read_text|read_bytes)$")) @site
