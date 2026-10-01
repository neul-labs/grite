# Locking

Grite uses lease-based locks stored as git refs. Locks are optional and designed for coordination, not enforcement.

## Lock refs

- Ref format: `refs/grite/locks/<resource_hash>`
- Payload: JSON with `owner`, `nonce`, `expires_unix_ms`, and `resource`.
- Acquire by pushing a new commit to the lock ref if it is missing or expired.
  The new commit is a child of the expired or released lock it replaces, so
  the ref only ever fast-forwards.

## Lock policy

Lock policy is configured in `.git/grite/config.toml`:

- `off`: no lock checks
- `warn` (default): warn on conflicts, but continue
- `require`: block write commands if a conflicting lock exists

When `require` is enabled, the CLI must check locks before write operations such as:

- `issue create/update/comment/close`
- `snapshot`
- `sync --push`

An optional `pre-push` hook can enforce the same policy for users who prefer git-level gating.

## Namespaces and why they matter

A lock namespace is a prefix embedded in the resource string (for example `repo:`, `path:`, `issue:`). It defines scope and conflict policy.

**Repo-wide lock (`repo:`)**
- One lock for the entire repository.
- Used for global operations like schema migrations, large refactors, or release tasks.
- When present, it should block acquisition of any other lock type.

**Path lock (`path:`)**
- Fine-grained lock for a specific file or directory.
- Allows multiple agents to work concurrently on different areas.
- Only blocks overlapping path locks; does not block unrelated paths.

**Why keep both**
- Repo-wide locks provide a simple “stop the world” switch for risky operations.
- Path locks allow safe parallelism without coordinating the entire team.
- The namespace tells clients how to apply conflict rules (global vs scoped).

## Example resources

- `repo:global`
- `path:src/parser.rs`
- `path:docs/`
- `issue:abcd1234`

## Lock lifecycle

- Acquire: create a new lock commit with a lease TTL
- Renew: push a new commit extending expiry (owner must match)
- Status: `grite lock status` reports current locks and conflicts
- Release: write a commit with expiry=0 (a tombstone) on top of the lock; the
  next push deletes the remote ref, then the local one
- GC: `grite lock gc` removes expired locks locally; released tombstones are
  kept until a push has propagated them

## Locks and sync

The remote's lock refs are what clones agree on, and every change `sync` makes
to one is a compare-and-swap on the tip it inspected. Locks are pushed
separately from the WAL, so a lock conflict never blocks the WAL.

- `sync --pull` fast-forwards lock refs, replaces a stale local lock with the
  remote's live lock, and drops expired locks and locks the remote no longer
  has (unless they belong to one of this repository's actors and are simply
  not pushed yet).
- `sync --push` pushes this repository's live locks, deletes expired or
  released locks from the remote, and re-parents a local lock onto the remote
  ref when their histories are unrelated and the remote lock is expired or
  has the same owner.
- A remote lock held live by another actor is never replaced or deleted; the
  local lock is reported in `lock_conflicts` and replaced by the remote one.
- Lock refs written by older versions (unrelated root commits) heal through a
  normal sync.
