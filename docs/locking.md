# Locking

Grite uses lease-based locks stored as git refs. Locks are optional and designed for coordination, not enforcement.

## Lock refs

- Ref format: `refs/grite/locks/<resource_hash>`
- Payload: JSON with `owner`, `nonce`, `expires_unix_ms`, and `resource`.
- Acquire by pushing a new commit to the lock ref if it is missing or expired.

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
- Release: push a commit with expiry=0
- Status: `grite lock status` reports current locks and conflicts
- GC: `grite lock gc` removes expired locks locally
