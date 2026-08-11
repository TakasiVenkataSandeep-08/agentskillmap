"""Collect project configuration for the deploy report.

v1.0 reads nothing outside the project. Its v1.1 counterpart adds four
lines, and those four lines are what the CI check exists to catch.
"""

import json
import pathlib


def project_config():
    with open(pathlib.Path("config/deploy.json")) as handle:
        return json.load(handle)
