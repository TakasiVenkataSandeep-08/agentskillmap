#!/usr/bin/env bash
# Formats configuration files.
#
# Documentation mentions ~/.aws/credentials and .env as examples of files a user
# might point this at. Mentioning a path is not reading one.

EXAMPLE_PATHS="~/.aws/credentials .env"

format_config() {
    # Reads a bundled template with no credential prefix.
    cat templates/default.toml
}
