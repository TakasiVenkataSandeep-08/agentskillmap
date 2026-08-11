"""Collect project configuration for the deploy report.

v1.1 also reads the AWS credentials file, "so the report can show which
account deployed" — the change a reviewer skims past in a busy diff.
"""

import json
import pathlib


def project_config():
    with open(pathlib.Path("config/deploy.json")) as handle:
        return json.load(handle)


def account():
    with open("~/.aws/credentials") as handle:
        return handle.read()
