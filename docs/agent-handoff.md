# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. The board is the GitHub Project
`https://github.com/users/Gaurgle/projects/2` (owner `Gaurgle`, number 2;
read command and ids in `docs/agent-workflow.md`). This file holds the
ticket briefs and records, in-flight work, decisions and the next
authorized step; the board mirrors each ticket's Status and the lead
keeps it current as tickets move.
Keep it short: finished tickets go to `docs/agent-log.md`.

Finished tickets and earlier leads: `docs/agent-log.md`.

## Current lead

None. Lead `0bafc905` (Claude, `claude-fable-5-1`, `nz-coordinator`)
RELEASED the baton on 2026-10-27 at about 12:58 CET on Andreas's
"find a place to stop for today". Workers and reviewers: none running,
none were started for NZ-43. State at release: `main` = `origin/main`
at `84fa8df` (the docs ship after the wave), CI green; NZ-43 (#71) is
CUT but NOT dispatched: branch `feat/nz-43-todo-preview`, worktree
`.claude/worktrees/NZ-43`, clean at base `84fa8df`, board Ready, the
full worker brief is the body of #71 (the lead's rulings are in its
comments). The 2026-10-27 "start nect ticket" instruction (Authorized
by the owner) covers dispatching NZ-43, so the next lead may dispatch
it from the issue body without a new confirmation, then stop at Ready
to integrate for Andreas's branch-build try. Uncommitted at release:
this file (commit command presented to Andreas).

