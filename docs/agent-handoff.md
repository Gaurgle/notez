# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. There is no board; this file holds
ticket status, in-flight work, decisions and the next authorized step.

## Current lead

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`aa27d2ed-a3e9-498c-ae61-db84f43eae7c`, took the baton on 2026-10-06 15:01
CEST. First lead since setup: no previous lead, no ticket worktrees or
branches, no workers running at takeover.

PAUSED at 17:20 CEST on 2026-10-06 at Andreas's request ("i must go now,
can we paus now and continue when i get bakc home?"). Baton RELEASED at the
pause. This session dispatches nothing more until Andreas says to continue;
if it is still open then, it retakes the baton and records the time here.
If it is gone, a new lead takes over from this file after Andreas confirms.
Read In flight first: one worker was still running at the pause.

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
- 2026-10-06, Andreas, in the lead session (`aa27d2ed`), answering the lead's
  three NZ-1 questions at Ready to integrate:
  - "Offline message: yes." to "Silencing it means changing `auto_sync`, so
    it would be a new ticket. Do you want one?": create that ticket (NZ-6).
    The lead reads this as wanting the ticket, not as an instruction to run
    it; it cannot start before NZ-1 is merged anyway, and the lead confirms
    with Andreas then.
  - "Yes" to "Should the tree keep one [a quit hint during the warning]
    too?": do it, as part of NZ-1.
  - "3. if you recommend." on pinning `core.hooksPath` in the abort-failure
    test: the lead recommends it, so it is done as part of NZ-1.
  Limit: these two NZ-1 changes only. NZ-1 still stops at Ready to integrate.
