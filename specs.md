Alright — here’s a concrete improved design that keeps **Beads’ strengths** (repo-local, auditable, diffable, agent-friendly) while eliminating the class of issues you saw (worktrees, tracked-log churn, daemon brittleness, destructive “doctor”, branch-shape confusion).

I’ll call it **Gems** (placeholder).

## Goals

**Keep (Beads strengths)**

* repo-local issues/tasks/memory
* append-only event history, auditable in git
* offline-first, fast local query
* simple CLI usable by agents
* deterministic merge / conflict-free by design

**Fix (Beads pain)**

* no tracked JSONL in working tree → no phantom changes / worktree fights
* no “sync branch health” heuristics → no destructive reset/force push
* daemon optional → correctness does not depend on background process
* explicit, non-destructive repair → “doctor” emits plan; never rewrites published history
* locks are leases with expiry → no deadlocks, supports teams/agents

## Core idea

**Canonical state is an append-only event log stored in a git ref, not a tracked file.**
Local DB is just a materialized view.

### Storage layers

1. **Git-backed WAL (source of truth)**

* stored as `refs/gems/wal` (or per-stream refs like `refs/gems/wal/<actor>`)
* each append produces a new commit (or a packed blob) that contains the new events
* sync is just `git fetch/push refs/gems/*`

2. **Local materialized view (fast query)**

* `sled` DB in `.git/gems/sled/` (never in the working tree)
* rebuilt deterministically from WAL
* `rkyv` for zero-copy value encoding in sled

3. **Optional export**

* a generated snapshot into `.gems/` for humans (markdown/JSON), but **ignored** and never canonical

This single change removes the worktree + “tracked file shows changes” class entirely.

---

# Data model

## Events (append-only)

Everything is an event. Issues are projections.

```rust
// Stored in WAL (git ref), and also in sled (materialized)
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum EventKind {
  IssueCreated { title: String, body: String, labels: Vec<String> },
  IssueUpdated { title: Option<String>, body: Option<String> },
  CommentAdded { body: String },
  LabelAdded { label: String },
  LabelRemoved { label: String },
  StateChanged { state: IssueState }, // Open/Closed/Blocked/etc
  LinkAdded { url: String, note: Option<String> },
  AssigneeAdded { user: String },
  AssigneeRemoved { user: String },
  AttachmentAdded { name: String, sha256: [u8;32], mime: String },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct Event {
  pub event_id: [u8;32],      // hash(content)
  pub issue_id: [u8;16],      // stable ID (random or hash of creation event)
  pub actor: [u8;16],         // device/user identity
  pub ts_unix_ms: u64,
  pub parent: Option<[u8;32]>, // optional causal chain
  pub kind: EventKind,
  pub sig: Option<Vec<u8>>,   // optional signing
}
```

### Why this fixes merges

* Merging becomes **set-union on `event_id`**.
* Order doesn’t matter for most fields; where it does, we use deterministic “last-writer-wins” by `(ts, actor)` or causal parent if present.
* No line-based conflicts; git merges only refs/commits.

---

# Git WAL format (non-destructive, worktree-safe)

## WAL commit layout

Each WAL update creates a commit on a dedicated ref:

* ref: `refs/gems/wal`
* tree contains:

  * `events/<yyyy>/<mm>/<dd>/<chunk>.bin` (rkyv-serialized Vec<Event>)
  * `meta.json` (small: schema version, actor id, chunk hash)

No branch divergence semantics. It’s a ref you push/pull.

### Append algorithm

1. Load last WAL head commit (if any)
2. Create a new commit with parent = head, adding a new chunk file
3. Update `refs/gems/wal` to new commit
4. Push that ref (optional)

If push fails (remote advanced):

* fetch remote ref
* rebase by creating a new commit whose parent is the fetched head, containing your chunk
* push again (fast-forward)

This is monotonic: **never rewrites history**.

---

# Local DB (sled + rkyv)

## sled keys

* `event/<event_id>` → archived `Event`
* `issue_state/<issue_id>` → archived `IssueProjection` (computed)
* `issue_events/<issue_id>/<ts>/<event_id>` → empty (index)
* `label_index/<label>/<issue_id>` → empty
* `fulltext/<token>/<issue_id>` → (optional, simple)

`IssueProjection` can be a compact struct:

```rust
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct IssueProjection {
  pub issue_id: [u8;16],
  pub title: String,
  pub state: IssueState,
  pub labels: Vec<String>,
  pub assignees: Vec<String>,
  pub updated_ts: u64,
  pub comment_count: u32,
}
```

### Rebuild policy

* On startup: check last applied WAL commit hash
* If differs: stream new chunks, apply events, update projections
* This is deterministic and idempotent

---

# IPC + daemon (async-nng)

## Principle

**No daemon required for correctness.**
Daemon only gives:

* warm cache
* background fetch/push
* file watch integration
* team locking service (optional)

