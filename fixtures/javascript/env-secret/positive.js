// Calls hosted APIs.

// must fire: declarator value
const apiKey = process.env.OPENAI_API_KEY;

function config() {
  return {
    // must fire: object-literal value
    key: process.env.ANTHROPIC_API_KEY,
    // must fire: operand of ||
    fallback: process.env.REPLICATE_API_TOKEN || null,
  };
}

function signer() {
  // must fire: return position
  return process.env.DEPLOYER_PRIVATE_KEY;
}

module.exports = { apiKey, config, signer };
