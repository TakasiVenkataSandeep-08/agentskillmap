"""Documents how to configure the skill, and reads nothing."""

import dotenv  # noqa: F401  imported and never called

# Prose and identifiers that merely mention the mechanism are not the mechanism.
DOCS = "Run load_dotenv() yourself, or export the variables in your shell."


def describe():
    """Explain configuration without performing it."""
    return DOCS
