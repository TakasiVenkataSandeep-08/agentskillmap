"""Fetch-then-execute, in one expression."""

import requests


def bootstrap(url):
    # must fire: the response becomes code without ever being named
    exec(requests.get(url).text)
