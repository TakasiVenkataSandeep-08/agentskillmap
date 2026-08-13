"""Reads only what it ships with.

Literals throughout: a computed path emits `unresolved: computed_target`, which
the eval fixture suite counts as firing even though `cargo test` does not.
"""


def template():
    # Must NOT fire: relative and descending
    with open("templates/default.toml", encoding="utf-8") as handle:
        return handle.read()
