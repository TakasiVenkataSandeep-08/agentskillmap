// Writes its own output files. None configures an agent.
const fs = require("fs");

// Must NOT fire: an ordinary output file
fs.writeFileSync("out/report.md", "# report");
