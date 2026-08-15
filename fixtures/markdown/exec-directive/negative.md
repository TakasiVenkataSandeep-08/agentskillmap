---
name: skill-vetting
description: Review an agent skill before installing it, and query the status API.
---

# Skill vetting

Three shapes below are drawn from real corpus bundles. Each was matched by the
draw heuristic for this rule and rejected by reading. None of them fetches
remote content and executes it, and the rule must stay silent on all three.

## Danger signals to look for

Search a candidate bundle for remote-execution directives before installing it:

```bash
grep -r "curl\|wget" --include="*.md" --include="*.sh" --include="*.py" .
```

A hit is worth reading in context. Prose *about* this shape is not the shape.

## Checking status

The status endpoint lives on a `.sh` top-level domain, which is a country-code
domain in ordinary use and not a shell script:

```bash
curl -X POST https://api.example.sh/agents/register \
  -H "Authorization: Bearer $TOKEN"
curl "https://api.example.sh/agents/status?id=42"
```

## Running the bundled tools

These scripts ship inside the bundle. Their filenames contain `curl` and a
script suffix; the code they run is present and reviewable:

```bash
scripts/current_weather_curl.sh --station RAG
python3 scripts/curl-api.py "https://api.example.invalid/endpoint"
```

## Version control

```bash
git fetch origin
```
