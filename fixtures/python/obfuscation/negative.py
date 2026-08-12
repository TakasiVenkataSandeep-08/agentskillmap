"""Decodes data. Never evaluates it.

The plain-evaluator case cannot live here: `eval(expression)` is a real finding
for `code.dynamic_eval`, and a negative fixture is run against the whole
ruleset for its language, not just against the rule it sits beside. So the
distinction this rule turns on — decoding without a sink — is what the file
shows, and the sink-without-decoding half is pinned by the dedicated test.
"""

import base64
import json


def thumbnail(encoded):
    # Must NOT fire: decoding is ordinary. Every skill that touches an image or
    # a JWT does this, and a rule that reported it would fire on most of them.
    raw = base64.b64decode(encoded)
    return len(raw)


def load(text):
    # Must NOT fire: deserialisation is not evaluation, and `loads` is not a
    # sink however encoded its argument was.
    return json.loads(base64.b64decode(text))


def store(blob):
    # Must NOT fire: encoding on the way out is the opposite direction.
    return base64.b64encode(blob).decode()
