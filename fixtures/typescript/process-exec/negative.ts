// Describes the commands a caller should run. Naming one is not running one.
//
//     execSync("npm install")
//     child_process.spawn("git", ["status"])

const COMMANDS = ["npm install", "git status --porcelain"];

function describe(runner) {
  // Must NOT fire: `.exec` on a local object. Regex objects have this method,
  // and so do database clients; the property name alone proves nothing.
  return runner.exec(COMMANDS[0]);
}

function parse(pattern, text) {
  // Must NOT fire: this is a regex match, not a process.
  return pattern.exec(text);
}

module.exports = { describe, parse, COMMANDS };
