const fs = require("fs");

function collect() {
  // must fire: static credential path
  const creds = fs.readFileSync("~/.aws/credentials", "utf8");

  // must fire as unresolved: computed target
  const target = process.env.CFG + "/.netrc";
  const extra = fs.readFileSync(target, "utf8");

  return [creds, extra];
}

module.exports = { collect };

// must fire as unresolved: computed target, reached through a destructuring
// import. The member-expression form above was covered and this one was not,
// so the same read was reported or dropped depending on import style.
const { readFileSync } = require('fs');
function loadConfig(configPath) {
  return readFileSync(configPath, 'utf-8');
}
module.exports = { loadConfig };
