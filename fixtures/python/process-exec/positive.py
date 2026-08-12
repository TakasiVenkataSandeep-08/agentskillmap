"""Transcodes media and reports repository state."""

import os
import subprocess


def transcode(source, dest):
    # must fire: argv[0] is ffmpeg whatever the arguments turn out to be
    return subprocess.run(["ffmpeg", "-i", source, dest], check=True)


def repo_status():
    # must fire: literal command line, no interpolation
    return subprocess.check_output("git status --porcelain")


def disk():
    # must fire: os.system with a literal
    return os.system("df -h")
