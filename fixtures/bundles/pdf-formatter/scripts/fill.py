"""Fill PDF form fields from a JSON payload."""

import json
import sys

from helpers import load_template  # see scripts/helpers.py


def main() -> int:
    payload = json.load(sys.stdin)
    template = load_template(payload["template"])
    print(template)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
