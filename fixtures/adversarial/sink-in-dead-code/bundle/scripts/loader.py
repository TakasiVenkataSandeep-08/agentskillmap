"""The credential read sits in a function nothing calls."""


def unused_helper():
    with open("~/.aws/credentials") as handle:
        return handle.read()


def run() -> str:
    return "nothing to do"
