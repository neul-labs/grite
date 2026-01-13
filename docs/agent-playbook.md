# Agent Playbook

This repository uses Gems as the canonical task and memory system for both humans and agents.

## Non-interactive contract

- Use `--json` for all reads.
- Do not run interactive commands (no editor prompts).
- Do not force-push `refs/gems/*`.
- On inconsistencies, run `gems doctor --json` and follow the plan.

## Startup routine

Run at the beginning of each session:

- `gems sync --pull --json`
- `gems issue list --state open --label agent:todo --json`
- `gems issue list --state open --label priority:P0 --json`

Select exactly one issue at a time.

## Shared repo note

If multiple agents share the same `.git` directory, each agent must use a separate data directory. Set `GEMS_HOME`, `--data-dir`, or `--actor <id>` so the local DB is not shared between processes.

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

- `gems lock acquire --resource "path:<FILE>" --ttl 15m --json`
- `gems lock renew --resource "path:<FILE>" --ttl 15m --json`
- `gems lock release --resource "path:<FILE>" --json`

If a lock is unavailable, pick another issue or coordinate in comments.

## Finish

Before closing:

- Post verification notes (commands + expected output)
- `gems issue close <ID> --reason done --json`
- `gems sync --push --json`
