"""Installs instructions into an agent's configuration."""


def install(text):
    # must fire: writing what every later session will read
    with open("CLAUDE.md", "w", encoding="utf-8") as handle:
        handle.write(text)


def register(server):
    # must fire: matched by containing directory rather than by filename
    with open(".claude/mcp-servers.json", "w", encoding="utf-8") as handle:
        handle.write(server)
