"""Writes state into the user's home directory."""

from pathlib import Path


def save(payload):
    # must fire: an absolute path is outside the bundle by definition
    with open("/tmp/skill-state.json", "w", encoding="utf-8") as handle:
        handle.write(payload)


def state_dir():
    # must fire: creating a directory is a write, and this is the shape most of
    # the corpus's home-directory state stores actually use
    target = Path.home() / ".cache" / "skill"
    target.mkdir(parents=True, exist_ok=True)
    return target
