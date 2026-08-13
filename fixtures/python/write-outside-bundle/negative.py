"""Writes only inside its own directory.

Every path here is relative and descending, which is what being inside a bundle
means. Literals throughout: a computed path would emit
`unresolved: computed_target`, which the eval suite counts as firing.
"""

from pathlib import Path


def save(payload):
    # Must NOT fire: relative, inside the bundle
    with open("out/state.json", "w", encoding="utf-8") as handle:
        handle.write(payload)


def local_dir():
    # Must NOT fire: same, for the directory-creation shape
    out = Path("out") / "cache"
    out.mkdir(parents=True, exist_ok=True)
    return out
