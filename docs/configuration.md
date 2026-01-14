# Configuration

This document defines the on-disk configuration files used by Grit. These
files live under `.git/` and are local to a clone; they are not canonical state.

## Repo config

Path: `.git/grit/config.toml`

Purpose: repo-scoped defaults (for example, the default actor and lock policy).

Example:

```toml
default_actor = "00112233445566778899aabbccddeeff"
lock_policy = "warn"

[snapshot]
max_events = 10000
max_age_days = 7
```

### Fields

- `default_actor` (optional): 16-byte hex actor ID used when no `--actor` or
  `GRIT_HOME/--data-dir` is provided.
- `lock_policy` (optional, default `warn`): one of `off`, `warn`, or `require`.
- `[snapshot]` (optional): local snapshot policy overrides.
  - `max_events` (optional, default 10000): create a snapshot when events since
    the last snapshot exceed this value.
  - `max_age_days` (optional, default 7): create a snapshot when the last
    snapshot is older than this many days.

## Actor config

Path: `.git/grit/actors/<actor_id>/config.toml`

Purpose: actor identity and optional metadata.

Example:

```toml
actor_id = "00112233445566778899aabbccddeeff"
label = "work-laptop"
created_ts = 1700000000000
public_key = "aabbcc...ff"
key_scheme = "ed25519"
```

### Fields

- `actor_id` (required): 16-byte hex actor ID (matches the directory name).
- `label` (optional): human-friendly name for the actor.
- `created_ts` (optional): unix timestamp (ms) when the actor was created.
- `public_key` (optional): hex-encoded public key for event signature verification.
- `key_scheme` (optional, default `ed25519`): signature algorithm for `public_key`.

## Notes

- The repo config may be absent; clients fall back to auto-init behavior.
- Config files are owned by the local clone; do not copy them between repos.
- Snapshot policy is advisory and does not affect WAL correctness.
