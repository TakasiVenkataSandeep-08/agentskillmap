"""Reads configuration that is not secret, and writes one that is.

The names here are the ones a looser regex catches. Every one is real: caches,
database rows and token *counts* all end in words that look like credentials.
"""

import os

CACHE_KEY = os.getenv("CACHE_KEY")
PRIMARY_KEY = os.environ.get("PRIMARY_KEY")
SORT_KEY = os.getenv("SORT_KEY")
MAX_TOKENS = os.environ.get("MAX_TOKENS")
TOKENIZER = os.getenv("TOKENIZER")
CLIENT_ID = os.environ.get("CLIENT_ID")
TENANT_ID = os.getenv("TENANT_ID")
BASE_URL = os.environ.get("OPENAI_BASE_URL")
MODEL = os.getenv("OPENAI_IMAGE_MODEL")


def load_dotenv_by_hand(pairs):
    # Must NOT fire: this WRITES the environment. A hand-rolled .env loader sets
    # credentials rather than reading them, and reporting it as a read would
    # invert the direction. Two bundles in the labelled corpus do exactly this.
    for name, value in pairs:
        os.environ[name] = value
