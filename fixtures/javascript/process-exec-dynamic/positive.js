// Runs a configured helper. argv[0] is not knowable from source.
const { execSync } = require("child_process");
const child_process = require("child_process");

function runConfigured(cmd) {
  // Deliberately does NOT fire, and this is the documented miss: a bare
  // identifier may well be bound to a literal one line up, and the query cannot
  // tell a name that folds from one that does not. Claiming the program is
  // unknowable here would be a false claim in the corpus's common case.
  return execSync(cmd);
}

function runFromConfig(config) {
  // must fire: member form, program read off an object
  return child_process.execSync(config.command);
}

function installPackage(pkg) {
  // must fire: interpolated command line, matched on the substitution
  return execSync(`npm install ${pkg}`);
}

module.exports = { runConfigured, runFromConfig, installPackage };
