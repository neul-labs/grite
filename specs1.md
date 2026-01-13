In an `agents.md` (or “Agent Playbook”) you want two things:

1. **A contract**: how an agent should read/write state safely and non-interactively.
2. **A workflow**: when to call the tool (and what to do with the results) during coding.

Below is how coding agents would use the improved tool (Gems) in practice, plus a ready-to-drop `agents.md` outline.

---

## What agents actually do with it

### 1) Bootstrapping: “what am I doing?”

Agents start every session by pulling the project’s work state:

* `gems sync --pull --json`
* `gems issue list --state open --label "agent:todo" --json`
* `gems issue list --state open --label "priority:P0" --json`

They pick one issue, read it, and treat it as the **source of truth task**.

### 2) Planning: store the plan as structured state

Before touching code, the agent writes its plan back:

* `gems issue comment <id> --body "<plan>"`

If you want this machine-usable, the agent posts YAML/JSON in the comment body.

### 3) Execution: checkpoints + evidence

During work, the agent posts short checkpoints:

* what changed (files/modules)
* why
* test results
* any decisions or tradeoffs

Example:

* `gems issue comment <id> --body "Checkpoint: implemented X; updated Y; tests: cargo test OK"`

### 4) Coordination: locks for risky edits

If the repo has multiple agents/humans, the agent can acquire a lease lock:

* `gems lock acquire --resource "path:src/parser.rs" --ttl 15m --json`
* work
* `gems lock release --resource "path:src/parser.rs"`

If lock is not available, agent chooses another task or waits/retries with backoff (but doesn’t spin forever).

### 5) Finishing: close or handoff

Agent posts:

* summary
* how to verify
* remaining TODOs / follow-ups as new issues

Then:

* `gems issue close <id> --reason "done" --json`
* `gems sync --push --json`

---

## Why this is better than “just GitHub issues” for agents

* Agents need **local, toolable, structured** state with low latency.
* Agents need to work offline and still capture decisions.
* Agents benefit from a deterministic state machine (“events → projection”), not free-form markdown.

And because state lives in git refs (monotonic), agents don’t break worktrees, don’t create dirty working trees, and don’t require a daemon.

---

## `agents.md` content (drop-in)

Here’s a good structure that coding agents follow reliably.

### Section A — Rules (the contract)

* Always use `--json` for reads.
* Never call interactive commands (no editor).
* Use `gems doctor` on errors; do not attempt manual git surgery.
* Post a plan before coding, and a verification note before closing.

### Section B — Startup routine

Commands agents run at the start.

### Section C — Selecting work

How to pick issues (labels, priorities).

### Section D — Recording progress

The required comment format.

### Section E — Locking rules

When to acquire locks and TTL guidance.

### Section F — Sync rules

Pull at start, push at end, also after large checkpoints.

### Section G — Failure modes

What to do if push rejected, db stale, locks stuck.

---

## Example `agents.md` (minimal but strong)

```md
# Agent Playbook (Gems)

This repo uses `gems` as the canonical task/memory system for both humans and coding agents.

## Non-interactive contract
- Use `--json` for all reads.
- Do NOT run interactive commands (no editor prompts).
- Do NOT run raw `git push --force` for any `refs/gems/*`.
- If something looks inconsistent, run `gems doctor --json` and follow the suggested plan.

## Startup routine
Run:
- `gems sync --pull --json`
- `gems issue list --state open --label agent:todo --json`
- `gems issue list --state open --label priority:P0 --json`

Select exactly one issue to work on at a time.

## Plan format
Before changing code, post a plan comment:

`gems issue comment <ID> --body "<PLAN>"`

PLAN should include:
- Intended changes (files/modules)
- Tests to run
- Rollback strategy (if risky)

## Checkpoints
After each meaningful milestone:
- Post a checkpoint comment (what/why/tests).
- Prefer short, factual updates.

## Locks (when to use)
Acquire a lock when editing:
- shared core modules
- schema definitions
- build/release pipelines

Commands:
- `gems lock acquire --resource "path:<FILE>" --ttl 15m --json`
- `gems lock renew --resource "path:<FILE>" --ttl 15m --json` (if needed)
- `gems lock release --resource "path:<FILE>" --json`

If lock acquisition fails:
- choose a different issue OR
- leave a comment asking for coordination.

## Finish criteria
Before closing an issue:
- Post a verification comment:
  - commands to run
  - expected output
  - any caveats
- Close the issue:
  - `gems issue close <ID> --reason done --json`
- Push state:
  - `gems sync --push --json`

## Troubleshooting
- If sync push fails (remote advanced): run `gems sync --pull --json` then retry `--push`.
- If local state seems stale: run `gems rebuild --json`.
- If locks appear stuck: run `gems locks gc --json` (never delete refs manually).
```

---

## How this plugs into real coding agents (Claude Code/Codex style)

Coding agents generally have:

* a **planner loop** (“decide what to do next”)
* an **executor loop** (“edit files, run tests”)
* a **memory layer** (notes, tasks, scratch)

Gems becomes the memory + task substrate.

A typical agent loop:

1. Pull + list P0 issues
2. Pick one issue → `show`
3. Comment a plan
4. Acquire lock if needed
5. Edit code + run tests
6. Comment checkpoints
7. Close issue + push

Because all of those are **deterministic shell commands with structured outputs**, it works with basically any coding agent that can run a CLI.

---

If you want, I can generate a **more opinionated `agents.md`** for your style (e.g., labels like `agent:todo`, `agent:blocked`, `risk:high`, `scope:small`) and a **strict comment schema** that makes it trivial to parse and build dashboards from.

