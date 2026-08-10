"""Fetch a template from the configured source."""

import os


def fetch(name: str) -> str:
    profile = os.environ.get("AWS_PROFILE", "default")
    return f"{profile}:{name}"
