---
title: Manual PR Review Workflow
updated: 2026-06-12
---

# Manual PR Review Workflow

Use this when running an explicit manual review session across multiple open
PRs. This process is intentionally serial and decision-gated.

## Review Loop

1. Enumerate open PRs sorted by PR number.
2. Review one PR at a time.
3. For each PR, provide:
   - intent and product-fit assessment
   - copy/paste smoke-test commands
   - expected pass/fail signals
   - concrete findings with severity and file/line references
4. Wait for an explicit decision before moving on:
   - `accept <PR#>`
   - `reject <PR#>`
5. Keep a visible accept/reject log throughout the session.

Do not merge or close PRs during the active review loop.

## Review Bar

- Verify behavior and product/API shape, not only green tests.
- Separate blocking issues from follow-up issues.
- Prefer existing per-PR worktrees when `gh pr checkout` reports branch or
  worktree conflicts.
- Avoid `set -e` / `set -o pipefail` in user-facing pasted commands.
- Use deterministic temp-DB smoke scripts for CLI features and clean up seeded
  data.
- Search tracking DBs for existing matching feature items before creating new
  ones; complete existing items instead of creating duplicates when appropriate.

## Finalization

When the session ends, run finalization strictly serially. Merge/close actions
must be one command at a time.

1. Fetch latest remote refs: `git fetch origin`.
2. Sync `main`.
3. Merge accepted PRs in PR-number order.
4. If conflicts occur:
   - fetch again
   - merge current `origin/main` into each accepted PR branch
   - resolve conflicts
   - test
   - push
   - then merge the PR
5. Close rejected PRs with a short comment.
6. Report remaining open PRs for the next session.

Prefer a clean integration worktree for finalization. If local `main` cannot
fast-forward because of local modifications, do not force-reset; report the
blocking files and current `HEAD` versus `origin/main`. After each merge/close
action, re-check PR state before continuing.
