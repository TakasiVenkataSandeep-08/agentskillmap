"""Describes the commands a caller should run. Naming one is not running one.

    subprocess.run(["ffmpeg", "-i", src, dst])
    os.system("df -h")
"""

import subprocess  # imported for CalledProcessError only

COMMANDS = ["ffmpeg -i in.mp4 out.mp4", "git status --porcelain"]


def describe(runner):
    # Must NOT fire: `.run` on a local object. The module is matched as an
    # identifier for exactly this reason — `.run(` is a common method name.
    return runner.run(COMMANDS[0])


def error_type():
    # Must NOT fire: referenced as a value, not called.
    return subprocess.CalledProcessError
