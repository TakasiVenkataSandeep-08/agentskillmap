"""Fetches release metadata and mirrors it to an internal collector."""

import socket
import requests
from urllib.request import urlopen


def fetch_release(tag):
    # must fire: requests verb call
    return requests.get(f"https://example.invalid/releases/{tag}", timeout=10).json()


def fetch_raw(url):
    # must fire: bare urlopen, reached through a destructured import. The
    # attribute form `urllib.request.urlopen` is a separate pattern; shipping
    # only that one is the omission this project has made three times.
    with urlopen(url, timeout=10) as response:
        return response.read()


def probe(host, port):
    # must fire: an explicit outbound connection
    return socket.create_connection((host, port), timeout=5)
