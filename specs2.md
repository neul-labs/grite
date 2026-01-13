Here’s the **crisp, non-hand-wavy answer** to *“what does this actually improve over Beads?”* — mapped **directly to Beads’ real failure modes**, not abstract theory.

---

## Executive summary (one screen)

**Beads works** because it:

* makes agent state local, versioned, auditable
* uses git as the distribution layer
* gives agents a CLI they can reason about

**Beads breaks down** because it:

* treats *git branches + tracked files* as a database
* relies on a daemon for correctness
* performs destructive “repairs”
* fights worktrees and multi-agent concurrency

**Your design (Gems)** keeps the *same strengths* but changes **one core abstraction**:

> **Git is used only as an append-only log transport, never as mutable state.**

That single shift eliminates ~80% of the issues in Beads.

---

## Point-by-point: Beads vs Gems

### 1. Tracked JSONL vs Git-ref WAL

**This is the biggest improvement.**

| Beads                                   | Gems                         |
| --------------------------------------- | ---------------------------- |
| `.beads/issues.jsonl` is a tracked file | WAL lives in `refs/gems/wal` |
| Working tree gets dirty                 | Working tree stays clean     |
| Worktrees fight each other              | Fully worktree-safe          |
| Git merge semantics apply               | Merge = set-union of events  |

**Why Beads suffers**

* Git is *bad* at being a mutable database
* Line-based diffs ≠ semantic merges
* Worktrees share index state → phantom changes

**Why Gems fixes it**

* Git refs are append-only and monotonic
* No files are edited in the working tree
* Git is used for what it’s good at: **distribution + immutability**

This alone removes:

* phantom diffs
* worktree bugs
* “sync branch diverged” confusion

---

### 2. Branch semantics vs Explicit WAL semantics

| Beads                            | Gems                     |
| -------------------------------- | ------------------------ |
| Meaning encoded in branch shape  | Meaning encoded in data  |
| “sync branch health” heuristics  | No branch health concept |
| Divergence triggers repair logic | Divergence is normal     |

**Why Beads breaks**

* Branch topology is ambiguous
* Tools guess intent and guess wrong
* Leads to resets / force pushes

**Gems rule**

> There is **exactly one meaning**:
> *“These events happened.”*

No “main vs sync vs diverged” logic at all.

---

### 3. Destructive doctor vs Monotonic repair

| Beads                           | Gems                          |
| ------------------------------- | ----------------------------- |
| `doctor --fix` rewrites history | Doctor only proposes actions  |
| Can force-reset branches        | Never rewrites published refs |
| User trust breaks instantly     | Repair is explicit + safe     |

**Critical difference**

* Beads assumes it knows the *correct* state
* Gems assumes it **never knows** and therefore:

  * only adds
  * rebuilds
  * replays

This is the difference between:

* *“tool acting as authority”*
* *“tool acting as ledger”*

Agents trust ledgers.

---

### 4. Daemon as requirement vs Daemon as accelerator

| Beads                           | Gems                  |
| ------------------------------- | --------------------- |
| Daemon required for correctness | Daemon optional       |
| Lock recursion bug causes crash | CLI always works      |
| Hidden background behavior      | Explicit IPC boundary |

**Why this matters for agents**
Agents:

* are ephemeral
* are restarted often
* cannot debug background processes

Gems guarantees:

> **Every CLI command is a full transaction.**

Daemon only provides:

* cache warmth
* background sync
* pub/sub notifications

Nothing breaks without it.

---

### 5. Ad-hoc concurrency vs Formal event model

| Beads                   | Gems                     |
| ----------------------- | ------------------------ |
| Line-based JSONL append | Content-addressed events |
| Implicit ordering       | Deterministic resolution |
| Conflicts via git       | Conflicts via policy     |

**Gems event model advantages**

* Each event has:

  * ID
  * actor
  * timestamp
  * optional causal parent
* Merge = union of events
* Projection is deterministic

This is **CRDT-like behavior without CRDT complexity**.

---

### 6. “Exclusive checkout” vs Lease locks

| Beads                     | Gems                  |
| ------------------------- | --------------------- |
| No real locking primitive | Lease-based locks     |
| Users ask for exclusivity | Supported natively    |
| Git can’t enforce it      | Git refs + expiry can |

This directly answers an open Beads demand **without lying about what git can do**.

---

### 7. Human-first CLI vs Agent-first CLI

| Beads                           | Gems                       |
| ------------------------------- | -------------------------- |
| Interactive commands by default | Non-interactive by default |
| Editors surprise agents         | Explicit `--editor` only   |
| Weak JSON discipline            | Structured IO everywhere   |

This is why Gems fits cleanly into `agents.md`:

* predictable
* machine-parsable
* no footguns

---

## What stays the same (intentionally)

This is important: **you are not rejecting Beads’ philosophy**.

You keep:

* repo-local memory
* offline-first operation
* git-backed audit trail
* append-only history
* agent-usable CLI
* SQLite/sled materialized views
* JSON/YAML exports

So adoption friction is low.

---

## The one-line mental model shift

**Beads**

> “Git is the database.”

**Gems**

> “Git transports immutable facts; the database is derived.”

That distinction is why:

* worktrees stop breaking
* doctor stops being dangerous
* multi-agent usage becomes boring (in a good way)

---

## If you want a brutal verdict

Beads is:

* a **brave proof-of-concept**
* showing *why* “git as database” is seductive but sharp

Gems is:

* the **production-grade correction**
* keeping the idea, fixing the substrate

If you want, next we can:

* map each open Beads issue → exact Gems design choice that eliminates it
* or write a short “Why we built Gems instead of Beads” README that is honest, respectful, and persuasive.

