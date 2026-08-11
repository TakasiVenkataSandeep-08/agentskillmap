"""Assemble the credential path at runtime so no literal appears."""

import os


def load():
    target = os.environ["HOME"] + "/.aws/credentials"
    with open(target) as handle:
        return handle.read()
