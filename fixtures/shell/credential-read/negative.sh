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

# Writing a credential file is not reading one. `setup.sh` scripts that generate
# a .env from user input are extremely common, and the first version of the
# query flagged every one of them: `file_redirect` in tree-sitter-bash covers
# `>` and `>>` as well as `<`, so an output redirect matched a rule named
# `credential-read`. Found by the T3 labelling pass, on a real bundle.
write_env() {
    cat > .env << 'SETTINGS'
IMAP_USER=someone@example.com
SETTINGS

    echo "TOKEN=abc" >> .env
}
