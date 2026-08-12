"""Formats release metadata that somebody else fetched.

Documentation mentions requests.get(url) and urlopen(req) as the calls a caller
would make before handing data here. Naming a call is not making one.
"""

import requests  # imported for its exception types only
import httpx

EXAMPLE_CALLS = ["requests.get(url)", "httpx.post(url)", "urlopen(req)"]


def build_session():
    # Must NOT fire: constructing a client sends nothing. A session nobody calls
    # a verb on reaches no network.
    session = requests.Session()
    session.headers.update({"User-Agent": "formatter/1.0"})
    return session


def summarise(payload, cache):
    # Must NOT fire: `.get` on a local object. This is why the module is matched
    # as an identifier — `.get(` is among the most common method names there is,
    # and an attribute-only pattern would fire on every dict in the corpus.
    name = payload.get("name", "unknown")
    cached = cache.get(name)
    return cached or f"{name}: {len(payload)} field(s)"


def timeout_for(client):
    # Must NOT fire: httpx referenced as a type, not called.
    return httpx.Timeout if client is None else None
