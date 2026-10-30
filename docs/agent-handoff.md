# Relay handoff

The baton between leads: at most 60 lines, read on every start. The board
(https://github.com/users/Gaurgle/projects/2) holds status, each ticket's
issue holds its record, brief and rulings, `docs/agent-log.md` holds
finished work and the owner's dated words (the pre-0.8.0 baton, including
the verbatim 2026-10-15 to 2026-10-29 entries and the NZ-30, NZ-47 and
NZ-52 briefs, is archived there, moved 2026-10-30).

## Current lead

None. Lead `7c4dc6c8` (Claude `nz-coordinator`, Fable 5.1) released the
baton 2026-10-29 about 14:25 CET at his stop condition. No workers or
reviewers running. `main` = `origin/main` at `b835e55`. Migrated to Relay
0.8.0 on 2026-10-30 by the advisor session, no lead active.

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

Coordination commits are not delegated: present the docs commit command.
Installing, tags and releases are his. NZ-18 does not run.

## In flight

- NZ-58 (#84): Ready to integrate, gate owner try. Branch
  `feat/nz-58-wrap-indent` at `40b02fb`, base `b835e55`, CI 40877527593
  green, reviewer accepted first (diff sha1 fb5502c7). Try:
  `cargo run --release --manifest-path .claude/worktrees/NZ-58/Cargo.toml -p notez-cli -- -g todo`.

## Open questions

- #77's question: what defines a todoz todo in a file, and should the
  inspector borrow the board's preview wholesale? Needs his yes to ticket.
- Frontmatter drawn as rule plus paragraph in the rendered preview (found
  by NZ-53); needs his yes to ticket.
- NZ-52 (#78) and NZ-47d / NZ-18 queue positions are his call.

## Next step

Merge NZ-58 on his accept (`Closes #84`), then the queue in order: NZ-57
(#83, footer hints, small), NZ-30 (#59, outline), NZ-54 (#80, agenda
`--json`), NZ-50 (write path, create when reached), NZ-47d brief. Per
ticket: worktree from `main`, worker, separate reviewer, lead verify
(touch a source file first), commit, push, green CI, `--no-ff`, Done.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts
from `AGENTS.md`.
