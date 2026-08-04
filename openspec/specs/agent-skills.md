# Agent Skills

Agent-agnostic skills for autonomous task execution using the `ztask` CLI.

## Skill Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  User invokes: /ztask-orchestrator <project-id>             │
│                                                             │
│  Orchestrator                                               │
│    ├─ ztask list --filter incomplete                        │
│    ├─ topological sort (respects depends_on)                │
│    ├─ spawn Worker A ─────────────────────────┐             │
│    ├─ spawn Worker B ────────────────┐        │             │
│    └─ spawn Worker C ─────┐          │        │             │
│                           │          │        │             │
│         Worker ───────────┤          │        │             │
│           claim           │          │        │             │
│           execute (TDD)   │          │        │             │
│           finalize        │          │        │             │
│                           ▼          ▼        ▼             │
│                      ztask update-status                     │
│                                                             │
│    collect results, report summary                          │
└─────────────────────────────────────────────────────────────┘
```

## Skills

| Skill | File | Invocation | Role |
|-------|------|------------|------|
| ztask-orchestrator | `.agents/skills/ztask-orchestrator/SKILL.md` | `/ztask-orchestrator <project-id>` | Coordinator — discovers tasks, spawns workers, monitors |
| ztask-worker | `.agents/skills/ztask-worker/SKILL.md` | (embedded in sub-agent prompt) | Executor — claims, executes, finalizes one task |
| ztask-status | `.agents/skills/ztask-status/SKILL.md` | `/ztask-status <project-id>` | Dashboard — project overview, stalled detection |
| ztask-ingest | `.agents/skills/ztask-ingest/SKILL.md` | `/ztask-ingest <project-id> <spec-dir>` | Ingestion — converts OpenSpec SDD to task graph |

## Design Decisions

1. **Skills are markdown, not code.** They instruct the LLM what to do; the LLM uses `ztask` CLI and `actor` tool.

2. **Worker instructions are embedded, not referenced.** The orchestrator copies the worker lifecycle into sub-agent prompts. This avoids requiring the sub-agent to discover and load a separate skill file.

3. **Status is the contract.** All coordination happens through `ztask update-status`. No shared memory, no IPC. The Zenoh keyspace is the single source of truth.

4. **Agent-agnostic.** Skills live in `.agents/skills/`, not `.mimocode/` or `.claude/`. Any skill-aware agent can use them.

5. **TDD is preferred, not enforced.** The worker skill recommends TDD but allows direct execution for non-code tasks.

## Dependency Model

Tasks declare `depends_on` (list of task IDs). The orchestrator:
1. Fetches all incomplete tasks
2. Builds a dependency graph from `depends_on`
3. Topologically sorts to determine execution order
4. Spawns workers only for tasks whose dependencies are `COMPLETED`
5. After a worker completes, re-evaluates the queue for newly-unblocked tasks

Circular dependencies are detected at ingestion time and rejected.

## Intervention Model

| Trigger | Detection | Action |
|---------|-----------|--------|
| Task fails 2+ times | `attempt_count >= 2` and status is PENDING | Ask user: retry, skip, or intervene |
| Dependency not met | `depends_on` contains non-COMPLETED task | Defer task, continue with others |
| External system unavailable | Worker reports in failure note | Ask user: provide access or skip |
| > 50% tasks failing | Ratio of failed to total | Pause orchestration, ask user |
| Sub-agent timeout/crash | `wait` returns timeout/error | Mark PENDING with note, increment attempt_count |
| Stall detection | `IN_PROGRESS` > 24 hours | Flag in status report |

## Concurrency Model

- Sub-agents run in parallel via the `actor` tool
- Status locking: `update-status` is the claim mechanism; first to set `IN_PROGRESS` wins
- Race detection: if a sub-agent finds the task already `IN_PROGRESS`, it backs off
- No distributed locks needed — Zenoh's eventual consistency is sufficient
