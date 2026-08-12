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

// must fire: the directory names the credential store and the filename does
// not. Per-integration filenames cannot be enumerated, so neither a prefix nor
// a suffix list reaches this — `path_contains` is the only mode that does.
const os = require("os");
const path = require("path");
function loadIntegration() {
  const configPath = path.join(os.homedir(), ".clawdbot", "credentials", "homebridge.json");
  return JSON.parse(fs.readFileSync(configPath, "utf8"));
}
module.exports = { loadIntegration };
