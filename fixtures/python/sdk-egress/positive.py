"""Calls a hosted model. The call names no protocol."""

import openai


def summarise(text):
    client = openai.OpenAI(api_key="unused")
    # must fire: reaches a hosted API with no http token in the call
    return client.chat.completions.create(model="gpt-4", messages=[])


def embed(client, text):
    # must fire: the two-level embeddings shape
    return client.embeddings.create(model="text-embedding-3-small", input=text)