- 2026-10-06, before the 15:51 ticket commit, Andreas, in the lead session (`aa27d2ed`),
  sent while the lead was working: "looks good to me. ship it and instal".
  The lead's reading, put back to him at the time:
  - "looks good to me": he accepts NZ-1's look. This is his acceptance of
    the footer, which the ticket required.
  - "ship it": under his own rules (`~/claude-config/rules/shipping.md` and
    this repo's `CLAUDE.md`) this authorizes committing and pushing NZ-1,
    and never a merge. Getting the ticket branch onto `main` takes
    `git merge --no-ff`, and no integration delegation is recorded here, so
    the lead asked him for an explicit merge instruction.
  - "instal": not something a lead may do. Agents never run `./install.sh`
    (`docs/agent-workflow.md`); Andreas runs it.
  On this instruction alone the lead committed the reviewed diff on the
  ticket branch (`922cde2`) and nothing else: no merge, no push, no install.
- 2026-10-06, a few minutes later, Andreas, in the lead session: "start by
  cleaning upp the disk, im guessing there qiute a few rust caches-bianries
  to clean up". Read as: remove Rust build output. The lead ran `cargo
  clean` in six projects and touched nothing else (details under In
  flight). One-off, not a standing permission to delete.
- 2026-10-06, shortly before the 16:15 merge, Andreas, in the lead session (`aa27d2ed`):
  "you can commit and ship it". This answered a message in which the lead
  had laid out the exact finishing sequence for NZ-1 (`git merge --no-ff
  feat/NZ-1-pull-on-open` into `main`, build and test on `main`, `git push
  origin main`), said that sequence waited on his word, and offered to
  commit the handoff. The lead's reading: commit `docs/agent-handoff.md`,
  and run that sequence for NZ-1. Limits: NZ-1 only, this once. It is not a
  standing integration delegation; later tickets still stop at Ready to
  integrate. It does not cover installing, tagging or releasing.
- 2026-10-06 16:29 CEST, Andreas, in the lead session (`aa27d2ed`): "run
  NZ-2 through NZ-5". STANDING SCOPE. It answered the lead's offer: "say
  'work through NZ-2 to NZ-5' and I'll run them in order with my
  recommended answers to the open questions (no spinner, tree keeps its
  order when searching, `s` cycles scope in the tree only, split size not
  remembered), still stopping at Ready to integrate on each for you to
  try", together with the NZ-2 defaults in the same message (help closes
  with `?` or `Esc`; no up-front refactor of the event loops).
  - Scope: NZ-2, NZ-3, NZ-4, NZ-5 as briefed under Tickets, in that order,
    one at a time, with those answers.
  - Limits: no commit, push or merge of ticket code. Each ticket stops at
    Ready to integrate and Andreas tries the branch build. The next ticket
    starts only after the previous one is on `main`, since each is based on
    it. No new dependency, no `notez-core` public API or file format change,
    no persisted state file. NZ-6 and the NZ-1 leftovers are not in scope.
  - Stop and ask when: a ticket needs something outside those limits; two
    fix cycles fail on the same blocker; the disk fills; the lead reaches
    its usage limit; or Andreas changes direction after trying a build.
- 2026-10-06 16:30 CEST, Andreas, in the lead session, sent right after
  "run NZ-2 through NZ-5": "then commit and push". The lead's reading,
  stated back to him the same turn: (a) commit and push
  `docs/agent-handoff.md` now; (b) for NZ-2 to NZ-5, once a ticket is
  accepted by review and passes the lead's checks, the lead commits exactly
  the reviewed diff on that ticket's branch and pushes the branch to
  `origin`. This replaces two limits in the standing scope above: the lead
  may now commit ticket code, and the next ticket may start from the
  previous ticket's reviewed commit instead of waiting for it to reach
  `main`. It does NOT cover merging into `main`: by Andreas's shipping
  rules a merge needs its own explicit request, he has not tried any of
  these builds, and their look is his to accept. The lead asked him
  whether he wants each ticket merged as it passes; until he says so, the
  tickets stop at Ready to integrate on pushed branches.
- 2026-10-06 16:35 CEST, Andreas, in the lead session (`aa27d2ed`),
  answering the lead's question whether NZ-2 to NZ-5 should wait on
  branches or be merged as they pass: "i only need to test at bigger
  changes, we can focus on building right now, you may ship changes. merge
  as you go along." INTEGRATION DELEGATION for the standing scope:
  - The lead may commit each of NZ-2, NZ-3, NZ-4 and NZ-5 on its branch,
    merge it into `main` with `git merge --no-ff`, and push `main`, and may
    commit and push `docs/agent-handoff.md` as the work moves.
  - Andreas waived trying each branch build first; he tests at bigger
    changes. The other safe-merge conditions in `docs/agent-workflow.md`
    still apply to every merge: a separate reviewer accepted the exact diff
    that is committed; `cargo build --workspace` and `cargo test
    --workspace` pass on the combined result; no dependency, CI,
    persistence format or external contract change. If one fails or is
    unclear, the ticket stops at Ready to integrate and the lead asks.
  - Limits: these four tickets only. Not NZ-6, not the NZ-1 leftovers, not
    any later ticket. No install, tag, release, force-push or deletion of
    remote branches. It ends when NZ-5 is on `main` or Andreas says stop.
  - This supersedes the "branches only" reading in the entry above: ticket
    branches are merged, not pushed to `origin` on their own.
- 2026-10-06 16:48 CEST, Andreas, in the lead session (`aa27d2ed`), after
  the lead put two design questions to him on the command line and on
  creating notes in the browser: "i think you can decide on these, and
  I'll test it out." With it he reported that `notez` in `~/Repos/notez`
  prints "No notes here." instead of opening the TUI. The lead's reading:
  the lead decides the design and builds it, as NZ-7 and NZ-8 (see
  Tickets). They join the standing scope and run in this order: NZ-2,
  NZ-7, NZ-8, NZ-3, NZ-4, NZ-5. The integration delegation above ("you may
  ship changes. merge as you go along") is applied to NZ-7 and NZ-8 too, on
  the reading that he can only test them once they are on `main`; the lead
  told him so in the same turn. Same limits and stop conditions.
- No other ticket execution is authorized. Andreas names which tickets run.

## In flight

NZ-7, dispatched 2026-10-06 17:14 CEST under the standing scope:

- Base commit: `b08e798` (`main` and `origin/main` at dispatch, the NZ-2
  merge).
- Branch: `feat/NZ-7-always-open-browser`.
- Worktree: `/Users/at-a/Repos/notez/.claude/worktrees/NZ-7`.
- Method: bounded ticket, one worker pass, no locked tests.
- Worker: `nz-worker`, model `opus`. It was still running at the 17:20
  pause; the lead let it finish. It REPORTED at 17:53 CEST and is stopped,
  so the worktree holds a complete worker result, not partial edits. No
  agent is running.
- Worker's claims, not yet reviewed or checked by the lead: all seven
  criteria satisfied (criterion 6 by reading the code, not by a test); the
  lead's reading of the cause confirmed by a failing test against the base;
  18 new tests; build clean, tests green (notez-cli 155, notez-core 143);
  no existing test changed. Usage: about 86k tokens.
- How it was done: an optional positional `words: Vec<String>` on `Cli`
  beside the optional subcommand; a pure `decide(...)` in `main.rs` picks
  browse, quick note, subcommand or the hint; `commands/tree.rs` chooses
  the view through a `View { Project, Global, Only(Scope) }` enum; all
  browser paths go through one `browse()`.
- Uncommitted change in the worktree: 5 files (`DESIGN.md`, `README.md`,
  `crates/notez-cli/src/cli/mod.rs`, `crates/notez-cli/src/commands/tree.rs`,
  `crates/notez-cli/src/main.rs`), 455 insertions, 54 deletions, nothing
  untracked. `git diff b08e798 | shasum -a 256` gives
  `60f92fa6f9ebb7552908af6cd481f6735596e3a76b07d84de30136844af67b40`.
- Points for the reviewer and for Andreas, from the worker: a mistyped
  subcommand (`notez tre`) now gets the quick-note hint instead of clap's
  "did you mean"; `notez -g -n foo` now runs nav where it was a parse
  error; project detection and the decision run before the config loads;
  the `Commands::Tree | Treez` match arm is now unreachable but kept;
  flags typed after the words count (`notez call the bank -g`); a word
  starting with a dash needs `--`.
- NEXT, on resume and not before: dispatch `nz-reviewer` on that hash with
  the NZ-7 ticket, then the lead's checks, commit, merge and push under the
  recorded delegation.
- Nothing of NZ-7 is committed, merged or pushed.

Queued behind it and not started, in this order: NZ-8, NZ-3, NZ-4, NZ-5.

Worktrees branch from a commit, so the lead checks `git status --short`
before each dispatch and stops if source files are dirty.

Machine note: the data volume was full on 2026-10-06 (228 GB, about 1 GB
free), which failed one agent run with `ENOSPC`. On Andreas's instruction
the lead ran `cargo clean` in six projects (notez, fleetz, pinz, zalary,
`RustroverProjects/Learn Rust`, `Repos/Rust/rustfinity/mutable-variables`),
about 13.5 GB, leaving 11 GB free after NZ-1. Rust build output was a small
part of the 181 GB in use, so it can fill again. A lead that sees `ENOSPC`
stops and asks; it does not delete anything on its own.

## Tickets

No board, so tickets live here. Status values: Draft (brief written, not yet
approved by Andreas), Ready, In flight, Ready to integrate, Done.

### NZ-1: pull the vault when an interactive session opens

Status: Done. Merged into `main` as `8160295` at 16:15 CEST on 2026-10-06
and pushed. Andreas ran `./install.sh` at 16:25; the installed binary is
that build (times here are from `git log` and the binary's timestamp).

Record:

- Authorized by Andreas ("you may start"), look accepted by him ("looks
  good to me"), integration on his "you can commit and ship it" (see
  Authorized by the owner).
- Base `85300ef`, branch `feat/NZ-1-pull-on-open`, ticket commit `922cde2`
  (8 files, 604 insertions, 88 deletions), merge commit `8160295` made with
  `git merge --no-ff`. To undo the ticket: `git revert -m 1 8160295`.
- Agents: `nz-worker` (opus) for the implementation and for fix cycle 1;
  `nz-small` (sonnet) for the two owner-requested changes and for one
  correction; four separate `nz-reviewer` (opus) invocations. About 427k
  agent tokens and 30 minutes of agent time in total, nine runs, one of
  which failed on the full disk.
- Reviews: review 1 requested changes (the tree warning was cleared by the
  first key press). Review 2 accepted the whole change. Andreas then asked
  for a quit hint beside the tree warning and a `core.hooksPath` pin in one
  test. Review 3 accepted that delta except a one-column misplacement of
  the hint, which came from the lead's brief. Review 4 accepted the fix.
  The committed diff hashes to
  `bb801f7c40a7e6d56210acf6bd1e31fe6175b65cdfa79405292ceb37392a7299`
  (`git diff 85300ef 922cde2 | shasum -a 256`), the hash review 4 accepted.
- Lead verification: `cargo build --workspace` and `cargo test --workspace`
  on `main` after the merge, from a clean `target/`: build clean, notez-cli
  99 passed, notez-core 143 passed, doc tests 0. Code on `main` is identical
  to `922cde2`.
- Scope note: the lead widened the allowed files by one item, the
  `--no-sync` doc string in `crates/notez-cli/src/cli/mod.rs`.
- Cleanup done after the push: worktree `.claude/worktrees/NZ-1` removed and
  the local branch deleted with `git branch -d`. The branch was never pushed.
- Left for Andreas to decide as later tickets, none authorized: (1)
  `warning_layout` in `tui/tree.rs` counts chars, not display columns, so a
  warning quoting a path with wide characters (CJK, emoji) can clip the quit
  hint; a fix would measure with ratatui's `Span::width()`. (2) A cosmetic
  test message in `warning_footer_quit_hint_lines_up_with_the_normal_footer`
  says "chars" but prints a byte length. (3) `commit_pending` now also runs
  on open, so a stray untracked file or an in-progress merge in `~/notez`
  is committed earlier than before; same behaviour as the exit sync and
  Pinz. (4) The offline exit message is NZ-6.

The ticket as it was run:

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

### NZ-6: quiet exit sync when the remote is unreachable

Status: Ready. Requested by Andreas on 2026-10-06 ("Offline message: yes").
Not authorized to run yet: Andreas asked for the ticket, not for it to be
run. NZ-1 is merged, so it is unblocked and would be based on current
`main`. The lead confirms with Andreas first. Stops at Ready to integrate.

Problem: after NZ-1 the open pull is silent offline, but the exit sync
(`notez_core::sync::auto_sync`, from `1faa10f`) still ends the session with
`notez: sync stopped (git pull --rebase failed: ...)` when the remote cannot
be reached.

Outcome: ending a session offline is quiet, as in Pinz, where a failed
fetch is idle (`~/Repos/pinz/crates/pinz-core/src/sync.rs`, the "stop rather
than guess" module doc and `fetch`).

Acceptance criteria:

1. With the remote unreachable at session end, pending vault changes are
   still committed locally, nothing is printed and the exit code is
   unchanged.
2. The next session that can reach the remote pushes those commits.
3. A conflict, a failed commit and a push rejected by a reachable remote
   are still reported exactly as today.
4. The explicit `notez sync` command still says so when it cannot reach the
   remote; only the automatic exit sync goes quiet.
5. Tests use a real bare remote made unreachable, like the existing sync
   tests, and cover criteria 1 to 4. README updated.

Allowed files: `crates/notez-core/src/sync.rs`,
`crates/notez-cli/src/main.rs`, `crates/notez-cli/src/commands/sync.rs`,
`README.md`. No new dependency. Existing public `notez-core` signatures and
the `AutoSync` variants stay as they are unless Andreas approves a change
(epoz pins `notez-core` by rev).

Open for Andreas, to settle before it runs: git does not tell "offline"
from "credentials expired" without parsing its error text. Following Pinz,
every failed fetch would be quiet, so an expired credential would also be
quiet until something shows sync state on screen (NZ-3's header would).
Options: (a) fully quiet like Pinz, the lead's recommendation if NZ-3 is
going ahead; (b) one short line such as "notez: offline, changes kept
locally" instead of git's error.

### NZ-7: the main commands always open the browser; quick notes by flag

Status: In flight (see In flight above). Authorized by Andreas on
2026-10-06 16:48 CEST ("i think you can decide on these, and I'll test it
out"), decisions by the lead. Runs before NZ-8 and NZ-3 to NZ-5.

Problem, reported by Andreas: `notez` inside `~/Repos/notez` prints "No
notes here." and exits instead of opening the browser. Cause (checked at
`af345a5`): with no scope flag inside a project, `Scope::from_flags` gives
`Scope::Public`, and `commands/tree.rs` `build_view` then shows only
`<project>/notez/`, which is empty there. The view its own comment calls the
"Default view: every scope of the current project" sits under
`Scope::Personal`, so it is reachable only with `-p`. That project has
notes in `~/notez/personal/notez/` which the bare command never shows.
Then `run` prints and returns when every section is empty.

Outcome and decisions:

1. No scope flag, no subcommand (`notez`), and `notez tree` / `treez` with
   no scope flag: inside a project, open the project view (personal,
   public, docs and local sections of that project). Outside a project,
   the global view, as today.
2. A scope flag and no title keeps opening the browser, narrowed to that
   scope: `-g` the global view as today, `-p` the project's personal notes
   only, `-l` the project's scratch notes only.
3. A scope flag followed by free text creates a quick note in that scope:
   `notez -g call the bank` behaves exactly like `notez -g quick call the
   bank` does today (same folder, same file naming, same editor and sync
   behaviour). A first word that is a subcommand name still runs that
   subcommand; `quick` remains the way to title a note with such a word.
4. Free text with no scope flag is not a quick note: `notez somthing`
   stays an error, and the error names `notez quick <title>` and the flag
   form, so a mistyped subcommand cannot silently create a note.
5. The truly empty case (no notes in the resolved view) is NZ-8's, which
   opens the browser with an empty state. Until then the message stays.

Acceptance criteria:

1. In a project with personal notes and an empty `notez/`, bare `notez`,
   `notez tree` and `treez` open the browser showing those notes.
2. `-g`, `-p`, `-l` with no title open the browser on the views in
   decision 2.
3. `notez -g <words>`, `notez -p <words>` and `notez -l <words>` create
   the same note as the matching `quick` command; a test compares the two
   paths for each flag.
4. `notez <unknown words>` with no flag exits non-zero with the hint in
   decision 4 and creates nothing.
5. Existing subcommands and the argv-0 aliases (`todoz`, `znote`, `treez`
   and the rest) behave as before. `Scope::from_flags` in `notez-core` is
   not changed; "no flag given" is decided in the CLI.
6. The vault pull on open and the exit sync apply to the browser paths as
   now, and a quick note created by flag syncs as `quick` does.
7. Tests cover criteria 1 to 5 without touching the real HOME or vault.
   `README.md`, `DESIGN.md` (scope flags section), the `print_help` text
   and the comment above the "No subcommand" branch are updated.

Allowed files: `crates/notez-cli/src/main.rs`,
`crates/notez-cli/src/cli/mod.rs`, `crates/notez-cli/src/commands/tree.rs`,
`crates/notez-cli/src/commands/add.rs`, `README.md`, `DESIGN.md`. Nothing
under `crates/notez-cli/src/tui/`. No new dependency, no `notez-core`
change.

### NZ-8: create a note from the tree browser

Status: Ready. Authorized with NZ-7, decisions by the lead. Runs after
NZ-7. Needs NZ-2's footer and help tables.

Outcome and decisions:

1. `n` in the tree browser starts a new note. The footer prompt names the
   target before anything is created, for example
   `new note in personal/ideas: _`.
2. The target is the folder under the cursor (the parent folder when the
   cursor is on a file), in that row's section, so the scope (personal,
   public, local, global) follows where the cursor is.
3. `Tab` in the prompt cycles the scope through the ones that apply
   (inside a project: public, personal, local, global; outside: global),
   targeting that scope's root, so a note can be made private or public
   even when that section is empty or not on screen. The prompt shows the
   scope by name.
4. `Enter` creates the note through the same code path as `notez add`, so
   file naming, numbering and any template are identical, then opens it in
   the editor as `add` does, and on return the tree is rebuilt with the
   new note selected. `Esc` cancels and creates nothing. An empty title
   becomes "untitled", as with `add`.
5. The browser opens even when the view has no notes, with an empty state
   line that names `n`. The "No notes here." exit is removed.
6. `n` and the prompt keys are in the key table, so the footer and the
   help overlay show them.

Acceptance criteria:

1. Creating with the cursor in each section writes the file where the
   matching `notez add` with that scope flag would, checked by tests on
   the target resolution (pure function: cursor row and cycled scope in,
   target directory and scope out).
2. `Tab` cycling, `Esc`, the empty title and a name collision behave as in
   decisions 3 and 4; a collision follows whatever `add` does and never
   overwrites.
3. An empty vault or project opens the browser; no key panics on an empty
   tree (navigation, open, tags, rename, filter, focus, view all, help).
4. After creation the new note is selected and visible, tags and expanded
   state of the rest of the tree are kept.
5. The public scope writes into the project repository
   (`<project>/notez/`), which is committed with the project and may be a
   public repo: the prompt must say "public" in so many words when that is
   the target.
6. Unit tests for target resolution, scope cycling and the empty tree;
   README updated.

Allowed files: `crates/notez-cli/src/tui/` (`tree.rs`, `footer.rs`,
`help.rs`), `crates/notez-cli/src/commands/tree.rs`,
`crates/notez-cli/src/commands/add.rs` (to expose the creation path, no
behaviour change to `add`), `README.md`. No new dependency, no
`notez-core` change unless Andreas approves.

### UI tickets NZ-2 to NZ-5 (drafts)

Drafted by the lead on 2026-10-06 from Andreas's direction below, and
authorized by him the same day as a standing scope ("run NZ-2 through
NZ-5", see Authorized by the owner). The section title keeps the word
drafts for history; the status line on each ticket is current.

Answers to the open questions, as accepted with that instruction. They
override the "Open for Andreas" paragraphs on the tickets:

- NZ-2: help closes with `?` or `Esc` only. No up-front refactor ticket.
- NZ-3: no loading spinner; it stays out until Andreas asks for it.
- NZ-4: the split and fold are not remembered between runs. The todo board
  is left alone.
- NZ-5: the tree keeps its order while searching and only the matching
  becomes fuzzy. `s` cycles scope, in the tree only. Not stated by Andreas
  and assumed by the lead: every subsequence match is shown, with no
  minimum score, for him to judge from the branch build.

What the code looks like today (surveyed at `85300ef`), which shapes all four:

- `tui/tree.rs` (1449 lines) and `tui/todo.rs` (1758 lines) each hold one
  `event_loop` of roughly 800 and 1300 lines, with state in local variables
  and drawing inline. Each has its own one-line footer and its own static
  `render_help`. Nothing is shared between them except `tui/text.rs`,
  `tui/tags.rs`, `tui/theme.rs` and `VimCommandMode` in `tui/mod.rs`.
- The tree is a fixed 50/50 list and preview split with no header row. The
  todo board is a single pane.
- Filtering is `notez_core::filter` (text tokens AND `#tag` sets) and the
  text match is a plain case-insensitive substring. Digits `1` to `5` are
  bound only inside tag mode, so `0` and the digits are free in normal mode.
- The loop blocks on `event::read()`, so nothing can animate while idle.

Shared rules for NZ-2 to NZ-5, on top of the constraints listed under
Decisions: one at a time, in order, each based on `main` after the previous
one is integrated; no new dependency; no change to `notez-core` public API or
file formats; existing keys keep their meaning unless the brief says
otherwise; every new piece of logic that can be tested without a terminal
(hint selection, split maths, scoring, hit testing) gets unit tests; README
key tables updated; each stops at Ready to integrate for Andreas to try with
`cargo run --release --manifest-path <worktree>/Cargo.toml -p notez-cli --
tree` (or `-- todo`).

Each ticket extracts only the piece it touches into its own module. There is
no up-front rewrite of the two event loops; if Andreas would rather have one
behaviour-preserving refactor ticket first, it goes before NZ-2.

#### NZ-2: context footer and per-view help overlay

Status: Done. Merged into `main` as `b08e798` at 17:12 CEST on 2026-10-06
and pushed. Not yet installed or tried by Andreas.

Record:

- Base `3864f48`, branch `feat/NZ-2-footer-help`, ticket commit `43ea4a2`
  (6 files, 1608 insertions, 271 deletions), merge commit `b08e798` made
  with `git merge --no-ff` under the integration delegation. To undo the
  ticket: `git revert -m 1 b08e798`.
- Agents: three `nz-worker` (opus) runs (pass 1 shared modules and tree,
  pass 2 todo board and README, fix cycle 1) and two separate
  `nz-reviewer` (opus) invocations. About 441k agent tokens and 38 minutes
  of agent time.
- Reviews: review 1 requested changes. B1, the tree drew no key hints in
  tag, rename and `:` modes (from the lead's pass 1 brief). B2, `Esc` in
  the `:` command line fell through and quit the view or cleared the
  filter, in both views, a defect older than the ticket that the new help
  text contradicted. Review 2 accepted the corrected diff with no
  blockers, after rendering every footer path at widths 0 to 200. The
  committed diff hashes to
  `4c905f9813f1c2b622e895914124db6cc856d8b3244178a0e14f99ce648244f9`
  (`git diff 3864f48 43ea4a2 | shasum -a 256`), the hash review 2 accepted.
- Lead verification: `cargo build --workspace` and `cargo test --workspace`
  in the worktree and again on `main` after the merge: build clean,
  notez-cli 137 passed, notez-core 143 passed, doc tests 0. Code on `main`
  is identical to `43ea4a2`.
- Deliberate behaviour changes: help closes only with `?` or `Esc`
  (Andreas's decision); a key pressed while the `:` command line is active
  is consumed by it, so `Esc` there only closes the line (the lead's
  decision on B2, Andreas said "sure").
- What exists now for later tickets: `tui/footer.rs` (`KeyHint` tables,
  `Mode`, `Toggle`, `select`, `line`, `status_line` for a lead plus hints
  plus a right part, `quit_column`, `QUIT_HINT_RESERVED_COLS`),
  `tui/help.rs` (`HelpState`, `rows`, `render`), `VimKey` in `tui/mod.rs`,
  and the tables `TREE_KEYS` and `TODO_KEYS`. A new key goes into the
  view's table and then shows in both the footer and the help.
- For Andreas to judge when he tries it: a lit key turns both the key and
  its word green; the footer now shows `/ filter` and `? help`, so narrow
  terminals drop `rename` and `view all` first in the tree; the help box
  is about 78 columns wide; the tag legend is wider than 50 columns, so
  no hints fit after it on a narrow terminal.
- Left alone, none authorized: mouse clicks and the wheel still act on the
  view under an open help overlay (as before the ticket); the tree's
  warning line still hardcodes its quit hint spans instead of using
  `footer::hint_spans`.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.

The ticket as it was run:

Outcome: the tree and the todo board share one footer and one help overlay
implementation, modelled on fleetz `src/ui/footer.rs` and `src/ui/help.rs`.

Acceptance criteria:

1. New `tui/footer.rs` and `tui/help.rs` are used by both views; the two
   private `render_help` functions and the inline footer span lists are gone.
2. The footer shows the keys that apply in the current mode (normal, filter,
   tag, rename, focus, vim command) instead of one fixed line.
3. A key whose action has an on state is lit while it is on: `f` while a
   section is focused, `/` while a filter is active, `t` in tag mode, `v`
   while everything is expanded, `?` while help is open.
4. Status messages and the NZ-1 sync warning still take the footer line and
   are not lost when hints change.
5. Hints that do not fit the width drop from the low-priority end; `?` and
   `q` always stay.
6. The help overlay lists every key of the current view, grouped (navigate,
   edit, filter, view), built from the same key table as the footer so the
   two cannot disagree. It scrolls when taller than the terminal.
7. `?` and `Esc` close help. Other keys no longer close it.
8. Unit tests cover hint selection per mode, the lit state, and width
   truncation.

Allowed files: `crates/notez-cli/src/tui/` (`footer.rs` and `help.rs` new,
`mod.rs`, `tree.rs`, `todo.rs`, `theme.rs`), `README.md`.

Open for Andreas: criterion 7 changes today's "press any key to close". Keep
the old behaviour instead?

#### NZ-3: header status segment

Status: Draft. Depends on NZ-1 (it displays the open-sync result).

Outcome: a one-line header above the panes in both views showing sync state,
dirty vault and counts.

Acceptance criteria:

1. Header shows the view name and scope on the left, and on the right: sync
   state (synced, offline or no upstream, pull stopped by a conflict, sync
   off with `--no-sync`), a dirty marker with the number of uncommitted
   vault files, and the note count (tree) or open and done counts (todo).
2. Sync state comes from the NZ-1 open-sync result; the header does not run
   its own network call.
3. Dirty and count values refresh after an action that changes them (edit,
   rename, tag, check) and never on every keypress if that costs a git call.
4. A non-repo vault shows no sync or dirty segment and no error.
5. Narrow terminals drop segments from the right-hand group in a fixed
   order; the header never wraps.
6. Unit tests cover segment selection per sync state and width truncation.

Allowed files: `crates/notez-cli/src/tui/` (`header.rs` new, `mod.rs`,
`tree.rs`, `todo.rs`, `theme.rs`), `crates/notez-cli/src/commands/tree.rs`
and `todo.rs` to pass the sync result in, `README.md`. A read-only dirty
count helper may be added to `crates/notez-core/src/sync.rs` (addition only).

Open for Andreas: the loading spinner. Today the pull finishes before the
TUI opens, so there is nothing to spin over. A real spinner means opening the
TUI at once, pulling on a background thread, polling events with a timeout
and reloading the tree when the pull lands. That is a concurrency change to
both loops and to NZ-1's flow, so the lead recommends leaving it out of NZ-3
and deciding on it as its own ticket once NZ-1 is in use.

#### NZ-4: resizable split, preview fold and pane focus (tree only)

Status: Draft.

Outcome: the tree browser's list and preview behave like fleetz panes.

Acceptance criteria:

1. `<` and `>` narrow and widen the list; `=` resets to 50/50. The split is
   clamped so neither pane becomes unusable.
2. Dragging the border between the panes resizes it; a grip mark sits on the
   border and lights while dragging, as in fleetz `src/ui/grips.rs`.
3. `1` focuses the list and `2` the preview; `Tab` cycles. The focused pane
   has a highlighted border and its number in the title.
4. With the preview focused, `j`, `k`, `PgUp` and `PgDn` scroll it. `J` and
   `K` keep scrolling the preview from the list, as today.
5. `2` on the focused preview folds it and the list takes the full width;
   `2` again unfolds it. A folded preview is not read from disk.
6. The mouse wheel scrolls the pane under the cursor. Clicking a pane
   focuses it. Existing clicks (filter dots, rows, tag dots) still work at
   any split width.
7. Below a minimum width the preview folds on its own and unfolds when
   there is room again.
8. Unit tests cover split clamping, border hit testing and the fold rules.

Allowed files: `crates/notez-cli/src/tui/` (`panes.rs` new, `tree.rs`,
`footer.rs`, `help.rs`, `theme.rs`), `README.md`.

Open for Andreas: (a) the split and fold are not remembered between runs in
this brief; remembering them needs a small state file, which is a new
persisted format and would need approval. (b) The todo board is a single
pane, so this ticket leaves it alone. Fine?

#### NZ-5: fuzzy search, tag filter clearing and scope cycling

Status: Draft. The largest of the four and the one with real design choices.

Outcome: `/` finds notes and todos by fuzzy subsequence (`flz` finds
`fleetz`), tag filters clear with one key, and the tree can be cycled
through scopes.

Acceptance criteria:

1. A scoring function ported from fleetz `src/fuzzy.rs` lives in
   `tui/text.rs` with its constants named and unit tested (prefix, word
   boundary and consecutive bonuses; no match returns none).
2. Text tokens in the filter match by subsequence instead of substring, in
   both views. `#tag` tokens and the AND and OR rules of
   `notez_core::filter` are unchanged, and `notez-core` is not edited.
3. Matched characters are highlighted in the rows.
4. `0` in normal mode clears the `#tag` tokens and keeps the free text.
5. A scope key cycles the tree through all, local, project and global, and
   the header or footer names the active scope.
6. An empty result shows a "no matches" row instead of an empty pane.
7. Unit tests cover scoring, ranking order, tag clearing, and that existing
   filter tests still pass unchanged.

Allowed files: `crates/notez-cli/src/tui/` (`text.rs`, `tree.rs`, `todo.rs`,
`footer.rs`, `help.rs`), `README.md`.

Open for Andreas:
(a) Ranking in a tree. fleetz ranks a flat list best match first; the tree
is hierarchical. Either the tree keeps its order and only the matching
changes (smaller, the lead's recommendation), or the tree flattens to a
ranked list of notes while text is in the filter (closer to fleetz, larger).
(b) Subsequence matching is looser than substring and will show more rows
for short queries. Accept, or require a minimum score?
(c) Scope cycling overlaps with `f` (focus section), which already narrows
the tree to one section. Is a separate key wanted, and which one? `s` is
free in the tree.
(d) Does the todo board need scope cycling, or the tree only?

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

New direction from Andreas (2026-10-06, about 16:45 CEST, during NZ-2), not
yet a ticket and not authorized: look over the notez commands. `notez` and
`treez` should always open the TUI; flags (`-g`, `-p`, with or without
`add`) should be the quick-note paths; and the TUI then needs a way to say
whether a note created inside it is private or public. Facts the lead
checked at `af345a5`: bare `notez`, `notez -g`, `notez -p`, `tree` and
`treez` already open the tree browser on the resolved scope
(`crates/notez-cli/src/main.rs`, the "No subcommand" branch, from the
recent `3d0bb7a`); notes are created only by `add`, `znote` and `quick`;
the tree browser has no create key at all. Open with Andreas: what a flag
with no title should do, and how the TUI picks the scope for a new note.
The lead proposed: create in the section under the cursor, with a scope
picker when that is ambiguous.

The four areas are drafted as NZ-2 (footer and help), NZ-3 (status header),
NZ-4 (layout and panes) and NZ-5 (search and filtering) under Tickets, in the
suggested running order. Open: Andreas approves or changes each brief,
answers the questions listed on it, and names which to run.

## Next step

NZ-1 is Done and installed by Andreas. He is trying it in daily use.

PAUSED since 17:20 CEST (see Current lead). On resume, with Andreas's go:
finish NZ-7 from the state under In flight (worker report or leftover
edits, then review, checks, commit, merge, push), then carry on down the
queue. The standing scope and the integration delegation recorded above
still stand for NZ-7, NZ-8, NZ-3, NZ-4 and NZ-5; the pause did not widen or
withdraw them. Andreas installed the NZ-2 build at 17:15 and saw that bare
`notez` in `~/Repos/notez` still says "No notes here."; that is NZ-7's fix
and he knows it has not landed. `notez -p` opens the browser there
meanwhile.

Standing scope: NZ-2 (done), then NZ-7, NZ-8, NZ-3, NZ-4, NZ-5 (see
Authorized by the owner for its limits). Per ticket the lead:

1. Runs the worker passes, then a separate `nz-reviewer` on the whole diff,
   re-reviewing after any change.
2. Runs `cargo build --workspace` and `cargo test --workspace` itself.
3. Commits exactly the reviewed diff on the ticket branch, staging the
   ticket's files by name.
4. Merges it into `main` with `git merge --no-ff`, reruns the checks on
   `main`, and pushes `main` (authorized: "merge as you go along"), provided
   the safe-merge conditions hold. Otherwise it stops at Ready to integrate
   and asks.
5. Marks the ticket Done, removes its worktree and local branch, commits
   and pushes this file, and tells Andreas what changed on screen.
6. Starts the next ticket in a new worktree based on `main`.

A lead resuming from this file first checks whether a worker is still
running in the worktree named under In flight and whether it holds
uncommitted work, and does not dispatch a second worker onto it.

Andreas installs and tries when he chooses (`./install.sh`, his to run).

Waiting on Andreas, none of it blocking the scope above:

- NZ-6 (quiet offline exit): his choice between fully quiet and one short
  line, and the instruction to run it. It touches `main.rs`, `sync.rs` and
  `commands/sync.rs`, so it could run alongside NZ-2, NZ-4 or NZ-5, but
  not NZ-3, which may add a helper to `sync.rs`.
- Whether the leftovers listed in NZ-1's record become tickets.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
