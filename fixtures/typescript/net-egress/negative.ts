// Formats release metadata that somebody else fetched.
//
// Documentation mentions fetch(url) and axios.post(url, body) as the calls a
// caller makes before handing data here. Naming a call is not making one.
const axios = require("axios"); // imported for its error types only

const EXAMPLE_CALLS = ["fetch(url)", "axios.post(url, body)"];

function summarise(cache, repo, imap) {
  // Must NOT fire: `.fetch` on objects that reach no network. All three are real
  // method names, and the third cost a label during the corpus pass — a mail
  // skill whose only apparent network site was an IMAP method named `fetch`.
  const cached = cache.fetch("latest");
  const commits = repo.fetch({ depth: 1 });
  const messages = imap.fetch([1, 2], { bodies: "" });
  return { cached, commits, messages };
}

function errorType() {
  // Must NOT fire: axios referenced as a value, not called.
  return axios.AxiosError;
}

module.exports = { summarise, errorType, EXAMPLE_CALLS };
