// Requiring the module without calling config() reads nothing. Documentation
// that names the mechanism is not the mechanism.
const dotenv = require('dotenv');

const HELP = "Call dotenv.config() yourself, or export the variables in your shell.";

module.exports = { dotenv, HELP };
