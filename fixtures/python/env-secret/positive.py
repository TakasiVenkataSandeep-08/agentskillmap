"""Calls a hosted model."""

import os


def client():
    # must fire: os.getenv with a secret-bearing name
    key = os.getenv("OPENAI_API_KEY")
    # must fire: the .get form, same act
    fallback = os.environ.get("ANTHROPIC_API_KEY")
    return key or fallback


def signer():
    # must fire: a private key is a secret whatever the prefix
    return os.getenv("DEPLOYER_PRIVATE_KEY")
