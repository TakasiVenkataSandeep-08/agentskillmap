; instruction.config_mutation
;
; Prose directing edits to agent configuration. A skill that rewrites the agent's
; own config changes what every later session does, which outlives the skill's own
; trigger and is not something a reviewer reading SKILL.md would expect.
;
; T13 measured the first version of this rule at 24/40 precision and 24/48 recall
; over 145 hand-labelled bundles, and the misses had one cause: the patterns
; anchored on ADJACENCY that real prose does not respect.
;
;   "add the Composio MCP server"     - a product name between article and noun
;   "Add <url> as an MCP server"      - a URL there instead
;   "configuring the Stop hook"       - an adjective there instead
;   "create or update CLAUDE.md with" - no preposition at all
;
; The last of those cost the most: one integration template phrases its setup as
; "Add <endpoint> as an MCP server in your client configuration", and that exact
; shape appears in 1915 of 34302 bundles - 5.58% of the corpus - against 193 the
; old rule fired on in total. It was missing roughly ten times what it caught.
;
; This version drops the adjacency requirement and gains a guard instead.
;
; THE HOOK GUARD, and why the bare noun had to go. `hook` does four different
; jobs in this corpus: an agent hook, a git pre-commit hook, a React hook, and a
; vendor CLI subcommand named `hooks`. The old rule matched the bare word after
; an install verb, so every git-hooks skill fired - one of them thirty-five times
; in a single document, touching no agent configuration at all. A hook now counts
; only when a named agent event or an agent config path is nearby, which is what
; makes it an agent hook rather than a word.
;
; Measured on the same 145 bundles: precision 60.0% to 65.5%, recall 50.0% to
; 79.2%. Still short of the 97-100% the other instruction signals hold, and
; shipped anyway because it beats the rule it replaces on BOTH axes - refusing an
; improvement for missing a bar the current version also misses would leave users
; with the worse rule. What remains is not reachable by a pattern: security
; scanners enumerating the shapes they detect, and command-reference tables.
;
; Periods are allowed throughout. The other rules exclude them to keep a match
; inside one sentence, and here that exclusion is what stopped a URL or a
; filename from sitting between the verb and its object. The line bound is kept
; by excluding newlines instead.

; THE VERB LIST IS DERIVED FROM THE CORPUS, NOT INVENTED.
;
; The object side of this rule was never guesswork - an agent config filename is
; an agent config filename. The verb side was, so it was measured: every line in
; the 34,413 unlabelled bundles naming one of those files was collected, and the
; leading word of each was counted. 31,197 lines.
;
; What that surfaced, and what was done with it:
;
;   configure  36 bundles   SHIPPED - "Configure in ~/.openclaw/openclaw.json:"
;   enable      3           SHIPPED - "Enable in openclaw.json:"
;   store       5           REFUSED - directive when read, and it cost precision
;   put         2           REFUSED - on the labelled set. Together these four
;   place       1           REFUSED - bring 11 bundles and one false positive,
;   define      1           REFUSED - so the ground truth decided against them
;   set        19           REFUSED - splits between directing a value and
;                           describing one; produced a false positive on read
;   save        3           REFUSED - produced the other false positive, on a
;                           sentence about unsaved sessions and last save
;   check     199           REFUSED - reading a config file, not writing one
;   use       653           REFUSED - tables of contents and advice
;   ensure    108           REFUSED, and this one cost something. It is the verb
;                           in a malicious bundle this project has labelled and
;                           still misses - "ensure your MCP settings include" -
;                           but the corpus says `ensure X exists` is usually a
;                           verification. Adding it to catch one bundle already
;                           known to be bad is fitting, so it stays out and the
;                           miss stands.
;
; THE HONEST RESULT, and it is not the one this work set out to produce. On the
; 145 hand-labelled bundles the derived verbs change NOTHING: recall stays 38/48
; and precision is unmoved. Corpus-wide the two kept verbs bring 39 bundles the
; invented list missed - 0.11% against the 1.69% already firing.
;
; The hand-written list was, empirically, close to right. An external review
; predicted that deriving it would take the instruction plane from 9.5% coverage
; to something a reviewer could rely on; the measurement says it moves it by a
; tenth of a percent. The prediction was wrong, the method that tested it was
; the same one used everywhere else here, and the negative result is written
; down at the size it actually is rather than dressed up.
;
; What this DOES establish is that the verb list is no longer the bottleneck,
; which is worth knowing before anyone spends another pass on it.

; THE VERB MUST NOT BE PART OF A DOTTED IDENTIFIER. `fs.write.agent_config` is a
; capability term this project's own docs name constantly, and a dot is a word
; boundary, so a bare word-boundary match found "write" inside it and fired on a
; sentence explaining what the term covers. The negative fixture caught it.
; Rust's regex has no lookbehind, so the preceding character is matched instead.

; An edit directed at a file the agent loads as configuration.
(((inline) @site)
  (#match? @site
    "(?i)(^|[^.\w])(add|append|write|edit|modify|update|insert|create|copy|paste|configure|enable)\\b[^\n]{0,70}(CLAUDE\.md|AGENTS\.md|settings\.json|settings\.local\.json|\.mcp\.json|mcp\.json|openclaw\.json)"))

(((fenced_code_block
    (code_fence_content) @site))
  (#match? @site
    "(?i)(^|[^.\w])(add|append|write|edit|modify|update|insert|create|copy|paste|configure|enable)\\b[^\n]{0,70}(CLAUDE\.md|AGENTS\.md|settings\.json|settings\.local\.json|\.mcp\.json|mcp\.json|openclaw\.json)"))

; Registering something the agent will load every session. `hook` is absent here
; on purpose - see the guard below.
(((inline) @site)
  (#match? @site
    "(?i)\\b(register|install|configure|configuring|set up|add|enable)\\b[^\n]{0,60}\\b(statusline|mcp server|subagent)s?\\b"))

; A hook, only when the surrounding text makes it an AGENT hook.
(((inline) @site)
  (#match? @site
    "(?i)\\bhooks?\\b[^\n]{0,80}(SessionStart|PreToolUse|PostToolUse|UserPromptSubmit|SubagentStop|Notification|settings\.json|\.claude/|\.openclaw/)"))

(((inline) @site)
  (#match? @site
    "(?i)(SessionStart|PreToolUse|PostToolUse|UserPromptSubmit|SubagentStop)[^\n]{0,80}\\bhooks?\\b"))
