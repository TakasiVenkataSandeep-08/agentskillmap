const fs = require("fs");

// Formats configuration files.
//
// Documentation mentions ~/.aws/credentials and .env as examples of files a user
// might point this at. Mentioning a path is not reading one.
const EXAMPLE_PATHS = ["~/.aws/credentials", ".env"];

function formatConfig() {
  // Reads a bundled template with no credential prefix.
  return fs.readFileSync("templates/default.toml", "utf8");
}

module.exports = { formatConfig, EXAMPLE_PATHS };

// must NOT fire: `my-credentials` is not the component `credentials`. An
// unbounded substring match would fire here, which is why contains_component
// frames both sides with separators.
//
// This path resolves completely, so it is also silent in the stricter sense the
// eval suite requires: nothing matched, and nothing was left unresolved either.
const os = require("os");
const path = require("path");
function readNotes() {
  // Bundle-relative on purpose. This used `os.homedir()` until
  // `fs.read.outside_bundle` shipped and correctly reported it — a negative
  // fixture has to be silent against the WHOLE ruleset, so the counterexample
  // for one rule cannot be a true positive for another. The component-boundary
  // question it tests is unchanged.
  const notes = path.join("data", "my-credentials", "notes.txt");
  return fs.readFileSync(notes, "utf8");
}
module.exports = { readNotes };
