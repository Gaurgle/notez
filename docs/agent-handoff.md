# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each issue its
record, brief and rulings, `docs/agent-log.md` finished work, the owner's
dated words and the pre-0.8.0 baton (archived 2026-10-30).

## Current lead

Lead `018zBNkK` (Claude `nz-coordinator`; Fable 5.1, Opus 5.5 since his
`/model` 2026-11-05) HOLDS the baton
again since 2026-11-05 on Andreas's "continue" (released 2026-11-04
16:15, no lead in between). `main` = `origin/main` at `bb3c913` (NZ-77a
merged; CI run 42032941123 green).

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

Coordination docs: lead `018zBNkK` may commit and push them this session (Andreas 2026-11-04, "You may commit and push this session"; ends with the session). Installing, tags and releases are his. NZ-18 and NZ-47d do not run.

## In flight

- NZ-76 (#105): Ready to integrate, gate owner look; e0ed58f (diff d424c329e66c), gate holds;
  NZ-77a merged as bb3c913 (Refs NZ-77, #106 open for NZ-77b). Worktree `.claude/worktrees/NZ-76`. NZ-77b after NZ-76.
- NZ-62 (#88): Review, gate owner look; doc uncommitted in
  `.claude/worktrees/NZ-62` (diff 2b30dd82643c), verdict open. Record on #88.

## Open questions

- NZ-71 (#100, Backlog): intuitive dated-todo creation, design first; NZ-74
  (#103, Ready): pane 2 drawn like the todoz board, design first; NZ-78 (#107,
  Backlog): what defines a log, change-date action, design first (2026-11-05).
- Need his yes to ticket: #77's question (what defines a todoz todo in a
  file; borrow the board's preview?); frontmatter drawn as rule plus
  paragraph in the rendered preview (NZ-53).
- NZ-52 (#78) his call; NZ-18, NZ-47d parked. NZ-69 (#95) Ready on his go; NZ-70
  (#96, scopes) deferred by him 2026-11-05; DESIGN.md "one key, one function" line proposed, not written.

## Next step

Queue (his 2026-11-03 "continue with the tickets", 2026-11-04 "other
tickets can go ahead"): NZ-76 (#105) in review, then
NZ-77b; NZ-65 (his call); NZ-69 (on his go); NZ-67 parked; NZ-30
(no brief). Per ticket: worktree, worker, reviewer, verify, CI, gate, Done.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
