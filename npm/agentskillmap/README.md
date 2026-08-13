# skillmap

A supply-chain auditor for AI agent skills. It answers **"what does this skill make my agent
capable of doing?"** with byte-level evidence, and diffs that answer across versions.

```
$ npx skillmap ci
✗ example-skill  capability escalation vs skillmap.lock
    + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
      reads ~/.aws/credentials — added in this update
```

## Install

```bash
npm install --save-dev skillmap
```

This package contains no binary. The binary for your platform arrives as an optional
dependency (`@agentskillmap/linux-x64` and friends), which means **there is no `postinstall`
script and nothing is downloaded at install time** — the bytes come through npm with the
same integrity hashes as every other dependency. For a tool whose subject is supply-chain
risk, that is not a detail.

If you install with `--omit=optional` or `--no-optional`, no binary is installed and
`skillmap` will tell you so rather than failing obscurely.

## Use

```bash
skillmap lock    # record what the skills in this project can do today; commit it
skillmap ci      # fail when that changes
skillmap scan    # print the capability manifest as JSON
skillmap rules   # list what this build can detect
```

Exit codes: `0` clean, `1` escalation against `skillmap.lock`, `2` a capability `policy.toml`
does not permit, `3` both, `4` the check could not run.

## Documentation

<https://github.com/agentskillmap/agentskillmap> — the invariants, the manifest schema, the rule format,
and the reasoning behind all of it.

Apache-2.0.
