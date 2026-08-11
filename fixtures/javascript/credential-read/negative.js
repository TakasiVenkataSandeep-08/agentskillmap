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
