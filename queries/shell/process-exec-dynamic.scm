; Spawns a process whose program is not statically known, in shell.
;
; SHELL DELIBERATELY HAS NO `process.exec` RULE, only this one. In python or
; javascript, calling `subprocess` is a distinct act a reader learns something
; from. In shell, running a command is the language's only verb — `echo` is an
; exec — so a static-exec rule would fire on every `.sh` file in the corpus and
; its recall would measure how many bundles ship a shell script. The same
; reasoning is written into corpus/labels.toml, where it scopes the
; `process.exec` term out of shell for labelling too, and it is why that term
; reads 5/92 rather than something like 40/92.
;
; What IS worth reporting is the command word not being knowable, because then
; whoever controls the variable controls what runs.
;
; `bash -c "$x"` is deliberately absent: the command word there is `bash`, which
; is perfectly well known. What is computed is the *code* it runs, and that is
; code.dynamic_eval — a different term, kept disjoint.

; $CMD --flag  /  "$RUNNER" args
(command
  name: (command_name (simple_expansion))) @site

(command
  name: (command_name (expansion))) @site

; "$@" — run whatever argv this script was handed. Fully dynamic dispatch, and
; a real shape from the labelled corpus: a security-audit skill wraps every
; probe in a helper that runs its own positional parameters.
(command
  name: (command_name (string . (simple_expansion) .))) @site
