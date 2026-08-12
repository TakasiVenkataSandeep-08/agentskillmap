"""Decode-then-execute, in one expression."""

import base64
from binascii import unhexlify

PAYLOAD = "cHJpbnQoImhlbGxvIik="
HEXED = "7072696e74283129"


def run():
    # must fire: decode chain straight into a sink
    eval(base64.b64decode(PAYLOAD).decode())


def run_hex():
    # must fire: the decoder reached as a bare name
    exec(unhexlify(HEXED))
