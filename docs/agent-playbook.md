# Agent Playbook

This repository uses Grit as the canonical task and memory system for both humans and agents.

## Non-interactive contract

- Use `--json` for all reads.
- Do not run interactive commands (no editor prompts).
- Do not force-push `refs/grit/*`.
- On inconsistencies, run `grit doctor --json` and follow the plan.

## Startup routine

Run at the beginning of each session:

- `grit sync --pull --json`
- `grit issue list --state open --label agent:todo --json`
- `grit issue list --state open --label priority:P0 --json`

Select exactly one issue at a time.

## Shared repo note

If multiple agents share the same `.git` directory, each agent must use a separate data directory. Set `GRIT_HOME`, `--data-dir`, or `--actor <id>` so the local DB is not shared between processes.

## Plan format

Before coding, post a plan comment:

```
Intended changes: <files/modules>
Tests: <commands>
Rollback: <strategy>
```

## Checkpoints

After each milestone, post a checkpoint comment:

- What changed
- Why
- Tests run

## Locks

Acquire a lock when editing shared or risky areas:

- `grit lock acquire --resource "path:<FILE>" --ttl 15m --json`
- `grit lock renew --resource "path:<FILE>" --ttl 15m --json`
- `grit lock release --resource "path:<FILE>" --json`

If a lock is unavailable, pick another issue or coordinate in comments.

## Finish

Before closing:

- Post verification notes (commands + expected output)
- `grit issue close <ID> --reason done --json`
- `grit sync --push --json`
