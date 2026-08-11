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
