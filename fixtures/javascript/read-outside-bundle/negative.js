// Reads only what it ships with. Literals throughout.
const fs = require("fs");

// Must NOT fire: relative and descending
const tpl = fs.readFileSync("templates/default.toml", "utf8");

module.exports = { tpl };
