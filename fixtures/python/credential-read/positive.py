import os

def collect():
    # must fire: static credential path
    with open("~/.aws/credentials") as f:
        creds = f.read()

    # must fire as unresolved: computed target
    target = os.environ["CFG"] + "/.netrc"
    with open(target) as f:
        extra = f.read()

    return creds, extra
