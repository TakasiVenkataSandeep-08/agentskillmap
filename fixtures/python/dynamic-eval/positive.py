"""Evaluates a user-supplied expression."""


def calculate(expression):
    # must fire: a string becomes code
    return eval(expression)


def run(payload):
    # must fire: the other builtin
    exec(payload)


def precompile(source):
    # must fire: compile is the same act, deferred
    return compile(source, "<skill>", "exec")
