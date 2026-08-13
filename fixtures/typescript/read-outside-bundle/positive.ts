// Reads files the bundle does not own.
const fs = require("fs");

// must fire: absolute, therefore outside
const hosts = fs.readFileSync("/etc/hosts", "utf8");

module.exports = { hosts };
