; Spawns a process whose program is statically known from source.
;
; The split between this and `process-exec-dynamic.scm` is STRUCTURAL, not
; runtime. A rule declares exactly one capability, and the engine cannot pick a
; term based on how a capture folded, so "the program is knowable" and "the
; program is not" have to be two queries that cannot both match the same site.
; `crates/skillmap-code/tests/fixtures.rs` asserts that disjointness directly —
; if it ever breaks, one call reports both terms and the manifest contradicts
; itself.
;
; What counts as knowable is argv[0], not the whole command line.
; `subprocess.run(["ffmpeg", "-i", user_path])` is `process.exec`: the program is
; ffmpeg no matter what the arguments turn out to be.

; subprocess.run(["git", "status"]) — list form, literal program
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (list . (string)))
  (#eq? @_mod "subprocess")
  (#match? @_fn "^(run|call|check_call|check_output|Popen)$")) @site

; subprocess.run("git status", shell=True) — string form, no interpolation.
;
; The `#not-match?` is what keeps this disjoint from the dynamic query: an
; f-string parses as `(string)` exactly like a plain one, so without it
; `subprocess.run(f"rm {path}")` would be reported as a statically known program,
; which is the opposite of true and the more dangerous direction to be wrong in.
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string) @_prog)
  (#eq? @_mod "subprocess")
  (#match? @_fn "^(run|call|check_call|check_output|Popen)$")
  (#not-match? @_prog "\\{")) @site

; os.system("ls -la") / os.popen("df -h")
(call
  function: (attribute
    object: (identifier) @_mod
    attribute: (identifier) @_fn)
  arguments: (argument_list . (string) @_prog)
  (#eq? @_mod "os")
  (#match? @_fn "^(system|popen)$")
  (#not-match? @_prog "\\{")) @site
