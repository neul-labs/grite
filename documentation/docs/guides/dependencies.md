# Dependencies

Grite supports typed relationships between issues with cycle detection and topological ordering. Dependencies sync automatically between actors via `grite sync`.

## Dependency Types

| Type | Meaning | Acyclic |
|------|---------|---------|
| `blocks` | This issue blocks the target | Yes |
| `depends_on` | This issue depends on the target | Yes |
| `related_to` | Symmetric link between issues | No |

## Adding Dependencies

```bash
# Issue A blocks issue B
grite issue dep add <issue-a> --target <issue-b> --type blocks

# Issue A depends on issue B
grite issue dep add <issue-a> --target <issue-b> --type depends_on

# Issues are related (symmetric, no direction)
grite issue dep add <issue-a> --target <issue-b> --type related_to
```

Both issues must exist before adding a dependency.

## Removing Dependencies

```bash
grite issue dep remove <issue-a> --target <issue-b> --type blocks
```

## Listing Dependencies

```bash
# Show what issue-a depends on / blocks
grite issue dep list <issue-a>

# Show what depends on / is blocked by issue-a
grite issue dep list <issue-a> --reverse
```

### Example Output

```json
{
  "issue_id": "8057324b1e03afd6...",
  "direction": "dependencies",
  "deps": [
    {
      "issue_id": "a1b2c3d4e5f67890...",
      "dep_type": "blocks",
      "title": "Fix login page"
    }
  ]
}
```

## Topological Ordering

Get issues in dependency order (issues with no blockers first):

```bash
# All open issues in dependency order
grite issue dep topo --state open

# Filter by label
grite issue dep topo --state open --label sprint-1
```

This uses Kahn's algorithm over the dependency DAG. Useful for determining which issues can be worked on next.

## Cycle Detection

For `blocks` and `depends_on` types, grite prevents cycles at command time:

```bash
$ grite issue dep add <a> --target <b> --type blocks
# OK

$ grite issue dep add <b> --target <a> --type blocks
# Error: Adding this dependency would create a cycle in the blocks graph
```

The `related_to` type has no cycle constraint since it represents symmetric relationships.

## Distributed Behavior

Dependencies are CRDT-compatible (add/remove sets). This means:

- Dependencies sync automatically via `grite sync`
- Concurrent add + remove of the same edge: add wins
- No manual conflict resolution needed

### Edge Case: Distributed Cycles

In rare cases, two actors may concurrently add edges that create a cycle:

```
Actor A: dep add issue-1 --target issue-2 --type blocks
Actor B: dep add issue-2 --target issue-1 --type blocks
```

Each operation passes local cycle detection, but after sync a cycle exists. Use `grite doctor` to detect these cases.

## Workflow Example

```bash
# Create a set of related issues
grite issue create --title "Design API schema" --label sprint-1
# → issue-1

grite issue create --title "Implement API endpoints" --label sprint-1
# → issue-2

grite issue create --title "Write API tests" --label sprint-1
# → issue-3

# Set up dependencies
grite issue dep add <issue-2> --target <issue-1> --type depends_on
grite issue dep add <issue-3> --target <issue-2> --type depends_on

# View execution order
grite issue dep topo --state open --label sprint-1
# Returns: issue-1, issue-2, issue-3 (correct order)
```
