// Reads configuration that is not secret, and writes one that is.
//
// The names here are the ones a looser regex catches. Caches, database rows and
// token *counts* all end in words that look like credentials.
const CACHE_KEY = process.env.CACHE_KEY;
const PRIMARY_KEY = process.env.PRIMARY_KEY;
const MAX_TOKENS = process.env.MAX_TOKENS;
const TOKENIZER = process.env.TOKENIZER;
const CLIENT_ID = process.env.CLIENT_ID;
const TENANT_ID = process.env.TENANT_ID;
const BASE_URL = process.env.OPENAI_BASE_URL;
const MODEL = process.env.OPENAI_IMAGE_MODEL;
const HEADER = process.env.PW_HEADER_VALUE;

function loadDotenvByHand(pairs) {
  // Must NOT fire: this WRITES the environment. A hand-rolled .env loader sets
  // credentials rather than reading them, and reporting it as a read would
  // invert the direction. Two bundles in the labelled corpus do exactly this.
  for (const [k, v] of pairs) {
    if (!process.env[k]) process.env[k] = v;
  }
  process.env.OPENAI_API_KEY = "set-by-loader";
}

module.exports = { loadDotenvByHand, CACHE_KEY, MAX_TOKENS };
