# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. There is no board; this file holds
ticket status, in-flight work, decisions and the next authorized step.

## Current lead

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`aa27d2ed-a3e9-498c-ae61-db84f43eae7c`, took the baton on 2026-10-06 15:01
CEST. First lead since setup: no previous lead, no ticket worktrees or
branches, no workers running at takeover. Baton held, not released.

## Authorized by the owner

- 2026-10-06, Andreas, in the setup session: set up Relay in this repo with
  prefix `nz`, checks `cargo build --workspace` and `cargo test --workspace`,
  no board, default models (lead `fable`, worker and reviewer `opus`, small
  `sonnet`). Integration: the lead presents commands; no commit, push or merge
  permission is granted.
- 2026-10-06, Andreas, in the lead session (`aa27d2ed`), answering the lead's
  three questions:
  - "you may start": run NZ-1. Limit: this ticket only. Stop condition: Ready
    to integrate, with commands presented. No commit, push or merge of ticket
    code is granted.
  - "can you commit the handoff?": the lead commits `docs/agent-handoff.md`
    once, for the takeover and NZ-1 dispatch record. It is not a standing
    permission, and it does not cover a push.
  - "3. yes": the lead drafts briefs for the four UI areas while NZ-1 runs.
    Drafting only; none of them is authorized to run.
- No other ticket execution is authorized. Andreas names which tickets run.

## In flight

NZ-1, dispatched 2026-10-06 about 15:10 CEST:

- Base commit: `85300ef` (`main` and `origin/main` at dispatch).
- Branch: `feat/NZ-1-pull-on-open`.
- Worktree: `/Users/at-a/Repos/notez/.claude/worktrees/NZ-1`.
- Worker: `nz-worker`, model `opus`. Reviewer: `nz-reviewer`, model `opus`,
  not yet dispatched.
- Method: bounded ticket, no locked tests.

The rename work flagged at setup is committed (`f754bbf`). Worktrees branch
from a commit, so the lead checks `git status --short` before each dispatch
and stops if source files are dirty.

## Tickets

No board, so tickets live here. Status values: Ready, In flight, Ready to
integrate, Done.

### NZ-1: pull the vault when an interactive session opens

Status: In flight. Queued by Andreas on 2026-10-06 and authorized by him the
same day in the lead session ("you may start"). The ticket stops at Ready to
integrate. Base, branch, worktree and model are under In flight above.

Outcome: `notez tree` (and bare `notez`), `notez todo` with no item and
`notez edit` sync the vault before they open, matching Pinz (`~/Repos/pinz`,
`crates/pinz-tui/src/main.rs` around the "Pull before loading" comment and
`crates/pinz-core/src/sync.rs`). Auto sync on exit already exists
(`notez_core::sync::auto_sync`, commit `1faa10f`).

Acceptance criteria:

1. Before the session opens, pending vault changes are committed, then
   `git pull --rebase` runs against `~/notez` only.
2. Offline, no upstream or a non-repo root is silent and opens normally.
3. A conflict aborts the rebase, leaves no `rebase-merge` or `rebase-apply`
   directory, opens the local vault, and shows the warning in the TUI footer
   (stderr is wiped by the alternate screen), then repeats on stderr after the
   terminal is restored.
4. If the pull stopped, the exit sync does not push over it, as in Pinz
   (`may_push`).
5. `--no-sync` skips both the open and exit sync.
6. `add`, `quick`, `log`, `logz` and `todo "item"` do not pull on open.
7. Tests cover a pull picking up a remote change, the conflict case, and the
   no-upstream case, using a real bare remote like the existing `auto_sync`
   tests. README updated.

Allowed files: `crates/notez-core/src/sync.rs`,
`crates/notez-cli/src/main.rs`, `crates/notez-cli/src/commands/` (tree, todo,
edit entry points), and the TUI footer code in `crates/notez-cli/src/tui/`
(tree and todo). `README.md`. No new dependency, no change to file formats.

Notes: touches `tui/tree.rs` for the footer warning, so it must not run in
parallel with any other ticket on that file. Look and feel of the footer
warning needs Andreas to try the branch build.

## Decisions and open questions

Direction from Andreas (2026-10-06): update the tree browser and todo board UI
a little and add some functionality, taking fleetz (`~/Repos/fleetz`, see
`src/ui/` and its README keybindings table) as the reference. Areas he chose:

1. **Footer hints and help overlay.** Context-sensitive footer whose keys
   light up while a mode is on; a help overlay that lists every key per view.
2. **Layout and panes.** Resizable list/preview split (`<` `>`, drag the
   border), fold the preview, pane focus with Tab or number keys.
3. **Status and indicators.** A header status segment (sync state, dirty
   vault, note count) and a loading spinner.
4. **Search and filtering.** Fuzzy ranking in the style of fleetz `fuzzy.rs`,
   `#tag` filters with `0` to clear, scope cycling.

Constraints for every ticket:

- `notez-core` file formats must stay compatible with epoz, which pins it by
  git rev. UI work belongs in `crates/notez-cli/src/tui/`.
- No new dependency without Andreas's approval (the CLAUDE.md rule).
- Look and interaction need Andreas to try the branch build first, so these
  tickets stop at Ready to integrate.
- The tree browser lives in one 1300-line file, `tui/tree.rs`. Parallel
  workers on it will collide; keep one worker on that file at a time.

Open: which of the four areas to run first, and in what order. Suggested:
footer and help first (smallest, sets the pattern), then status, then layout,
then search.

## Next step

NZ-1: wait for the worker's report, dispatch `nz-reviewer` on the exact diff
as a separate invocation, re-review any later change, then run the checks on
the result against current `main` and stop at Ready to integrate with commit
and merge commands for Andreas. He tries the branch build for the footer
warning before accepting.

UI areas: the lead drafts briefs as Draft tickets in this file. Andreas
approves the briefs and names which to run. They all touch `tui/tree.rs`, so
they run one at a time and only after NZ-1 is integrated.

A lead resuming from this file checks first whether the NZ-1 worker is still
running and whether the worktree holds uncommitted work, and does not
dispatch a second worker onto it.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