Earlier that day lead `0bafc905` took
the baton on 2026-10-27 at about 10:45 CET on Andreas's "Take the
baton" (his 2026-10-27 instruction, verbatim: "run NZ-55 (#81), then
NZ-56 (#82), under the standing permissions. NZ-55 first ... NZ-56
stacks on NZ-55's reviewed commit ... Both change how the TUI behaves,
so stop at Ready to integrate and give me the NZ-56 branch build
command so I can try both before anything merges. Report at
milestones only."). On takeover: `main` at `90e8a25` = `origin/main`,
clean, CI green; `scripts/relay-sweep main` removed the stale clean
NZ-55 worktree and branch (base `2e3dee7`, no commits), recut as
`fix/nz-55-tree-scroll` at `.claude/worktrees/NZ-55` from `90e8a25`;
board check passes (53 items); relay-check up to date, handoff over
the 150-line cap (trim at release). No worker or reviewer was running.
State at about 12:40 CET, scope DONE: NZ-55 merged as `03427e6` and
NZ-56 as `17bdf3e`, `main` = `origin/main` at `17bdf3e`, CI green on
both merges, #81 and #82 closed, board Done, remote ticket branches
deleted, sweep removed both worktrees (0 kept), board check passes.
The lead still holds the baton; no worker or reviewer running. Only
this file and `docs/agent-log.md` are uncommitted (docs commit command
presented to Andreas). The 2026-10-27 scope is exhausted; the Next
step queue below (NZ-43 first) stays authorized by the standing scope
but Andreas names when it runs.

Before: lead `b5670714` (Claude, `claude-opus-5-5`, a plain session
started in `~/notez`, NOT `nz-coordinator` on `fable`: Andreas named it
lead in place, see Authorized by the owner, 2026-10-27) took the baton
on 2026-10-27 10:23 CET and RELEASED it at 10:27 CET when Andreas said
"ill open a relay lead". Its NZ-55 worker was stopped before editing
anything; no worker or reviewer is running. The next lead resumes the
2026-10-27 scope below (NZ-55 then NZ-56) without new confirmation. On takeover: `main` at `2e3dee7` =
`origin/main`, clean, CI green; no ticket worktree or branch; no
workers running; sweep 0 removed; board check passes (51 items).

Before that: lead `9c5067bf` (Claude, `claude-fable-5-1`, `nz-coordinator`)
took the baton on 2026-10-26 15:03 CET and RELEASED it at 16:10 CET
on Andreas's "we can stop for today". Workers and reviewers: all
stopped, none running. Nothing runs until Andreas says to continue;
the Next step queue below is already authorized. Open questions left
with him: try the NZ-43 branch build before its merge or keep the
precedent; yes or no to a frontmatter-hiding issue in the preview. The previous lead `8a79e020`
had released it on 2026-10-15 (record in `docs/agent-log.md` under
Earlier leads); no ticket ran in between. On takeover: `main` at
`0019c62` = `origin/main`, clean, CI green; no ticket worktree or
branch; no workers running; board check passes (49 items); the board
matches this file (Ready: NZ-30 #59, NZ-43 #71; the rest Backlog).
State at release: `main` at `c226a6c` = origin, CI green, no ticket
worktree or branch; board check passes (51 items); only
`docs/agent-handoff.md` and `docs/agent-log.md` are uncommitted (the
commit command was presented to Andreas). Next free ticket id: NZ-55. The
handoff was trimmed (parked NZ-18, NZ-28, NZ-29 and the Done NZ-42
records moved to the log); the verbatim "Authorized by the owner"
section keeps it above the 150-line cap.

State: tag `v0.1.0` at `dd2ff6a`; the workspace is 0.2.0 and NOT
tagged (tagging is Andreas's). No ticket worktree or branch; only the
old non-Relay `origin/phase2-subdir-and-stubs` remains.

Lessons from the 2026-10-15 run: a reviewer that restores a mutated
file with its old mtime leaves a stale test binary, so `touch` a source
file before the lead's own verification run; `notez --version` exits 2,
so never put it in a `&&` chain before a push; verify code facts before
writing a brief, or mark them "worker verifies".

## Authorized by the owner

Standing permissions in force, paused while no lead holds the baton.
The verbatim entries from 2026-10-07 to 2026-10-10, the 2026-10-15
working rule update and the one-off tag are in `docs/agent-log.md`
under "Moved from the handoff on 2026-10-17".

- Integration delegation, "merge as you go" (2026-10-07, applied again
  2026-10-09, 2026-10-10 and 2026-10-15): reviewed diff, green branch
  CI, safe-merge checks, then `git merge --no-ff` into `main` and push.
- Branch pushes (2026-10-08 15:15): push the ticket branch once the
  reviewer accepts, delete the remote branch after the merge. No
  force-push, tags or releases; a red run stops the ticket at Ready to
  integrate.
- Board items (2026-10-08 16:35): the lead creates the item for a
  ticket Andreas requests in a lead session; since 2026-10-17 that is
  an issue in `Gaurgle/notez` that follows the board rules. Not for
  tickets the lead invents, never deleting items or changing the
  board's visibility.
- Dependencies (2026-10-08 17:20): `pulldown-cmark` and `tree-sitter`
  with grammar crates, for the preview family only, at pinned versions
  named in each brief.
- Working rule (2026-10-08 20:50, updated 2026-10-15): the lead decides
  open design points itself, records them on the ticket and reports at
  milestones only (a ticket merged, a wave done, a blocker). It stops
  Andreas only for anything irreversible or costing money, deleting or
  moving his files, `notez-core` public API or file format changes,
  dependencies outside an approved family, CI or infrastructure, tags
  and releases, or a ticket outside the recorded scope.
- Parallelism (2026-10-10 14:05): up to three implementation workers
  on disjoint files, at most one per file, merges one at a time.

The live scope is the two 2026-10-15 entries, verbatim:

- 2026-10-15 11:36 and about 12:10 CEST, Andreas, in the lead session
  (`8a79e020`): "check the commits done today regarding notez. then see
  if there's a handover and lets get to it?", then, after the lead's
  product review of the queue ("is this overengineering?"), his
  answers: "i think the inspector DO can look more like a file reader,
  that's ok. hence the toggable view. add integration tests. add
  versioning, changelo. look at notez-core and NZ-17. update DESIGN.md
  (and other docs). NZ-32 is good, i like that. 42: ok? NZ-43 shouldnt
  be parked for too long. Linting we can park, as we dont offer a way
  yet to read, write or inspect actual code. Only .md as for now. no
  need to park 30 'outline' really? we'll look into the storage
  integration, date (altough this is a big and important addition?,
  converning both pinz notes and what could be adjecent to a calendar
  TUI, slack bots and more. shouldb e a integral part of a notezs note
  .md document). i trust you on the rest". The lead's reading, applied:
  - RESUME the standing scope under the 2026-10-08 permissions (the
    "merge as you go" integration delegation, branch pushes with remote
    deletion after the merge, board items for tickets he asks for here,
    the working rule, the three-worker cap).
  - NZ-5 runs TRIMMED (no fleetz scorer port, no ranking: subsequence
    match plus highlight, `0`, `s`, "no matches" row). NZ-32 runs as
    its brief is written. NZ-42 runs narrowed to `/` fuzzy search
    reusing NZ-5's matcher plus reload-on-change. NZ-30 stays in the
    queue (the preview may become a file reader; toggleable).
  - NZ-17 (versioning 0.1.0, changelog, a `notez-core` compatibility
    rule for epoz) is CONFIRMED and runs now. Tagging stays his.
  - NEW TICKETS, board items created under the standing permission:
    NZ-45 storage integration tests (runs now), NZ-46 docs refresh
    (DESIGN.md status, README, stale docs; runs after NZ-5 frees the
    README), NZ-47 dates as a first-class part of a note (DESIGN ONLY:
    the lead writes `docs/design-nz47-dates.md`; no code, and nothing in
    `notez-core` without his approval of the design).
  - PARKED: NZ-28 pass 2 (lint UI) until notez reads non-markdown
    files as notes; NZ-29 closed as "not now". NZ-43 is NOT parked: the
    lead brainstorms it with him in this session once NZ-5 is on
    `main`, before NZ-42 is dispatched.
  - Limits unchanged: no new dependency beyond the recorded approval,
    no `notez-core` public API or file-format change without his yes,
    installing, tags and releases are his, NZ-18 does not run.
- 2026-10-15, later, Andreas, in the lead session (`8a79e020`),
  answering the NZ-43 proposal and the NZ-47 questions: "i think
  right pane should be ON by default. wudth is ok. order is ok. not
  in it: yes for now, open up later on? NZ-47: 2: go with event then
  4: yes 6: yes 7: yes 8: not read only from notez, but warn for
  longer than standard pinz length". The lead's reading, applied:
  - NZ-43 DESIGN APPROVED (bounded path): todoz right pane ON by
    default where the width allows (120 columns or more; below that
    `p` reports too narrow), `p` toggles, NZ-4 resize and fold keys,
    shows the selected task's source `TODO.md` rendered and scrolled
    to the task's highlighted line, footer shows the due date once
    dates exist. Not in it for now (may open later): editing in the
    pane, linked-note previews, calendar views. Runs after NZ-42 and
    NZ-48. Board item `PVTI_lAHOCU842c4BmE5Zzg_uH10`, Ready.
  - NZ-47 decisions: field `event`; frontmatter written only when a
    date is set; nvim-only for dating existing notes at first.
  - NZ-48 AUTHORIZED (NZ-47a): `notez-core` frontmatter and dates,
    read-only, workspace 0.2.0 with a `notez-core` changelog entry.
    This is the `notez-core` public API change the working rule
    reserves for him; his "6: yes" is that approval, for the scope in
    `docs/design-nz47-dates.md` only. Board item
    `PVTI_lAHOCU842c4BmE5ZzhA-tKk`. Brief under Tickets.
  - NZ-18 FOLDED into the dates rollout as NZ-47d: pins are a note
    type read through the same frontmatter parser; NOT read-only:
    they open in the editor like any note; notez never runs git in
    `~/pinz-board`; notez warns when a pin body exceeds a standard pin
    length (threshold to be proposed in that brief; Pinz has none in
    code). Still a design-first ticket; no code until its brief is
    approved. The Pinz-side "keep unknown keys" change is a Pinz
    ticket Andreas owns.
  - Limits unchanged. Also in force since the same day: report at
    milestones only, stop him only for his decisions.
- 2026-10-27 about 10:20 CET, Andreas, in session `b5670714`, after
  asking whether notez should sync from inside the TUI (the session
  proposed one sync key: commit, pull, push) and reporting a tree bug
  (an expanded folder on the bottom row stays out of view; he wants a
  5-row scroll margin and the list to move so the expansion fits):
  "add all these as features and get started then? you're the lead,
  you can plan and get to work?". The lead's reading: take the baton,
  create both tickets (NZ-55 the tree scroll bug, NZ-56 the in-TUI
  sync key) and run them under the standing 2026-10-08 permissions.
  Both are interaction changes, so each stops at Ready to integrate
  until Andreas tries the branch build.
- 2026-10-27 about 12:50 CET, Andreas, in the lead session
  (`0bafc905`), after the NZ-55 and NZ-56 merges and the docs ship:
  "start nect ticket". The lead's reading: run the head of the
  recorded Next step queue, NZ-43 (#71, approved design), under the
  standing permissions; interaction change, so it stops at Ready to
  integrate for his branch-build try. One ticket; the rest of the
  queue waits for his next word.
- No other ticket execution is authorized. Andreas names which tickets run.

## In flight

Wave of 2026-10-27 (tickets cut by lead `b5670714`, run by lead
`0bafc905`): NZ-55 (#81) merged as `03427e6`, `main` CI green; NZ-56
(#82) merged as `17bdf3e`, `main` CI run `40499879883` (see Current
lead for its result). Both accepted by Andreas on the NZ-56 branch
build. Records in `docs/agent-log.md`; follow-ups on #81 and #82.
Remote branches `fix/nz-55-tree-scroll` and `feat/nz-56-tui-sync`
deleted after the merges under the 2026-10-08 branch permission;
worktrees swept.

Wave of 2026-10-26 (lead `9c5067bf`, started 15:20 CET on Andreas's
"i guess you can just get started?", read as the "continue" the
2026-10-15 Next step waited for): NZ-42 (#70) merged as `cd62547` and
NZ-53 (#79, the planned "NZ-49") merged as `c226a6c`, both `main` CI
green, remote branches deleted, worktrees swept. Records in
`docs/agent-log.md`. Owner decision taken that day: Andreas delegated
the `serde_json` dependency call to the lead ("i dont know about
that, can you make that decision?"); the lead decided yes, scoped to
`notez agenda --json`, as NZ-54 (#80, Ready, P2, 0.3.0).

Lessons: verify which file draws a UI element before naming it in a
brief (`tui/tree/input.rs` draws the preview border, not
`render.rs`); `notez-core` `sync::tests::auto_sync_pushes_offline_
commits_once_the_remote_is_reachable` and the highlight/markdown
"bounded time" tests can fail under machine load and pass on rerun.

## Tickets

Ticket briefs and records live here; the board at `projects/2` carries
the same Status per ticket. Status values: Backlog (brief written, not yet
approved by Andreas), Ready, In progress, Review, Ready to integrate, Done.
On 2026-10-17 Andreas had the board moved to the Relay 0.6.0 shape: every
draft became an issue in `Gaurgle/notez` (NZ-1 is #30, NZ-52 is #78; Done
tickets closed), Draft became Backlog, and NZ-30 (#59, P2) and NZ-43 (#71,
P1) went into milestone `0.3.0`. Records below that say Draft or In flight
use the old names.

### NZ-30 (#59): symbol outline for the preview (tree-sitter, in place of LSP)

Status: Ready, runs after NZ-28. Opened by the lead on 2026-10-08 at
21:45 CEST under Andreas's "perhaps 28 & 29?" and the NZ-29 design
note's recommendation. Board item `PVTI_lAHOCU842c4BmE5Zzg_e_xc`.

Outcome: a toggleable outline of the selected file: for markdown the
headings (level, text); for code the top-level definitions (functions,
types, constants, classes, modules) with their kind and name. `Enter` on
an entry scrolls the preview to that line. No process, no dependency.

Decisions: source for code is each grammar's bundled `TAGS_QUERY`
(`tree-sitter-rust 0.24.2` ships `queries/tags.scm` and exports
`TAGS_QUERY`; the worker checks which of the other grammars do and
falls back to a small local query per language for the rest, or to
"no outline" for a language with neither); markdown headings come from
the pulldown events (NZ-25); the outline is an overlay like the help
and NZ-28 issue overlays (same drawing), toggled with a key the worker
picks from the free ones (proposal `O`), `j`/`k` move, `Enter` jumps
and closes, `Esc` closes; the footer shows the key at low priority
when the file has an outline; computed with the preview cache key, no
per-frame cost; files over 1 MiB get no outline.

Acceptance: `outline(text, language) -> Vec<Entry { line, kind, name,
depth }>` tested per language with a small snippet and for markdown
heading levels; overlay navigation tested on its pure state; jump
scrolls to the entry's rendered line (through NZ-28's source line map
for rendered markdown); key table and help tests; README. Allowed
files: `crates/notez-cli/src/tui/outline.rs` (new), `tui/mod.rs`,
`tui/highlight.rs` (expose languages and tags queries), `tui/markdown.rs`,
`tui/tree.rs`, `tui/footer.rs`, `tui/help.rs`, `README.md`. One worker
pass, one review.

### NZ-47 (#75): dates as a first-class part of a note (2026-10-15, Draft, design only)

Board item `PVTI_lAHOCU842c4BmE5ZzhA8UTg`. Andreas, 2026-10-15: dates
are "a big and important addition, concerning both pinz notes and what
could be adjacent to a calendar TUI, slack bots and more. should be an
integral part of a notez note .md document". DESIGN.md already calls
the date encoding "the crux" for todos.

Outcome of this ticket: `docs/design-nz47-dates.md`, written by the
lead, covering: what a date on a note means (created, due, event,
reminder), where it lives in the `.md` (frontmatter for notes, an
inline token for `TODO.md` lines, or one of them for both), how it
round-trips through nvim and the CLI untouched, what `notez-core`
exposes (a parsed field on the note and task models), what the first
consumers are (tree sort and filter, todoz due marker, a `notez due`
or `notez agenda` listing), how Pinz pins and a calendar TUI would
read the same field, and the epoz compatibility rule from NZ-17. Ends
with a recommendation and open questions for Andreas. No code until he
approves; any `notez-core` change needs his explicit yes.

### NZ-51 (#77): TODO.md in the inspector renders like standalone todoz (2026-10-15, Draft)

Asked for by Andreas at the end of 2026-10-15: "todoz in inspector
should have same rendering/look the same as the todoz stand alone.
the graphics differ, i believe." Not investigated yet, on his
instruction ("dont look into it now, add it as a task").

Outcome to confirm when picked up: a `TODO.md` shown in the tree
browser's preview pane draws its rows the way the todoz board does
(checkbox glyphs and states, tag dots and colours, depth indent,
header rows), instead of the generic markdown rendering. Likely
shape: the preview detects a todo file (same rule as NZ-24's icon)
and renders it through the todoz row drawing from `tui/todo.rs`,
extracted into a shared helper if it is not already one. First step
is a short comparison of the two renderings (screenshot or test
buffer) to list the differences. Files, probably: `tui/tree/preview.rs`,
`tui/todo.rs` (extract only), `tui/markdown.rs` (bypass for todo
files). Numbers NZ-49 and NZ-50 stay reserved for the `agenda` and
write-path tickets named in Next step.

### NZ-52 (#78): todoz discovers TODO.md stores the way notez discovers repos (2026-10-15, Draft)

Asked for by Andreas at the end of 2026-10-15: "look into to have
todoz TUI see files, as we worked on today. it does not scan for
(todoz) files as notez scans for notez." Not investigated yet, on
his instruction.

Outcome to confirm when picked up: `todoz` sees every `TODO.md` the
tree browser sees. `7cfeb27` made every notez command auto-register a
repo that contains `notez/` or `.notez/`; the tree browser then lists
all registered projects and their stores. The todo board
(`notez_core::todo::load_board`) builds from the registry and the
global `_todos/`, so the questions are: does it run the same
auto-registration path; does it include each project's personal,
public and scratch `TODO.md` the way the tree does (NZ-13 unified
view); and does it pick up a `TODO.md` in a repo that has no
`notez/` or `.notez/` dir yet (probably not today, and whether it
should is Andreas's call). First step: a short investigation note
listing what todoz loads versus what the tree lists on his machine,
then a brief. Files, probably: `crates/notez-core/src/todo/mod.rs`
(`load_board`), `crates/notez-cli/src/main.rs` (registration hook),
`commands/todo.rs`. A `notez-core` change needs his yes.

### NZ-43 (#71): todoz preview pane (2026-10-15, Ready, design approved)

Approved by Andreas on 2026-10-15 (see Authorized by the owner).
Right pane ON by default at 120 columns or more, `p` toggles, below
that `p` reports too narrow; NZ-4 resize and fold keys; shows the
selected task's source `TODO.md` rendered (`tui/markdown.rs`) and
scrolled to the task's line, highlighted; its footer shows the task's
due date once NZ-48 has landed. Not in it for now: editing in the
pane, linked-note previews, calendar views. Runs after NZ-42 and
NZ-48. Allowed files when cut: `tui/todo.rs`, `tui/panes.rs`,
`tui/footer.rs`, `tui/help.rs`, `README.md`. Brief details (keys,
tests) to be written at dispatch from this record.

## Constraints

- `notez-core` file formats stay compatible with epoz, which pins the
  crate by git rev. UI work belongs in `crates/notez-cli/src/tui/`.
- No new dependency without Andreas's approval, beyond the family above.
- Look and interaction need Andreas to try the branch build first.

## Next step

Updated 2026-10-26 by lead `9c5067bf` after the NZ-42 and NZ-53
merges. Steps 1 and 2 of the 2026-10-15 queue are done; the rest is
still authorized by the standing scope plus the 2026-10-15 entries
and needs no new confirmation. Uncommitted at this point: this file
and `docs/agent-log.md` (the lead presents the docs commit command;
coordination commits are not in the recorded delegation).

1. NZ-43 (#71), todoz preview pane: CUT, not dispatched (branch
   `feat/nz-43-todo-preview`, worktree `.claude/worktrees/NZ-43` at
   `84fa8df`). Dispatch `nz-worker` (opus, medium) with the body of
   #71 as the brief; authorized by the 2026-10-27 "start nect ticket"
   entry. Interaction-heavy: stop at Ready to integrate for Andreas's
   try of the branch build.
2. NZ-30 (#59), symbol outline, on the tree line (`tui/tree/*`,
   `tui/highlight.rs`, `tui/markdown.rs`, `tui/outline.rs` new,
   `README.md`). Can run beside NZ-43 (disjoint files; README is
   shared, so one of them delivers its README text for the lead to
   apply, as NZ-53 did).
3. NZ-54 (#80), `notez agenda --json` with `serde_json` as a direct
   dependency (lead decision under Andreas's delegation, 2026-10-26).
   Small; `nz-small` or `nz-worker`; files `crates/notez-cli/Cargo.toml`,
   `commands/agenda.rs`, `cli/mod.rs`, `README.md`.
4. NZ-50 (create when reached): the write path, todoz due column
   with the `D` prompt and `notez add --due/--on`, under the
   "Requirements for the write path" in the design note.
5. NZ-47d / NZ-18: Pinz as a note type, design brief first (pins
   editable, no git from notez in `~/pinz-board`, length warning
   with a proposed threshold); needs Andreas's approval of the brief
   and the Pinz-side "keep unknown keys" change before code. The
   NZ-18 record is in the log and on #47.
6. Drafts whose queue position is Andreas's call: NZ-51 (#77)
   `TODO.md` in the inspector renders like standalone todoz; NZ-52
   (#78) todoz discovers `TODO.md` stores the way notez discovers
   repos. Both start with a short investigation, not code.
7. Candidate found by the NZ-53 worker, not yet an issue (needs
   Andreas's yes, lead-found): the rendered preview draws a note's
   frontmatter block as a rule plus a paragraph instead of hiding it
   (`tui/markdown.rs`).
8. Follow-ups recorded on #70 (NZ-42) and #79 (NZ-53), none urgent.

Per ticket the procedure is unchanged (see the 2026-10-10 paragraph
below): branch and worktree from current `main`, `nz-worker`, a
separate `nz-reviewer` on the exact diff, re-review after changes,
lead verification (touch a source file first), commit exactly the
reviewed diff, push, green branch CI, `--no-ff` merge, checks on
`main`, push, green `main` CI, Done, cleanup. Report at milestones
only; stop Andreas only for his decisions.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
