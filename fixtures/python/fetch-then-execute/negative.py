"""Parses local data. Fetches nothing, evaluates nothing.

Three things cannot live in this file, all for one reason: a negative fixture
runs against the whole ruleset for its language, and the eval suite counts an
`unresolved` entry as firing just as it counts a capability.

  - A fetch that is NOT executed is still a real `net.egress` finding. That
    contrast is pinned per term by
    `a_fetch_without_a_sink_is_egress_and_not_fetch_then_execute` instead.
  - `open(dest, "w")` with a computed `dest` emits `unresolved: computed_target`,
    because the write rules declare a path filter and folding cannot resolve it.
  - So does `open(path)` for READING with a computed path, for the same reason.

Every path here is therefore a literal, which is the only way to be silent in
the strict sense the eval requires.
"""

import json

CONFIG = "config/defaults.json"


def config():
    # Must NOT fire: a literal, relative path, and parsing is not evaluating.
    with open(CONFIG, encoding="utf-8") as handle:
        return json.loads(handle.read())
