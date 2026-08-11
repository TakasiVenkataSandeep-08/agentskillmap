# `skillmap.lock` and `policy.toml`

Two files a scanned project owns and commits. Neither belongs to this repository — they
describe the project being audited.

They answer different questions, and the whole design turns on keeping them apart:

| File | Question | Failure it produces |
|---|---|---|
| `skillmap.lock` | *Is this capability **new**?* | exit `1`, escalation |
| `policy.toml` | *Is this capability **allowed here at all**?* | exit `2`, policy violation |

A skill can hold a permitted capability it did not have yesterday — worth a human look
even though nothing forbids it. A skill can hold a forbidden capability it has held since
before the lock was written — which the diff would never mention, because nothing changed.
Collapsing the two into one failure makes both unreadable.

---

## `skillmap.lock`

Canonical JSON, written by `skillmap lock`, read by `skillmap ci`. Committed.

```json
{
  "bundles": {
    "example-skill": {
      "capabilities": [
        "fs.read.credential"
      ],
      "content_digest": "sha256:5b337214…",
      "resolver": "claude-code",
      "schema_version": "1.0.0"
    }
  }
}
```

### Framing

Identical to the manifest's (`docs/02-manifest-schema.md`): sorted keys, two-space indent,
LF, trailing newline, no floats. A lockfile is diffed on every change, so its serialization
has to be at least as stable as the artifact it summarizes. Any instability here shows up as
review noise in someone else's pull request.

### Fields

| Key | Type | Meaning |
|---|---|---|
| `bundles` | object | Keyed by `target.root` — the bundle's path relative to its resolver's discovery root. Not an array: the key is the identity, and an array would invite two entries for one bundle. |
| `bundles[].resolver` | string | The resolver id that discovered it, e.g. `claude-code`. Recorded because the same `root` under a different resolver is a different bundle. |
| `bundles[].content_digest` | string | `sha256:…`, the merkle root over the sorted inventory. Changes whenever any byte does. |
| `bundles[].capabilities` | array of string | Capability terms, sorted and deduplicated. |
| `bundles[].schema_version` | string | The manifest schema version this entry was written against. |

### Why capabilities are strings, not the enum

A lock outlives the binary that wrote it. If an older `skillmap` dropped terms it did not
recognise, running it once would silently rewrite the lock, and the next run of a newer
build would report the losses as fresh escalations — a downgrade that manufactures alarms.
Unknown terms therefore round-trip untouched. `an_unrecognised_capability_term_survives_a_round_trip`
in `crates/skillmap-diff/src/lib.rs` is the test.

### Why it is not the manifest

A manifest carries every byte span, every snippet hash, every unresolved entry — hundreds of
lines per bundle. A lock diff is meant to be read in a pull request by someone who is
reviewing something else. So the lock holds the capability set and the digest, and the
manifest is regenerated on demand when somebody wants the evidence.

### What the diff reports

| Change | Escalation? |
|---|---|
| `CapabilityAdded` | **yes** — the case the tool exists for |
| `BundleAdded` with at least one capability | **yes** |
| `BundleAdded` with none | no — installing a skill that can do nothing is not a privilege change, and failing CI for it teaches people the check cries wolf |
| `CapabilityRemoved` | no — reported, because "this update dropped the credential read" is worth seeing |
| `BundleRemoved` | no |
| `ContentChanged` | no — reported, because "changed code and happened not to trip a rule" is different from "did nothing" |

Within a bundle, escalations print first. The budget in `docs/00-tasks.md` is ten seconds,
and a digest change printed above the credential read spends it on nothing.

### A missing lock is an error, not an empty baseline

`skillmap ci` exits `4` when the lock is absent, telling the user to run `skillmap lock`.
Treating absence as an empty lock would report every existing bundle as newly added and fail
the very first run in every repository — the fastest known way to get a check disabled.

---

## `policy.toml`

TOML, written by hand, read by `skillmap ci`. Committed. This is where judgement lives:
invariant 1 keeps it out of the manifest, because whether `fs.read.credential` is acceptable
is a question only a specific repository can answer, and the answer differs between a
credential manager and a markdown formatter.

```toml
# Capabilities accepted from any bundle in this repository.
[allow]
capabilities = ["process.exec", "net.egress"]

# Additions for one bundle, keyed by its `target.root`.
[bundle."aws-deploy"]
capabilities = ["fs.read.credential"]
```

### Fields

| Key | Type | Meaning |
|---|---|---|
| `allow.capabilities` | array of string | Terms accepted from every bundle. |
| `bundle.<root>.capabilities` | array of string | Terms additionally accepted from that one bundle. |

Unknown keys are rejected. A typo'd `capabilties = [...]` that parsed and did nothing would
fail CI later with no visible cause.

### Bundle entries are additive only

A `[bundle.…]` section widens what that bundle may do; it can never narrow the repository
default. Narrowing would let a policy grant something globally and quietly retract it in one
place, which is how an allowlist stops being readable — and an allowlist nobody can read at a
glance is not doing the job an allowlist exists for.

### Unknown capability terms are rejected

A term outside the taxonomy in `docs/02-manifest-schema.md` fails the load with exit `4`.
A misspelled term permits nothing, so the capability it was meant to allow fails CI and the
reason is invisible. Failing on the file is far kinder than failing on the scan.

### Absent is not the same as empty

| State | Meaning |
|---|---|
| No `policy.toml` | No opinion. The policy check does not run, and `skillmap ci` says so on stderr. The escalation check still does. |
| `policy.toml` with no capabilities | A real, restrictive opinion: nothing is permitted. |

This is invariant 3 applied to configuration, and it costs a repository dearly in the same
direction either way it is collapsed. Treat absent as permissive and the check silently
approves everything. Treat absent as restrictive and the first run in every repository that
has not written a policy fails on every capability it already had.

---

## Exit codes

```
0  clean
1  a bundle gained capability it did not have in the lock
2  a capability is present that policy.toml does not permit
3  both
4  the check could not run — bad arguments, no rules loaded, missing lock, bad policy
```

`4` is separate on purpose, and it is the same argument as invariant 3 one level up:
*"could not run"* must never be readable as *"ran and found nothing"*. A scanner that loads
zero rules and reports a clean project is the single worst thing this tool could do, so
`skillmap` refuses to scan with an empty ruleset rather than producing a confident silence.

`1` and `2` are separate so a consumer can branch. A repository mid-migration may want to
fail on escalation and only warn on policy, and it cannot do that if both exit `1`.

---

## Using it

```bash
skillmap lock            # record what the skills in this project can do today
git add skillmap.lock
```

Then in CI:

```bash
skillmap ci
```

The GitHub Action at `action.yml` wraps exactly that. Accepting a change is
`skillmap lock` again — a reviewable diff, which is the point.
