# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each issue its
record, brief and rulings, `docs/agent-log.md` finished work, the owner's
dated words and the pre-0.8.0 baton (archived 2026-10-30).

## Current lead

None. Lead `67917d20` (Claude `nz-coordinator`, Fable 5.1) RELEASED the
baton 2026-11-03 evening on Andreas's "find a good place to stop for
today". No workers or reviewers running. `main` = `origin/main` at
`dc84401` (CI run 41677789041; confirm green on the next start).

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
| NZ-59, NZ-60, NZ-61 added to the queue after NZ-57, in that order; NZ-62 design doc only, to him | 2026-11-03, verbatim in the log | same queue rules; NZ-60 is a look change | queue done |

Coordination commits are not delegated: present the docs commit command. Installing, tags and releases are his. NZ-18 and NZ-47d do not run.

## In flight

- NZ-62 (#88): Review, gate owner look. Doc
  `.claude/worktrees/NZ-62/docs/design-nz62-sync-sharing.md` (uncommitted,
  diff id 2b30dd82643c) waits for Andreas's approval. Record on #88.
- The sync runner test in `tui/sync.rs` flakes (6 times today, 2 of 4 on
  the base): NZ-68 (#94, test-only, his yes 2026-11-03) fixes it; run it
  first so CI on `main` stops being at risk.

## Open questions

- Need his yes to ticket: #77's question (what defines a todoz todo in a
  file; borrow the board's preview?); frontmatter drawn as rule plus
  paragraph in the rendered preview (NZ-53).
- NZ-52 (#78) position is his call. NZ-18 and NZ-47d parked (2026-11-03).
- NZ-62 design doc: his verdict (approve, amend, park) is still open.

## Next step

Queue (his 2026-11-03 "continue with the tickets"): NZ-68 (#94) first,
then NZ-65 (#91, look), NZ-66 (#92, look), NZ-67 (#93, look, "dont
start implementing" until he queues it), NZ-30 (#59), NZ-50 (create
when reached). Per ticket: worktree from `main`, worker, separate
reviewer, lead verify, commit, push, CI, `relay-gate`, Done.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
