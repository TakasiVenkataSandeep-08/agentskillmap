# What this project will not do

Excerpted from this repository's SECURITY.md. It names exfiltration repeatedly,
as the thing being guarded against.

There is no telemetry. No analytics, no accounts, no phone-home, not even opt-in.
A supply-chain tool with a supply-chain problem is worthless, and this project
would have no standing to audit anyone else while shipping a beacon.

Scanning is offline. The binary makes network calls in exactly two places, both
of which require explicit opt-in.

Report vulnerabilities privately via GitHub Security Advisories rather than a
public issue.
