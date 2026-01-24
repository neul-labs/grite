# CRDT Merging

Grit uses CRDT (Conflict-free Replicated Data Type) semantics for deterministic merging. This ensures no manual conflict resolution is ever needed.

## Overview

CRDTs allow distributed systems to merge concurrent changes without conflicts. Grit uses two CRDT strategies:

1. **Last-Writer-Wins (LWW)**: For scalar fields like title and body
2. **Add/Remove Sets**: For collection fields like labels and assignees

## Merge Strategies by Field

| Field | Strategy | Behavior |
|-------|----------|----------|
| Title | Last-writer-wins | Latest update wins |
| Body | Last-writer-wins | Latest update wins |
| State | Last-writer-wins | Latest state change wins |
| Labels | Add/remove set | Add and remove operations commute |
| Assignees | Add/remove set | Add and remove operations commute |
| Dependencies | Add/remove set | Add and remove operations commute |
| Comments | Append-only list | All comments preserved |
| Links | Append-only list | All links preserved |
| Attachments | Append-only list | All attachments preserved |
| File context | Last-writer-wins (per path) | Latest index wins |
| Project context | Last-writer-wins (per key) | Latest value wins |

## Last-Writer-Wins (LWW)

For scalar fields, the most recent update wins.

### Comparison Key

Updates are compared by a tuple:

```
(ts_unix_ms, actor, event_id)
```

1. Higher timestamp wins
2. If tied, lexicographically greater actor wins
3. If still tied, lexicographically greater event_id wins

### Example

```
Event A: ts=1000, actor=aaa, title="Fix bug"
Event B: ts=1000, actor=bbb, title="Fix login bug"

Result: title = "Fix login bug" (bbb > aaa lexicographically)
```

### Tie-Breaking Rationale

- Timestamps may be identical due to clock skew
- Actor comparison provides deterministic ordering
- Event ID as final tiebreaker ensures total ordering

## Add/Remove Sets

For collection fields like labels and assignees, grit uses add/remove sets.

### Semantics

- Adding an element: Insert into set
- Removing an element: Remove from set
- Operations are commutative: Order doesn't matter

### Example: Labels

```
Event 1: LabelAdded { label: "bug" }
Event 2: LabelAdded { label: "feature" }
Event 3: LabelRemoved { label: "bug" }

Final labels: ["feature"]
```

### Concurrent Operations

```
# Concurrent on different actors:
Actor A: LabelAdded { label: "bug" }
Actor B: LabelRemoved { label: "bug" }

# Both applied in timestamp order
# If A's add comes after B's remove, label is present
# If B's remove comes after A's add, label is absent
```

The final state depends on event timestamps, but is always deterministic.

## Append-Only Lists

Comments, links, and attachments are append-only lists.

### Ordering

Events are ordered by:

```
(ts_unix_ms, actor, event_id)
```

This creates a deterministic total order across all actors.

### Example: Comments

```
Actor A @ ts=1000: CommentAdded { body: "Working on it" }
Actor B @ ts=1001: CommentAdded { body: "Found the bug" }
Actor A @ ts=1002: CommentAdded { body: "Fixed!" }

Final comment order:
1. "Working on it" (A @ 1000)
2. "Found the bug" (B @ 1001)
3. "Fixed!" (A @ 1002)
```

## Projection Algorithm

The projection algorithm folds events in timestamp order:

```python
def compute_projection(issue_id, events):
    proj = IssueProjection()

    # Sort events by (ts, actor, event_id)
    sorted_events = sort(events, key=lambda e: (e.ts, e.actor, e.event_id))

    for event in sorted_events:
        match event.kind:
            case IssueCreated(title, body, labels):
                proj.title = title
                proj.body = body
                proj.labels = set(labels)
                proj.created_ts = event.ts

            case IssueUpdated(title, body):
                if title: proj.title = title
                if body: proj.body = body

            case StateChanged(state):
                proj.state = state

            case LabelAdded(label):
                proj.labels.add(label)

            case LabelRemoved(label):
                proj.labels.discard(label)

            case CommentAdded(body):
                proj.comments.append(Comment(event.actor, body, event.ts))

            # ... similar for other event kinds

        proj.updated_ts = event.ts
        proj.version = event.event_id

    return proj
```

