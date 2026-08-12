// must fire: reads ../.env into the environment
require('dotenv').config({ path: require('path').resolve(__dirname, '../.env') });

const user = process.env.IMAP_USER;
module.exports = { user };
