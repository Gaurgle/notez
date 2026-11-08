# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each issue its
record, brief and rulings, `docs/agent-log.md` finished work, the owner's
dated words and the pre-0.8.0 baton (archived 2026-10-30).

## Current lead

None. Lead `c26dc5ef` (Claude `nz-coordinator`, Fable 5.1) RELEASED the
baton 2026-11-08 14:25 on Andreas's "then stop for today". No workers or
reviewers running. `main` = `origin/main` = `24c81e4` (NZ-80, CI green).
One worktree: `.claude/worktrees/NZ-76` (branch pushed, see In flight).

## Standing authority

| What | Granted | Scope | Until |
|---|---|---|---|
| "Merge as you go": reviewed diff, green branch CI, safe-merge checks, `merge --no-ff`, push | 2026-10-07, reapplied to 2026-10-15 | all Relay tickets | revoked by word |
| Push ticket branch on accept; delete the remote branch after the merge | 2026-10-08 15:15 | standing; no force-push, tags, releases | revoked by word |
| Create the issue for a ticket Andreas asks for in a lead session | 2026-10-08 16:35 | not for lead-invented tickets; never delete items or change visibility | revoked by word |
| Dependencies `pulldown-cmark`, `tree-sitter` and grammars, pinned | 2026-10-08 17:20 | preview family only | revoked by word |
| Decide open design points, report at milestones only | 2026-10-08, updated 2026-10-15 | stop him only for: irreversible or costly actions, his files, `notez-core` API or format, dependencies outside the family, CI, tags, releases, out-of-scope tickets | revoked by word |
| Up to three workers on disjoint files, merges one at a time | 2026-10-10 14:05 | standing | revoked by word |
| Run the 2026-10-15 queue (see Next step) | 2026-10-15, two entries, archived verbatim | the queue below; look changes stop at Ready to integrate for his try | queue done |
| Architecture refactor tickets P1 to P6 (`docs/architecture.md`): file, run in the recommended order, P1 before or with NZ-76, merge as you go | 2026-11-08 (words in the log) | NZ-80 to NZ-85 (#109 to #114); one seam each, no behavior change | NZ-85 Done |

Installing, tags and releases are his. NZ-18 and NZ-47d do not run.

## In flight

- NZ-76 (#105): amendment 3 Ready to integrate, gate owner look: 2b8869a on
  `feat/nz-76-rename-stem`, diff 76384e3b323d accepted, branch CI green,
  worktree `.claude/worktrees/NZ-76`. Tried 2026-11-08: refusals hold,
  TODO.md lock kept (on #105); `r` inside `_todos` not yet seen. His "merge
  NZ-76": combined check, gate, merge. Then NZ-86 (#115), NZ-77b (#106),
  NZ-81 to NZ-85 (P3, P4, P2, P5, P6) in that order. NZ-80 Done (log).
- NZ-62 (#88): Review, gate owner look; the uncommitted design doc was lost
  with its worktree (2026-11-08); summary is the last comment on #88; redo
  needs his word.

## Open questions

- Design first, on his go: NZ-71 (#100), NZ-74 (#103), NZ-78 (#107).
- Need his yes to ticket: #77's question (what defines a todoz todo in a
  file); frontmatter drawn as rule plus paragraph in the preview (NZ-53).
- NZ-52 (#78) his call; NZ-18, NZ-47d parked; NZ-70 (#96) deferred
  2026-11-05; DESIGN.md "one key, one function" line proposed, not written.

## Next step

NZ-80 to merge, then NZ-76 amendment 3 rebased on it (same file,
`rename_prompt.rs`), then NZ-81, NZ-82, NZ-83 (alone: NZ-69 shares
`tui/todo.rs`), NZ-84, NZ-85. Still queued (his 2026-11-03/04 words): NZ-77b;
NZ-65 (his call); NZ-69 (on his go); NZ-67 parked; NZ-30 (no brief).

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
