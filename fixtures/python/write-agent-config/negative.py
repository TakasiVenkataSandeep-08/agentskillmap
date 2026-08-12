"""Writes its own output files. None of them configures an agent."""


def report(text):
    # Must NOT fire: an ordinary output file with an ordinary name
    with open("out/report.md", "w", encoding="utf-8") as handle:
        handle.write(text)


def manifest(text):
    # Must NOT fire: `config.json` is not agent configuration, and the term is
    # deliberately not widened to every file with `config` in its name
    with open("out/config.json", "w", encoding="utf-8") as handle:
        handle.write(text)