### Components

* `gems` CLI (always works)
* `gemsd` daemon (optional)
* `libgems` Rust crate (shared core)

### IPC

Use `async-nng`:

* `REQ/REP` for commands (`create`, `list`, `sync`, `doctor`)
* `PUB/SUB` for event stream notifications (“new issues”, “sync complete”)
* `SURVEY` for discovery (“is daemon running?”)

Message payloads: `rkyv` archived structs for low overhead.

---

# Locking (solves the “exclusive checkout” demand safely)

## Lease locks as git refs (distributed, no central server)

Lock ref: `refs/gems/locks/<resource_hash>`

Lock payload commit stores:

```json
{ "owner":"<actor>", "nonce":"...", "expires_unix_ms": ..., "resource":"..." }
```

Acquire:

* fetch lock ref
* if missing or expired → create new lock commit, push ref
* if push rejected → someone else won; retry/backoff

Renew:

* push a new commit extending expiry (must still be owner)

Release:

* push a commit with expiry=0 or delete-ref (delete requires permissions; expiry=0 is safer)

This works with teams and doesn’t deadlock.

(If you want **local-only locks** for single machine workflows, sled file locks are enough.)

---

# “Doctor” redesigned (never destructive)

## Doctor outputs a plan

`gems doctor` does:

* check WAL ref exists and is monotonic
* check sled view matches WAL head
* check local identity keys present
* check remote configured and reachable (optional)
* check locks not stale (optional)

It returns:

* status summary
* remediation plan as explicit commands:

  * `gems rebuild`
  * `gems sync --pull`
  * `gems sync --push`
  * `gems locks gc`

`gems doctor --apply` only runs **safe monotonic actions**:

* rebuild local DB
* fetch refs
* create new WAL commits
  Never: reset, force-push, rewrite refs except your own local.

---

# CLI that’s agent-safe

## Commands (non-interactive by default)

* `gems init`
* `gems issue create --title ... --body ... --label ...`
* `gems issue list --label bug --state open --json`
* `gems issue show <id> --json`
* `gems issue comment <id> --body ...`
* `gems issue close <id>`
* `gems sync [--pull] [--push]`
* `gems doctor [--json] [--apply]`
* `gems export --format md|json` (generated, ignored)

### Structured IO

* `--json` always available
* `--yaml` optional
* `--quiet` for agents

No interactive editors unless explicitly requested:

* `gems issue edit <id> --editor` (human mode)
  Agents will never hit it accidentally.

---

# Merge/conflict semantics

Because we’re appending events, “conflicts” become policy:

* Title/body updates:

  * last-writer-wins by `(ts, actor)` (deterministic)
  * optionally prefer causal chain if `parent` is set
* Labels/assignees:

  * treat add/remove as commutative set ops; apply in causal order if available
* Close/reopen:

  * last state change wins

This yields deterministic state across all replicas after sync.

---

# Repo layout (minimal footprint)

Nothing in working tree is required.

* `.git/gems/sled/` local DB
* `.git/gems/config.toml`
* `.gems/` optional export (gitignored)
* refs:

  * `refs/gems/wal`
  * `refs/gems/locks/*` (optional)

This avoids the `.beads/` tracked file pitfalls.

---

# Implementation plan (Rust)

## Crates

* `libgems-core`

  * event types, hashing, projection, sled store
* `libgems-git`

  * create/read commits, push/fetch refs, pack chunks
* `libgems-ipc`

  * rkyv message schema + async-nng bindings
* `gems` (CLI)

  * clap + json output
* `gemsd` (daemon)

  * async runtime (tokio), sync loop, pub/sub notifications

## Milestone 1: Local-only MVP (no git sync yet)

* create/list/show/close/comment
* sled projections
* export md/json
* tests: deterministic rebuild

## Milestone 2: Git WAL

* write/read WAL commits on local repo
* fetch/pull/push refs
* reconcile on push rejection (fast-forward strategy)

## Milestone 3: Daemon + IPC

* daemon discovery
* `gems sync` uses daemon if present, otherwise direct

## Milestone 4: Locks + team workflows

* lease refs
* lock gc
* `gems issue edit --lock` (optional)

---

# Hard requirements to bake in (to avoid Beads’ failure modes)

1. **Monotonic writes only** (no force push, no ref rewrites except local)
2. **Worktree-safe by design** (no tracked WAL file)
3. **Daemon optional** (CLI correctness always)
4. **Explicit repair** (doctor emits plan; apply is safe)
5. **Deterministic merge** (event-id union + deterministic projection)

---

If you want, I can turn this into:

* a full repo skeleton (Cargo workspace layout),
* message schemas for async-nng,
* the git WAL encoding format (commit tree + chunk sizing),
* and a test matrix (worktrees, concurrent writers, push rejection, partial sync, corrupted sled rebuild).

