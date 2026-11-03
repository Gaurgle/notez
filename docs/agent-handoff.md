# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each ticket's
issue holds its record, brief and rulings, `docs/agent-log.md` holds
finished work, the owner's dated words and the pre-0.8.0 baton (archived
2026-10-30, with the NZ-30, NZ-47 and NZ-52 briefs).

## Current lead

Lead `67917d20` (Claude `nz-coordinator`, Fable 5.1) took over 2026-11-03
about 12:30 CET on Andreas's "Take over as lead"; lead `7c4dc6c8` released
2026-10-29 (a lead `018zBNkM` commented on #84 on 2026-11-02, merged
nothing, left no baton entry). `main` = `origin/main` at `0c1a5c7`.

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

Coordination commits are not delegated: present the docs commit command.
Installing, tags and releases are his. NZ-18 and NZ-47d do not run.

## In flight

- NZ-62 (#88): Review, gate owner look. Doc
  `.claude/worktrees/NZ-62/docs/design-nz62-sync-sharing.md` (uncommitted,
  diff id 2b30dd82643c) waits for Andreas's approval. Record on #88.

## Open questions

- #77's question: what defines a todoz todo in a file, and should the
  inspector borrow the board's preview wholesale? Needs his yes to ticket.
- Frontmatter drawn as rule plus paragraph in the rendered preview (found
  by NZ-53); needs his yes to ticket.
- NZ-52 (#78) position is his call. NZ-18 and NZ-47d parked (2026-11-03).
- His NZ-57 try (2026-11-03): "add relevant keybinds in the footer and
  help menu": which keys is asked, not answered; NZ-63 (#89) and NZ-64
  (#90, look) created from the same words, Ready, position is his.

## Next step

NZ-57 merged `a6ce239`. NZ-62 doc waits for his approval (no code). Then
the queue in order: NZ-59 (#85), NZ-60 (#86, look), NZ-61 (#87), NZ-30
(#59), NZ-54 (#80), NZ-50 (create when reached); NZ-63 and NZ-64 where he
puts them. Per ticket: worktree from `main`, worker, separate reviewer,
lead verify, commit, push, green CI, `relay-gate`, `--no-ff`, Done.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
