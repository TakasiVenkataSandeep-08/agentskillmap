; instruction.fetch_as_instruction
;
; The bundle's operative instructions are not in the bundle. Something the agent
; will act on arrives from a URL at trigger time, and a reviewer reading the
; shipped file never sees it.
;
; T13 rewrote this rule after measuring the original at 10/30 precision and
; 10/26 recall over 145 hand-labelled bundles. The original tried to describe
; indirect prompt injection in general and matched, instead, security scanners
; enumerating attacks, a terminal simulator defining a brace convention, and
; installer prose that fetches a binary and runs it - which is fetched CODE and
; is already named by net.fetch_then_execute and instruction.exec_directive.
;
; What survived the corpus is narrower and has a shape of its own: a hosted
; service ships a bootstrap document and serves the real one from its own
; domain, refreshed on a heartbeat. 100 of 33871 unlabelled bundles carry it.
;
; TWO NODE TYPES, and the second one is why the first rewrite would have failed.
; The install block that does this is almost always inside a fenced code block,
; and a query matching only (inline) cannot see fence content - verified with a
; probe placing the same sentence in five positions, where paragraphs, lists and
; blockquotes fire and tables and fences do not.
;
; PERIODS ARE DELIBERATELY ALLOWED HERE, unlike every other rule in this
; directory. The others exclude them to keep a match inside one sentence. This
; signal is ABOUT URLs, URLs contain periods, and that exclusion is precisely
; why the shipped rule could not match "Fetch https://host/heartbeat.md and
; follow it". The line bound is kept by excluding newlines instead.

; A KNOWN LOOSENESS, LEFT IN DELIBERATELY AND MEASURED. The redirect is matched
; as a bare `>`, so any angle bracket between the verb and the filename satisfies
; it. This repository's own T13 notes tripped that: a placeholder written as
; `<vendor>` supplied the `>` and the sentence supplied the rest. The obvious fix
; - requiring the next character to begin a path - was written, and it silently
; MATCHED NOTHING AT ALL. A character class placed after `\s*` in a `#match?`
; predicate compiles without a diagnostic and then never fires; `[~/$.]` and
; `[~/.]` both behaved that way, verified by rebuild and smoke test each time.
; Shipping a rule that quietly detects nothing is worse than shipping one that is
; slightly loose, so the loose form stands - it is the form measured at 39/39
; precision over 175 labelled bundles - and the documentation was reworded
; instead. Anyone tightening this must smoke-test the rebuilt binary, not just
; check that the query compiles.

; A remote document written over an instruction file the agent loads.
(((fenced_code_block
    (code_fence_content) @site))
  (#match? @site
    "(?i)(curl|wget|fetch)[^\n]{0,90}>\s*[^\n]{0,60}(SKILL|HEARTBEAT|AGENTS|CLAUDE)[^\n]{0,10}\.md"))

(((inline) @site)
  (#match? @site
    "(?i)(curl|wget|fetch)[^\n]{0,90}>\s*[^\n]{0,60}(SKILL|HEARTBEAT|AGENTS|CLAUDE)[^\n]{0,10}\.md"))

; THE DOCUMENT MUST BE REMOTE. Without the URL requirement this matched a
; troubleshooting tip - capture the curl requests provided in SKILL.md and run
; them locally - where the .md names a file the bundle already ships and nothing
; arrives from anywhere. This term is about instructions that are NOT in the
; bundle, so the address is the thing that makes it the term.

; A markdown document fetched from a URL and then followed, obeyed or executed.
(((inline) @site)
  (#match? @site
    "(?i)\\b(fetch|download|retrieve|curl|wget)\\b[^\n]{0,70}?https?://[^\n]{0,70}?\.md\\b[^\n]{0,40}?\\b(and|then)\\b[^\n]{0,30}?\\b(follow|obey|execute|run)\\b"))

(((fenced_code_block
    (code_fence_content) @site))
  (#match? @site
    "(?i)\\b(fetch|download|retrieve|curl|wget)\\b[^\n]{0,70}?https?://[^\n]{0,70}?\.md\\b[^\n]{0,40}?\\b(and|then)\\b[^\n]{0,30}?\\b(follow|obey|execute|run)\\b"))

; The shipped file declaring itself incomplete and naming where the real one is.
(((inline) @site)
  (#match? @site
    "(?i)\\b(must be read|read this file)\\b[^\n]{0,60}\\b(via|from)\\b[^\n]{0,40}(curl|https?://)"))

; Instructions to follow, located at a URL.
(((inline) @site)
  (#match? @site
    "(?i)\\bfollow\\b[^\n]{0,30}\\b(instruction|step|directive)s?\\b[^\n]{0,30}\\b(from|at|in)\\b[^\n]{0,20}https?://"))
