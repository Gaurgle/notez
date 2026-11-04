# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each issue its
record, brief and rulings, `docs/agent-log.md` finished work, the owner's
dated words and the pre-0.8.0 baton (archived 2026-10-30).

## Current lead

Lead `018zBNkK` (Claude `nz-coordinator`, Fable 5.1) HOLDS the baton
since 2026-11-04, taken from the released state of lead `67917d20`
(2026-11-03). `main` = `origin/main` at `de71d36` (NZ-50c merged; CI run
41823800494 green). Andreas 2026-11-04: "continue" (the queue below).

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

- NZ-50b (#98): Ready to integrate, gate owner look; f7751d9, CI green, gate
  holds on de71d36; `.claude/worktrees/NZ-50b`. Record on #98.
- NZ-65 (#91): Ready to integrate, gate owner look; 744543b, gate holds on
  c51a378; `.claude/worktrees/NZ-65`. Likely superseded by NZ-69; lead says drop, his call.
- NZ-62 (#88): Review, gate owner look; doc uncommitted in
  `.claude/worktrees/NZ-62` (diff 2b30dd82643c). His 2026-11-04 "Other
  tickets can go ahead" parks it, verdict open. Record on #88.

## Open questions

- NZ-71 (#100, Backlog): his 2026-11-04 note, creating a dated todo or
  note must be intuitive (forms such as +1y, +1mo, +2h; surfaces); design first.
- Need his yes to ticket: #77's question (what defines a todoz todo in a
  file; borrow the board's preview?); frontmatter drawn as rule plus
  paragraph in the rendered preview (NZ-53).
- NZ-52 (#78) position is his call. NZ-18 and NZ-47d parked (2026-11-03).
- NZ-69 (#95), NZ-70 (#96): Ready from his 2026-11-04 words; assumptions
  in the bodies await his go. DESIGN.md "one key, one function" line: proposed.

## Next step

Queue (his 2026-11-03 "continue with the tickets", 2026-11-04 "other
tickets can go ahead"): NZ-50b (#98, his look); NZ-65 (his call); NZ-69, NZ-70 (#95, #96, on his go);
NZ-67 (#93, parked until he queues it); NZ-30 (#59, no brief yet). Per ticket: worktree from `main`, worker, separate
reviewer, lead verify, commit, push, CI, `relay-gate`, Done.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
