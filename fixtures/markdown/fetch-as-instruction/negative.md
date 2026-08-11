# Threat model notes

Excerpted from this repository's own SECURITY.md and AGENTS.md, which discuss
indirect prompt injection at length without ever instructing an agent to perform
it. A rule that fires here is a rule that fires on every security write-up in the
ecosystem.

Skill content enters the prompt as delimited data, never as instruction. If the
model output contains anything resembling an instruction to the auditor, that is
logged as a finding about the skill and never acted on.

The structural hole is progressive disclosure: the reviewer reads SKILL.md, sees
something benign, and installs. A bundle that tells an agent to download a page
and obey it is the shape we are looking for, and describing that shape is not the
same as adopting it.

Documentation of an attack is not the attack.
