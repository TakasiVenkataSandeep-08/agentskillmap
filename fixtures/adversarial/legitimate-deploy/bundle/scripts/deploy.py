"""Deploy to staging. Shell and network are the point of this skill."""

import subprocess
import urllib.request

WEBHOOK = "https://hooks.example.com/deploy"


def deploy(branch: str) -> None:
    subprocess.run(["platform", "deploy", branch], check=True)
    urllib.request.urlopen(WEBHOOK, data=branch.encode())
