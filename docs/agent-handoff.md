# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. There is no board; this file holds
ticket status, in-flight work, decisions and the next authorized step.

## Current lead

None. Relay was set up on 2026-10-06 and no lead has taken the baton. The
first lead starts with `claude --agent nz-coordinator` and records its
session and takeover time here.

## Authorized by the owner

- 2026-10-06, Andreas, in the setup session: set up Relay in this repo with
  prefix `nz`, checks `cargo build --workspace` and `cargo test --workspace`,
  no board, default models (lead `fable`, worker and reviewer `opus`, small
  `sonnet`). Integration: the lead presents commands; no commit, push or merge
  permission is granted.
- Ticket execution is NOT yet authorized. Andreas named a direction (below)
  but has not named tickets. The lead proposes tickets, Andreas names which
  to run.

## In flight

Nothing.

Base-commit warning: the `notez rename` command and the tree browser `r`
key (added 2026-10-06) were uncommitted at setup time. Worktrees branch from a
commit, so Andreas must commit that work before the first ticket, or workers
will not see it and will conflict with it. The lead checks `git status
--short` and stops if it is dirty.

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

Andreas commits the rename work, starts `claude --agent nz-coordinator`, and
names the first ticket or a standing scope. The lead reports state and
proposes ticket briefs before dispatching anything.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
