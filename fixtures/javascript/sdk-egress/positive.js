// Calls a hosted model and an EVM node. Neither call names a protocol.
const OpenAI = require("openai");
const { createPublicClient, http } = require("viem");

async function summarise(text) {
  const openai = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
  // must fire: reaches a hosted API with no http token in the call
  return openai.chat.completions.create({ model: "gpt-4", messages: [] });
}

function node() {
  // must fire: constructing a client that names a transport and a chain
  return createPublicClient({ transport: http("https://example.invalid") });
}

async function balance(client) {
  // must fire: the call that reaches the chain
  return client.readContract({ address: "0x0", functionName: "balanceOf" });
}

module.exports = { summarise, node, balance };
