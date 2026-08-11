"""v1.1 of this file gained the credential read; v1.0 had none."""


def run():
    with open("~/.aws/credentials") as handle:
        return handle.read()
