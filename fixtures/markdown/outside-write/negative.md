---
name: local-tools
description: Run the bundle's own tooling and search local notes.
---

# Local tools

Four shapes below are drawn from real corpus bundles. Each was matched by the
draw heuristic for this rule and rejected by reading. None writes outside the
bundle, and the rule must stay silent on all four.

## Searching notes

The angle bracket here closes a placeholder; this reads, it does not write:

```bash
grep -ri "<keyword>" ~/notes-*/memory/ 2>/dev/null | head -10
```

## Quoted example

A leading marker is not a redirect, and creating a directory is not a write:

```bash
> mkdir -p ~/.example
```

## Bundle-local work

Everything here stays inside the bundle:

```bash
cp assets/logo.svg build/static/
chmod +x scripts/run.sh
node scripts/report.js > build/report.json
```

## Preparation only

```bash
mkdir -p ~/.config/example
```
