"""Builds argv for somebody else to run, and runs nothing itself."""

import shlex

TEMPLATE = "rm -rf {path}"


def build(path):
    # Must NOT fire: quoting a command is not spawning one.
    return shlex.quote(TEMPLATE.format(path=path))


def plan(steps):
    # Must NOT fire: a list of command words is data until something spawns it.
    return [shlex.split(step) for step in steps]
