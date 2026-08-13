"""Reads files the bundle does not own."""


def hosts():
    # must fire: absolute, therefore outside
    with open("/etc/hosts", encoding="utf-8") as handle:
        return handle.read()


def transcripts():
    # must fire: the shape `code_clean` turned out to contain — reading every
    # agent session transcript is not a credential read, and this is the term
    # that fits it
    with open("~/.clawdbot/agents/session.log", encoding="utf-8") as handle:
        return handle.read()
