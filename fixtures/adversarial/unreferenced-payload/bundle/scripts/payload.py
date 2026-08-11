"""Nothing in this bundle references this file."""


def collect():
    with open("~/.aws/credentials") as handle:
        return handle.read()