## Output Ordering

For deterministic output, collections are sorted:

- `labels`: Lexicographically sorted
- `assignees`: Lexicographically sorted
- `comments`: By timestamp then actor then event_id

## Conflict Scenarios

### Concurrent Title Updates

```
Actor A @ ts=100: IssueUpdated { title: "Title A" }
Actor B @ ts=100: IssueUpdated { title: "Title B" }

# Same timestamp, compare actors
# Result: "Title B" wins (B > A)
```

### Add and Remove Same Label

```
Actor A @ ts=100: LabelAdded { label: "bug" }
Actor B @ ts=101: LabelRemoved { label: "bug" }

# Apply in order: add then remove
# Result: label absent
```

```
Actor A @ ts=101: LabelAdded { label: "bug" }
Actor B @ ts=100: LabelRemoved { label: "bug" }

# Apply in order: remove then add
# Result: label present
```

### Concurrent Close and Reopen

```
Actor A @ ts=100: StateChanged { state: Closed }
Actor B @ ts=101: StateChanged { state: Open }

# Apply in order
# Result: Open (later timestamp wins)
```

## Sync Behavior

During sync, events from multiple actors merge automatically:

1. Pull fetches remote events
2. Events sorted by timestamp
3. Projection computed from combined events
4. All changes preserved, no conflicts

### Example Sync

```
Local events:
  ts=100: IssueCreated { title: "Bug" }
  ts=102: CommentAdded { body: "Local comment" }

Remote events:
  ts=101: LabelAdded { label: "urgent" }
  ts=103: CommentAdded { body: "Remote comment" }

After sync (merged):
  ts=100: IssueCreated { title: "Bug" }
  ts=101: LabelAdded { label: "urgent" }
  ts=102: CommentAdded { body: "Local comment" }
  ts=103: CommentAdded { body: "Remote comment" }

Final state:
  title: "Bug"
  labels: ["urgent"]
  comments: ["Local comment", "Remote comment"]
```

## Dependencies

Dependencies use the same add/remove set semantics as labels.

### Add/Remove Semantics

```
Actor A: DependencyAdded { target: issue-2, dep_type: Blocks }
Actor B: DependencyRemoved { target: issue-2, dep_type: Blocks }

# Applied in timestamp order, same as labels
```

### Cycle Detection

Cycle detection for `blocks` and `depends_on` types is a **local validation** at command time. It is not enforced at the CRDT level because:

- Concurrent edges from different actors cannot be validated against each other without coordination
- The CRDT accepts all well-formed events
- `grit doctor` detects cycles that formed due to concurrent operations
- The `related_to` type has no acyclicity constraint

### Concurrent Conflict Example

```
Actor A: DependencyAdded { source: issue-1, target: issue-2, dep_type: Blocks }
Actor B: DependencyAdded { source: issue-2, target: issue-1, dep_type: Blocks }

# Both are accepted by the CRDT (no coordination needed)
# Result: A cycle exists in the Blocks graph
# grit doctor will flag this for manual resolution
```

## Context Store

Context events use LWW semantics per file path (or per key for project context).

### File Context LWW

Each file path has at most one `FileContext` projection. When multiple `ContextUpdated` events exist for the same path, the one with the highest `(ts, actor, event_id)` version wins.

### Project Context LWW

Each key has at most one `ProjectContextEntry`. The latest `ProjectContextUpdated` event per key wins.

### Why LWW for Context

Context represents the current state of a file or project setting. Unlike labels (where multiple values coexist), there's only one correct current context for a given file path. LWW naturally resolves concurrent updates by preferring the most recent indexing.

## Guarantees

1. **Convergence**: All actors with same events compute same projection
2. **Commutativity**: Event order during sync doesn't affect final state
3. **Idempotency**: Applying same event twice has no additional effect
4. **No conflicts**: Merging never requires manual intervention

## Next Steps

- [Data Model](data-model.md) - Event structure
- [Git WAL](git-wal.md) - Event storage
- [Syncing Guide](../guides/syncing.md) - Practical sync usage
