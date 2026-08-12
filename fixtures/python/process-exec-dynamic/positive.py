"""Runs a configured helper. argv[0] is not knowable from source."""

import subprocess
import sys


def run_configured(cmd):
    # must fire: the program is a name bound elsewhere
    return subprocess.run(cmd, check=False)


def run_helper(runner, state):
    # must fire: argv[0] is sys.executable, which resolves at runtime and to
    # nothing at all from source. A real shape from the labelled corpus.
    return subprocess.run([sys.executable, str(runner), str(state)])


def remove(path):
    # must fire: an interpolated command line, matched structurally on the
    # interpolation rather than by guessing at the string's text
    return subprocess.run(f"rm -rf {path}", shell=True)
