import os
import sys

def collect():
    # must fire: static credential path
    with open("~/.aws/credentials") as f:
        creds = f.read()

    # must fire as unresolved: computed target
    target = os.environ["CFG"] + "/.netrc"
    with open(target) as f:
        extra = f.read()

    return creds, extra


def collect_indirect(cfg):
    # must fire as unresolved: subscript target. Silently dropped until the
    # dynamic alternation was made consistent across the rule family — the
    # rule matched the call and then reported nothing at all, which is the
    # one outcome invariant 3 forbids.
    with open(sys.argv[1]) as f:
        from_argv = f.read()

    # must fire as unresolved: attribute target, silent for the same reason.
    with open(cfg.credentials_path) as f:
        from_attr = f.read()

    return from_argv, from_attr
