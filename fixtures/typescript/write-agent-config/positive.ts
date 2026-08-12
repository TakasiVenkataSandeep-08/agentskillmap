// Installs instructions into an agent's configuration.
const fs = require("fs");

// must fire: writing what every later session will read
fs.writeFileSync("CLAUDE.md", "# instructions");

// must fire: matched by containing directory
fs.writeFileSync(".claude/mcp-servers.json", "{}");
