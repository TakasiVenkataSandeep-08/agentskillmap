// Installs dependencies and records repository state.
const { execSync } = require("child_process");
const child_process = require("child_process");

function install() {
  // must fire: bare form, from a destructured import
  return execSync("npm install");
}

function status() {
  // must fire: member form, same call reached differently
  return child_process.execSync("git status --porcelain");
}

function browsers() {
  // must fire: a template literal with no substitution is still a literal
  return execSync(`npx playwright install chromium`);
}

module.exports = { install, status, browsers };
