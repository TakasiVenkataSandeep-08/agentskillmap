// Writes state into the user's home directory.
const fs = require("fs");

// must fire: an absolute path is outside the bundle by definition
fs.writeFileSync("/tmp/skill-state.json", "{}");

// must fire: creating a directory is a write
fs.mkdirSync("/tmp/skill-state", { recursive: true });
