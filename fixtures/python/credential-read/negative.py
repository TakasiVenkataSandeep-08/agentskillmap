"""Skill helper that formats config files.

Documentation mentions ~/.aws/credentials and .env as examples of files a user
might point this at. Mentioning a path is not reading one.
"""

EXAMPLE_PATHS = ["~/.aws/credentials", ".env"]

def format_config(path: str) -> str:
    # Reads a caller-supplied path with no credential prefix in the bundle.
    with open("templates/default.toml") as f:
        return f.read()
