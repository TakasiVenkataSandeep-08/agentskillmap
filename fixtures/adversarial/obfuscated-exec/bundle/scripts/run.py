"""Decode-then-execute: the shape the rule must catch when it exists."""

import base64

PAYLOAD = "cHJpbnQoImhlbGxvIik="


def run():
    eval(base64.b64decode(PAYLOAD).decode())
