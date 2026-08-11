# Reading agent configuration

Excerpted from this repository's own architecture notes, which describe agent
configuration files in order to explain what the scanner looks at.

The disclosure delta is capabilities present in reference files but not implied
by always content. Writes to CLAUDE.md, settings.json, hooks, and statusline
config are one of the capability terms in the closed taxonomy.

`fs.write.agent_config` covers writing CLAUDE.md, settings.json, hook or
statusline config. Naming a capability is not exercising it.
