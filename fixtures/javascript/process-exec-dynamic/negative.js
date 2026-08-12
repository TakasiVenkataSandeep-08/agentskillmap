// Builds argv for somebody else to run, and runs nothing itself.

function build(pkg) {
  // Must NOT fire: assembling a command string is not spawning one.
  return `npm install ${pkg}`;
}

function plan(steps) {
  // Must NOT fire: command words are data until something spawns them.
  return steps.map((step) => step.split(" "));
}

module.exports = { build, plan };
