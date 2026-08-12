// Mirrors release metadata to an internal collector.
const axios = require("axios");

async function fetchRelease(tag) {
  // must fire: the global, as a bare identifier
  const res = await fetch(`https://example.invalid/releases/${tag}`);
  return res.json();
}

async function publish(body) {
  // must fire: a verb on a binding actually named axios
  return axios.post("https://example.invalid/api/builds", body);
}

function watch(url) {
  // must fire: a persistent connection is egress like any other
  return new WebSocket(url);
}

module.exports = { fetchRelease, publish, watch };
