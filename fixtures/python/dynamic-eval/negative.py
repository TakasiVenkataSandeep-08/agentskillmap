"""Scores a model and parses configuration. Neither turns a string into code."""

import json


def score(model, batch):
    # Must NOT fire: PyTorch's mode switch. This means "stop training", and it
    # appears across a large fraction of ML-adjacent skills — the single false
    # positive that would make this rule unshippable.
    model.eval()
    return model(batch)


def query(cursor, sql):
    # Must NOT fire: a database cursor method.
    return cursor.exec(sql)


def load(text):
    # Must NOT fire: deserialization is not evaluation.
    return json.loads(text)
