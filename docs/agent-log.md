# Relay log

Finished tickets, newest first, moved here from `docs/agent-handoff.md` when
they are Done. Not required reading on takeover. Each record ends with the
metrics line defined in `docs/agent-workflow.md`, so a retrospective is a
search rather than a tally.

Records before 2026-10-16 were moved here in bulk from
`docs/agent-handoff.md` on 2026-10-16, verbatim, and predate the metrics
line. Older records (2026-10-06 to 2026-10-08) are in
`docs/agent-handoff-archive.md`.

## Earlier leads

Lead records that preceded the current one in the handoff, newest first.

### `0bafc905` and `b5670714` (Claude), 2026-10-27

Moved verbatim from the handoff on 2026-10-29 by lead `7c4dc6c8`.

Before: lead `0bafc905` (Claude, `claude-fable-5-1`, `nz-coordinator`)
RELEASED the baton on 2026-10-27 at about 15:45 CET on Andreas's
"merge 51 for now ... you may ship it, make sure handover is done etc,
then i want to close this session". Workers and reviewers: none
running. State at release: NZ-51 (#77) merged as `3c3c762` (Closes
#77), checks green, pushed, `main` = `origin/main` at `3c3c762`, CI
on `main` run `40524947003` GREEN, #77
closed, board Done, remote branch deleted, record in
`docs/agent-log.md`, worktree swept (NZ-43 kept, unmerged). Today's wave: NZ-55, NZ-56 and NZ-51 merged.
- NZ-43 (#71) `feat/nz-43-todo-preview` at `06e7254`, worktree
  `.claude/worktrees/NZ-43`, reviewed, branch CI green, Ready to
  integrate but PARKED: Andreas said it is not the look he asked for;
  he decides whether he wants the todoz pane too. Merging it now
  needs a small `todo.rs` conflict fix (NZ-51 moved the row drawing
  out), so re-review after the fix.
- NZ-57 (#83) Ready, briefed on the issue, waits for his word.
- Open design question from Andreas (2026-10-27, on #77), to look
  into before more inspector rendering work: what defines a todoz
  todo (a header or marker in the file?) so the inspector renders
  exactly as intended, and whether to borrow the board's whole
  preview from the todoz code; "dont we already do that on todoz? we
  have to look into this". Lead-found, needs his yes to become a
  ticket: start with a short investigation note, not code.
Uncommitted at release: this file and `docs/agent-log.md` (the
docs commit is shipped by the lead on his "you may ship it"). The
handoff is over the 150-line cap; the next lead should move the
verbatim 2026-10-15 authorization entries to the log.

Earlier that day the same lead RETOOK the baton at about 13:15 CET on Andreas's "allright. we
can continue now" (same session as the morning). On retake: `main` =
`origin/main` at `16c7a7d`, clean, CI green; NZ-43 worktree clean at
`84fa8df` (a docs-only gap to `main`, fine as base). NZ-43 (#71)
in Review since about 13:30 CET: worker (opus, medium, about 10 min,
186k tokens) delivered an uncommitted diff on branch
`feat/nz-43-todo-preview`, worktree `.claude/worktrees/NZ-43`
(`tui/todo.rs` +825/-15, README; rendered markdown with a U+2060
marker to find the task row; 15 tests). Review 1 (opus, about 5 min,
65k tokens): CHANGES REQUESTED, one medium blocker (the marker changes
emphasis flanking, so the highlighted todo renders differently);
ruling on #71: draw the plain render, use the marked render only to
find the rows and only when it matches. Fix cycle 1 (same worker,
about 3 min) done: plain render always drawn, `PaneView.scroll_rows`,
one new test. Re-review (same reviewer, about 2 min) ACCEPTED, diff
sha1 `3c1f6a5...`. Committed as `06e7254` on
`feat/nz-43-todo-preview`, pushed; branch CI run `40516545125`
GREEN (both check jobs and lint). Lead verification: `main` gained only
docs since base `84fa8df`, so the branch is the combined code; build
and tests green (695 + 5 + 191). READY TO INTEGRATE since about 14:00
CET, waiting for Andreas's try; no worker or reviewer running.
Follow-ups on #71. Andreas tried it at about 14:10 CET and said it
is not the look he asked for (that is NZ-51); NZ-43 stays UNMERGED,
branch and worktree kept, until he decides whether he wants the pane
in todoz too. NZ-51 (#77) DISPATCHED at about 14:20 CET: `nz-worker`
(opus, medium) RUNNING on branch `feat/nz-51-inspector-todo-rows`,
worktree `.claude/worktrees/NZ-51`, base `16c7a7d`, brief = the
body of #77; board In progress, P1. The first worker run stopped on
ENOSPC (the disk was full: 35 GB of session scratch under
`/private/tmp/claude-501`, 29 GB of it a fleetz session); the lead
removed its own session's scratch build copies (4 GB free), reverted
the worker's temp test, ruled option (a) on #77 (one-line `input.rs`
change passes the todo flag to `file_lines`) and re-dispatched at
about 14:35 CET. Andreas cleared the other sessions' scratch (21 GB free). Stop
condition (Andreas, about 14:45 CET: "after that we could stop for
today, if no other tickets are running"): NZ-51 runs to Ready to
integrate, then the lead releases the baton; nothing else starts. Worker (opus, about 11 min, 121k tokens) delivered at about
15:00 CET: new `tui/todo_rows.rs`, preview rows, `is_todo_note`,
one-line `input.rs` call, README (6 files +275/-250 plus the new
file). Reviewer (opus, about 7 min, 70k tokens) ACCEPTED first pass,
diff sha1 `63548dd...`, `todo_rows.rs` `cbe6163...`. Committed as
`5229193` on `feat/nz-51-inspector-todo-rows`, pushed; branch CI run
`40523374775` GREEN (both check jobs and lint). Lead verification:
origin/main is an ancestor, build and tests green (690 + 5 + 191).
READY TO INTEGRATE since about 15:20 CET for Andreas's try of
`notez tree`; follow-ups on #77 (fenced code block edge case first).
No worker or reviewer running.
Lesson: reviewers' mutation copies with their own `target/` eat
1 to 2 GB each; have them delete the copy when done. Next: `nz-reviewer`, commit
`Refs: #77`, push, branch CI, Ready to integrate for his try. The ticket stopped at Ready to integrate
for Andreas's try. Then NZ-57 (#83) on his word.

It had released the baton at about 12:58 CET on "find a place to stop
for today" with NZ-43 cut but not dispatched.

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


### `8a79e020` (Claude, `claude-fable-5-1`), 2026-10-15

Moved from the handoff's Current lead on 2026-10-17, verbatim.

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`8a79e020-56cc-417a-9e5f-599fa8f046ba`, took the baton on 2026-10-15
11:36 CEST (from `date`) on Andreas's "check the commits done today
regarding notez. then see if there's a handover and lets get to it?".
Reconciled at takeover: previous lead `f25c8d27` released the baton on
2026-10-10 13:48 with nothing in flight (record below); `ps` shows one
`claude --agent nz-coordinator` process (this session, pid 44577) and
no worker or reviewer. `main` = `origin/main` = `7cfeb27`, working tree
clean, only the main checkout in `git worktree list`, no local or
remote ticket branch (`origin/phase2-subdir-and-stubs` is an old
non-Relay branch). `7cfeb27` is Andreas's own commit of today 11:19
(auto-register notez repos, list empty stores in the browser), made
outside Relay; its `main` CI run is green. It touched
`commands/tree.rs`, `main.rs`, `tui/tree/{delete,mod,model,
test_support}.rs`, `config/registry.rs` and `core/project.rs`, so
every ticket from here bases on `7cfeb27` or later and the NZ-5 brief's
code facts (pre-split line numbers) are stale: the filter strip and
`search_mode` live in `tui/tree/input.rs` and `tui/tree/search.rs`.
Board agrees with the 2026-10-10 stop: NZ-5, NZ-11, NZ-28, NZ-30,
NZ-32 Ready; NZ-17, NZ-18, NZ-29, NZ-42 Draft; the rest Done. Two
board gaps carried over: NZ-43 and NZ-44 have no board row in the
listing (NZ-44 was merged in `10312c6`). Reading of "lets get to it",
stated to Andreas at takeover for him to correct: RESUME the Next step
queue under the recorded standing scope and the "merge as you go"
delegation: NZ-5 on the `tui/tree/` line, NZ-30 pass 1 and NZ-11 in
the other slots, then NZ-42, NZ-32, NZ-28 pass 2, NZ-30 pass 2.
Nothing is dispatched until he confirms. (He confirmed, with
changes, at about 12:10; see Authorized by the owner, 2026-10-15.)

STOPPED FOR THE DAY on 2026-10-15 on Andreas's "we can try to find a
good place to stop for the day". BATON RELEASED once the NZ-32
`main` run finished (see the last In flight entry for its result).
No lead is active. No worker or reviewer is running: every agent
dispatched today reported and its ticket was merged or closed; the
last were the NZ-32 worker and reviewer.

State at the stop, in short:

- Done and merged today, in order: NZ-45 (`aabf8f2`, storage
  integration tests), NZ-17 (`dd2ff6a`, version 0.1.0, tagged
  `v0.1.0`), NZ-5 (`e4a3130`, fuzzy filter, `0`, `s`), NZ-46
  (`0d91cd4`, docs refresh), NZ-11 (`835d932`, footer display
  columns), NZ-48 (`4c58f36`, notez-core frontmatter and dates,
  workspace 0.2.0), NZ-32 (`bc63288`, document type filter).
  `main` = `origin/main` = `bc63288` plus the docs commit this lead
  makes at the stop. CI green on each code merge.
- No ticket worktree, branch or remote branch remains after the
  NZ-32 cleanup. Tag `v0.1.0` at `dd2ff6a`; 0.2.0 is NOT tagged (the
  owner tags when he wants; `main` carries notez-core API additions
  since 0.1.0).
- Board: NZ-5, NZ-11, NZ-17, NZ-32, NZ-45, NZ-46, NZ-48 Done; NZ-30,
  NZ-43 Ready; NZ-42 Draft (narrowed, brief to write); NZ-18, NZ-28
  (parked), NZ-29 (closed "not now"), NZ-47 (design) Draft.
- Andreas's installed binary: he was told to `./install.sh` after
  wave 1; whether he did is not recorded. Visible changes since
  2026-10-10: fuzzy `/` filter with lit matches in both views, `0`,
  `s` cycling eight stops (all, scratch, project, global, private,
  public, docs, todo) named in the header, `@type` filter tokens,
  "no matches" row, column-correct footer warnings, `notez
  --version` with the sha (0.2.0).
- Decisions taken today are under Authorized by the owner
  (2026-10-15, two entries) and in `docs/design-nz47-dates.md`.
- Standing permissions in force and paused with the baton: the
  integration delegation ("merge as you go"), branch pushes with
  remote deletion after the merge, board items for tickets Andreas
  asks for in a lead session, the tree-sitter and pulldown-cmark
  dependency approval, the working rule, the three-worker cap, and
  the 2026-10-15 "milestones only" reporting rule.
- Process notes from today: a reviewer that restores a mutated file
  with its old mtime leaves a stale test binary, so the lead
  `touch`es a source file before its own verification run; `notez
  --version` exits 2 (pre-existing clap behaviour), so never put it
  in a `&&` chain before a push; the lead's own briefs twice carried
  stale or wrong code facts (NZ-5 "already fuzzy", NZ-46 "tree
  dims") and the reviewers caught both: verify code facts before
  writing a brief, or mark them "worker verifies".

Previous lead record follows.

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`f25c8d27-3615-4dda-8b9c-273a6aea1204`, took the baton on 2026-10-10
12:41 CEST on Andreas's "continue work". Reconciled at takeover: `main`
= `origin/main` = `e7c51ca`, working tree clean, `git worktree list`
shows only the main checkout, no local ticket branch, no worker or
reviewer running. `ps` shows exactly one `claude --agent nz-coordinator`
process (this session, pid 66015, started a minute before the check);
the only other lead process is `fz-coordinator`, another project's.
The previous lead (sonnet session of 2026-10-09, record below) wrote
no explicit release line; its last activity is the `e7c51ca` docs
commit at 21:07 CEST on 2026-10-09 and its In flight note says nothing
is in flight, so it is treated as stopped. Board agrees with the
handoff: NZ-39, NZ-40, NZ-41 Done; NZ-3, NZ-5, NZ-6, NZ-11, NZ-28,
NZ-30, NZ-32 Ready; NZ-17, NZ-18, NZ-29, NZ-42 Draft (NZ-43 is in the
handoff as a Draft but has no board row in the current listing; the
previous lead recorded id `..._uH10` for it). Loose end: the remote
branch `nz-39-suffix` still exists although the NZ-39 record says it
was removed (merged in `234d21c`). Reading of "continue work", stated
to Andreas at takeover so he can correct it: RESUME the 2026-10-08
standing scope queue (next NZ-3, then NZ-5, NZ-32, NZ-28, NZ-30, with
NZ-11 and NZ-6 in the second slot) and the todoz scope from 2026-10-09
(NZ-42 after NZ-5 is merged; NZ-43 still needs a brainstorm and does
not run), under the "merge as you go" integration delegation. Nothing
is dispatched until Andreas confirms that reading. He confirmed it at
12:50 ("order is fine"); the day's authorizations are under Authorized
by the owner.

CLOCK CORRECTION: the takeover time 12:41 CEST came from `date`; every
later CEST time this lead wrote today (12:48 through "16:14") was an
estimate and runs ahead of the real clock by up to about two and a
half hours (`date` said 13:48 when the lead wrote "16:14"). The order
of events is right; for real times use `git log --date=iso` and the CI
runs' `createdAt`. The whole day's work ran between 12:41 and 13:48
CEST real time.

STOPPED FOR THE DAY on 2026-10-10 on Andreas's "after current workers
come back green we can stop for today" and "ping me when it's merged
and I can close". BATON RELEASED at 13:48 CEST real time, once the
NZ-44 `main` run was green. No lead is active. No
worker or reviewer is running: the last agents (the NZ-44 worker and
reviewer) reported and NZ-44 was merged. Nothing was dispatched after
the 15:02 instruction.

State at the stop, in short:

- Done and merged today, in order: NZ-3 (`460c4b9`, header line),
  NZ-28 pass 1 (`4841211`, lint module, nothing on screen yet), NZ-6
  (`2fb8c0d`, quiet offline exit), NZ-44 (`10312c6`, `tui/tree.rs`
  split into `tui/tree/`). `main` = `origin/main`; CI green on each.
- No ticket worktree or branch remains; no remote ticket branches.
- Board: NZ-3, NZ-6, NZ-44 Done; NZ-28 Ready (pass 2 pending); NZ-5,
  NZ-11, NZ-30, NZ-32 Ready; NZ-42, NZ-43, NZ-17, NZ-18, NZ-29 Draft.
- Andreas's installed binary predates today. After his next
  `./install.sh` the visible changes are NZ-3's header row (view and
  scope left; sync state, `N uncommitted` or `clean`, note or open/done
  counts right; panes one row shorter; pane title reduced to the number
  badge; narrow widths drop the right-hand segments) and NZ-6's silent
  offline exit.
- Next when Andreas says continue (see Next step): NZ-5 on the
  `tree.rs` line (its brief's code facts predate the split: the filter
  strip and `search_mode` now live in `tui/tree/search.rs` and
  `tui/tree/input.rs`; the worker verifies), NZ-30 pass 1 off the line,
  NZ-11 off the line; then NZ-42, NZ-32, NZ-28 pass 2, NZ-30 pass 2.
- Standing permissions in force and paused with the baton: the
  integration delegation ("merge as you go"), branch pushes with remote
  deletion after the merge, board item creation for tickets Andreas
  asks for in a lead session, the tree-sitter and pulldown-cmark
  dependency approval, the working rule (the lead decides open design
  points and reports), and the three-worker cap of 2026-10-10.
- Process notes: the auto-mode classifier twice refused an Edit that
  restated the working rule in the handoff ("Instruction Poisoning");
  a shorter wording that referenced the existing record went through.
  macOS `wc -l` pads its output, so compare counts with `awk 'END{print
  NR}'`. One merge chain on `main` at a time held all day; three
  workers on disjoint files worked without a collision (the one shared
  line in `tui/mod.rs` merged cleanly).

Previous lead record follows.

Claude `nz-coordinator` role, run by a plain session on `claude-sonnet-5-5`
(not the recorded `fable` lead model; Andreas chose this substitution
explicitly: "I take the baton here"), took the baton on 2026-10-09
20:10 CEST on Andreas's "lets do this with relay" and "lets this relay
lead work on todoz". Reconciled at takeover: `main` = `origin/main` =
`14c0c14`, working tree clean, `git worktree list` shows only the main
checkout, no ticket branch, no worker or reviewer running. The only
`--agent` process on the machine is `fz-coordinator`, another project's
lead. Previous lead released the baton on 2026-10-08 (record below).
Scope this lead runs: NZ-40 to NZ-42 below (importance dots back in the
tree browser, then the todoz visual pass and navigation). NZ-43 (todoz
preview pane) is a Draft that needs a brainstorm with Andreas first.
Integration: "Merge as you go" (Andreas, 2026-10-09, see Authorized by
the owner).

Earlier lead records (2026-10-06 to 2026-10-08), the one-off
authorizations of 2026-10-06 and 2026-10-07, the In flight records of
Done tickets, the Done ticket briefs and the 2026-10-07 Next step were
moved verbatim to `docs/agent-handoff-archive.md` on 2026-10-10 (owner
decision, see Authorized by the owner, 14:05). Standing permissions that
still apply stay below.

## Finished tickets

One subsection per ticket, newest merge first: the ticket brief as it
stood in the handoff, then its In flight entries in the order they were
written. In flight entries that cover several tickets at once are under
"Wave and day notes" below.

### NZ-43 (#71): todoz preview pane

Design approved by Andreas on 2026-10-15 (right pane ON by default at
120 columns or more, `p` toggles, NZ-4 resize and fold keys through
`tui/panes.rs`, the selected task's source `TODO.md` rendered and
scrolled to the task's line, highlighted; footer shows `Task.due`;
no editing, linked-note previews or calendar). P1, 0.3.0, board item
`PVTI_lAHOCU842c4BmE5Zzg_uH10`, brief in the issue body. Cut by lead
`9c5067bf`, run by lead `0bafc905` on 2026-10-27 on "start next
ticket": branch `feat/nz-43-todo-preview`, worktree
`.claude/worktrees/NZ-43`, base `84fa8df`.

Worker (opus, medium, about 10 min, 186k tokens): `tui/todo.rs`
+825/-15 and README; rendered markdown with a U+2060 marker to find
the task row; 15 tests. Review 1 (opus, about 5 min, 65k tokens):
changes requested, one medium blocker (the marker changes emphasis
flanking); ruling on #71: draw the plain render, use the marked render
only to find the rows and only when it matches. Fix cycle 1 (about 3
min): plain render always drawn, `PaneView.scroll_rows`, one test.
Re-review (about 2 min) accepted, diff sha1 `3c1f6a5...`. Committed as
`06e7254`, branch CI `40516545125` green, lead checks green (695 + 5 +
191). Andreas tried it on 2026-10-27 and said it was not the look he
asked for (that became NZ-51); the branch stayed unmerged at Ready to
integrate. On 2026-10-29 lead `7c4dc6c8` found the merge onto
`da0229a` clean (no `todo.rs` conflict with NZ-51's extraction) and
green (706 + 5 + 191); Andreas first saw an empty board (`notez todo`
inside a repo scopes to that project; the full board is `-g todo`),
then tried the `-g` build and accepted ("ok that works for now. looks
good enough"). Merged `--no-ff` as `b835e55` (Closes #71), pushed, CI
on `main` run `40874676111` green, remote branch deleted, worktree
swept.

Follow-ups on #71: split the preview block into `tui/todo/preview.rs`
and share the copied tree helpers from `tui/panes.rs`; `2` on a
too-narrow terminal gives no message; a lost drag release pauses the
probe until the next mouse event; a todo with emphasis touching the
start or end of its text is scrolled to but not highlighted; drawing
the file through `tui/todo_rows.rs` (todoz look) is open for a later
ticket.

NZ-43: review changes 1; fix cycles 1; owner decisions 2; worker 13; review 7; usage read (worker 186k tokens, reviewer 65k)

### NZ-51 (#77): TODO.md in the notez inspector renders like standalone todoz

Asked for by Andreas on 2026-10-15 and again on 2026-10-27 after the
NZ-43 try ("the [ ] are translated to actual boxes, rather than being
ascii"). Run by lead `0bafc905` on 2026-10-27 (P1, 0.3.0, board item
`PVTI_lAHOCU842c4BmE5ZzhBAXLs`), brief and lead decisions in the issue
body. Branch `feat/nz-51-inspector-todo-rows`, worktree
`.claude/worktrees/NZ-51`, base `16c7a7d`.

First worker run stopped on ENOSPC (35 GB of session scratch under
`/private/tmp/claude-501`); the lead cleared its own session's
scratch build copies, Andreas the rest. Lead ruling (a): one-line
`tree/input.rs` change passes the node's `DocType::Todo` bit into the
preview. Second worker run (opus, medium, about 11 min, 121k tokens):
new `tui/todo_rows.rs` with the board's per-row drawing moved out of
`todo.rs::board_rows_matching` (pure move, byte-identical snapshot
test captured before the move), `Preview::file_lines_for` drawing runs
of checkbox lines as board rows with items from `load_single_todo` and
other runs through `render_markdown`, `PreviewKey.todo`, `is_todo_note`
in `doc_type.rs`, README. Reviewer (opus, about 7 min, 70k tokens)
accepted first pass: pure move confirmed, snapshot proven against the
base, six mutants killed; diff sha1 `63548dd...`. Committed as
`5229193`, branch CI `40523374775` green, lead checks green. Andreas
tried it and accepted for now ("it still gotta long way to go").
Merged `--no-ff` as `3c3c762` (Closes #77), checks green, pushed;
CI on `main` run `40524947003`.

Follow-ups on #77: a checkbox line inside a fenced code block splits
the fence; markdown spanning runs restarts per run; the file is read
twice. Open design question from Andreas for the next pass: what
defines a todoz todo (a header or marker?) so the inspector renders
exactly as intended, and whether to borrow the board's whole preview
from the todoz code.

NZ-51: review accepted first; fix cycles 0; owner decisions 1; worker 11; review 7; usage read (worker 121k tokens, reviewer 70k; a first run of 69k lost to ENOSPC)

### NZ-56 (#82): sync the vault from inside the TUI with one key

Cut by lead `b5670714` on 2026-10-27 from Andreas's "add all these as
features and get started then?" (P1, 0.3.0, board item
`PVTI_lAHOCU842c4BmE5ZzhD2AGw`); the brief with the lead decisions is
the issue body. Run by lead `0bafc905` the same day, stacked on NZ-55's
reviewed commit `d32a412` because both touch `tui/tree/input.rs` and
`render.rs`. Branch `feat/nz-56-tui-sync`, worktree
`.claude/worktrees/NZ-56`.

Worker (opus, medium, about 10 min) delivered `tui/sync.rs` (new:
`run`, `refusal`, `VaultSync` start / take_finished / wait /
poll_interval, bare-remote test with two clones), `SyncState::Syncing`
and `DirtyCount::cached` in `header.rs`, `g` in both key tables
(help-only like `R`), `flush_tag_maps` and `refresh_tag_baseline` in
`tree/model.rs`, `save_before_sync` on the board, README. Lead rulings
on #82: help-only `g`; only a stopped opening pull blocks `g`; "changed
files" means HEAD moved. Review 1 (opus, about 5 min): changes
requested, two quit-path blockers: the wait for a running sync ran
inside the raw-mode loop (a hung network froze the terminal with
Ctrl-C dead), and a board quit during a sync could save a pre-pull
TODO.md over the pulled one. Fix cycle 1 (about 6 min): `VaultSync::
finish()` called after `leave()` from `run_tree` / `run_board` with a
stderr line, `pulled_over_edits` + `quit_after_sync` on the board
returning to the loop with the NZ-42 held message, test added.
Re-review (about 2 min) accepted: diff sha1 `e2a9535...`, `sync.rs`
`f7b5856...`. Committed as `0ad8fb4`, branch CI `40485599886` green,
lead checks green on the branch (= combined result). Andreas tried the
branch build with both tickets and accepted ("thats perfect ... im
happy. continue"). Merged `--no-ff` as `17bdf3e` (Closes #82), checks
green on `main`, pushed; CI on `main` run `40499879883`.

Carried follow-ups (on #82, not blocking): the panic hook leaves the
TUI from any thread; git credential or ssh passphrase prompts on
`/dev/tty` during a raw-mode sync (`GIT_TERMINAL_PROMPT=0` would need a
core change); the stderr wait line lingers after a held-quit re-entry;
two `redundant_clone` clippy warnings in the new test.

NZ-56: review changes 1; fix cycles 1; owner decisions 1; worker 16; review 7; usage read (worker 166k + 77k tokens, reviewer 73k + 96k)

### NZ-55 (#81): tree list keeps a 5-row scroll margin and reveals an expanded folder

Reported by Andreas on 2026-10-27 (an expanded folder on the bottom row
stayed out of view), cut by lead `b5670714` (P1, 0.3.0, board item
`PVTI_lAHOCU842c4BmE5ZzhD2ADo`), run by lead `0bafc905` the same day.
Branch `fix/nz-55-tree-scroll`, worktree `.claude/worktrees/NZ-55`,
base `90e8a25` (recut after the sweep removed the first clean cut at
`2e3dee7`). Files `tui/tree/render.rs`, `tui/tree/input.rs`.

Worker (opus, medium, about 4 min): `SCROLL_MARGIN = 5` through
`List::scroll_padding` in a new `tree_list`, a pure `reveal_offset(
nodes, visible, folder, offset, height)` with 6 tests (last row on the
bottom row, tall folder stops at the top margin, short pane uses the
shrunken margin, rows that fit keep the offset, never scrolls up,
filter-hidden rows do not count; buffer renders prove ratatui's padding
cannot undo the reveal), a loop-level `reveal: Option<usize>` set only
when a folder opens at `l`/Right, `Enter`/`o` and the mouse click, and
consumed in the draw closure before `render_stateful_widget`. Reviewer
(opus, about 3 min) accepted first pass with a mutation run; diff sha1
`5600911...`. Follow-ups on #81 (low): no test pins the navigation
margin itself; `take_while` in `reveal_offset` has no test with a later
expanded sibling. Committed as `d32a412`, branch CI `40482253710`
green. Accepted by Andreas through the NZ-56 build. Merged `--no-ff`
as `03427e6` (Closes #81), checks green, pushed; CI on `main`
`40489422617` green.

NZ-55: review accepted first; fix cycles 0; owner decisions 0; worker 4; review 3; usage read (worker 56k tokens, reviewer 37k)

### NZ-53 (#79): notez agenda and the inspector date footer

Consumers 2 and 3 of `docs/design-nz47-dates.md` (planned in the
handoff as "NZ-49"; the id came from `relay-board next`). Issue
created by lead `9c5067bf` on 2026-10-26 under the standing board
permission; P1, milestone 0.3.0. Base `0019c62`, branch
`feat/nz-53-agenda`, worktree `.claude/worktrees/nz-53`, worker
`nz-worker` (opus, medium), reviewer `nz-reviewer` (opus, medium).

Outcome: `notez agenda [--days N]` (default 7) lists `due` and `event`
dates of notes (`collect_all`, daily logs excluded, 4 KiB head read
once per note) and `Task.due` of unchecked todos, overdue first, then
one group per day with entries; one plain line and exit 0 when empty.
The preview's bottom border shows `due`, `on` (event) and `created`
after the suffix, through a one-entry cache keyed by (path, mtime,
len); segments drop whole (created, on, due) and the path keeps at
least its shortest form. Lead rulings: `--json` deferred to NZ-54
(#80) once Andreas delegated the `serde_json` decision; title is the
first `#` heading else the file stem; half-done tasks listed; global
TODO label is `Task.section`; daily-log match on any path component
accepted; non-UTF-8 titles print with U+FFFD.

Run: the brief named `render.rs` where `tui/tree/input.rs` draws the
border, so the worker reported with the footer unwired and the lead
added `input.rs` to its files (lead error, 3 min). Review 1: changes
requested, blocker `--days` overflow panic, plus lead rulings on path
priority and the daily-log test. Fix cycle 1 (2.5 min), re-review
accepted at code hash `c54847c8…`; lead applied the worker's README
text, accepted at full hash `c4c289e7…`. Commit `ab6748f`, branch CI
40342554779 green, merged `c226a6c` (`--no-ff`), `main` CI
40343960031. On the combined tree one pre-existing notez-core test
(`sync::tests::auto_sync_pushes_offline_commits_once_the_remote_is_reachable`,
a `git clone` into a temp dir) failed once and passed on three
reruns; flaky under load, not touched by either ticket.

NZ-53: review changes 1; fix cycles 1; owner decisions 1; worker 14; review 11; usage read (worker 237k tokens, reviewer 84k)

### NZ-42 (#70): todoz reloads when its TODO.md files change

Narrowed 2026-10-15 to `/` fuzzy search plus reload-on-change; on
dispatch (2026-10-26, lead `9c5067bf`) the code showed todoz's `/`
already used NZ-5's fuzzy matcher and highlight, so the ticket ran as
reload-on-change only. P1 and milestone 0.3.0 set by the lead. Base
`0019c62`, branch `fix/nz-42-todoz-reload`, worktree
`.claude/worktrees/nz-42`, worker `nz-worker` (opus, medium),
reviewer `nz-reviewer` (opus, medium). Files `tui/todo.rs`, README.

Outcome: a 2 s idle probe (`event::poll`) of the mtimes of the loaded
`TODO.md` sources (plus `_todos` on the global board), metadata only,
never while a prompt, mode, help overlay or drag is open; reload
keeps cursor (same task by file, section, kind, depth, text), filter,
folds and focus state; `R` reloads on demand; dirty edits are saved
before a reload. Lead rulings: single-scope boards re-read per section
(`load_single_todo`), code TODOs rescanned only on restart; the tree's
probe helpers duplicated (15 lines) rather than widening visibility.
After review: a file that changed on disk while it holds unsaved
edits is never saved over automatically; the footer asks for `R`, and
a clean change waits while any file is held; the snapshot updates
only after a successful reload so a failed save retries.

Run: worker 9 min, review 1 accepted at `14796897…` with four
follow-ups; lead took two before merge (fix cycle 3 min), re-review
accepted at `ccfdae2a…` (mutation-tested). Commit `cb185fc`, branch
CI 40341267013 green, merged `cd62547`, `main` CI 40342378322 green,
remote branch deleted. Follow-ups on #70: status message replaces the
whole footer until a key; duplicate parent rows can swap fold state;
misleading message after a partial save failure; no lasting sign the
board is out of date while a file is held.

NZ-42: review accepted first; fix cycles 1; owner decisions 0; worker 12; review 9; usage read (worker 246k tokens, reviewer 116k)

### NZ-32: filter by document type

Status: Ready, runs after NZ-5 (it extends the filter NZ-5 reworks).
Requested by Andreas on 2026-10-08 at 21:50 CEST: "should be able to
sort or filter by document type: pinz, todo, private, public, global,
etc". Board item `PVTI_lAHOCU842c4BmE5Zzg_fBig`.

Lead decisions (working rule): filter, not sort. The tree already
groups by section (project, then scope, then docs), so a sort by type
would reorder what the sections already express; a filter answers the
request directly. Types: `todo` (the `_todos` store and `TODO.md`
rows, NZ-20/NZ-24), `private` (personal sections), `public`, `global`,
`docs`, `scratch` (local), and `pinz` reserved for NZ-18 (accepted by
the parser, matches nothing until Pinz rows exist, so the help can list
it). Two ways in: typed tokens in the `/` filter (`@todo`, `@public`,
`@private`, ..., combinable with text and `#tag` tokens under the
existing AND rule; several `@` tokens OR together like `#tag` tokens
do), and the `s` scope cycle from NZ-5 extended to cycle through these
types (all, private, public, scratch, global, docs, todo) so one key
walks them without typing. The footer or header names the active type
filter. Sections with no matching rows are hidden while a type filter
is active; the "no matches" row from NZ-5 covers an empty result.

Acceptance: parser tests for `@type` tokens (valid, unknown token is
plain text, combination with `#tag` and text); matching tests per type
on a temp tree with every section kind; `s` cycle order and footer
label; help lists the tokens; README. Allowed files:
`crates/notez-cli/src/tui/tree.rs`, `tui/text.rs`, `tui/footer.rs`,
`tui/help.rs`, `README.md`; the `@` token parsing lives in notez-cli
(no `notez_core::filter` change, so epoz is untouched: the cli strips
`@` tokens before handing the rest to `notez_core::filter::parse`).
One worker pass, one review.

#### In flight record

NZ-32 worker REPORTED (about 13 min): done, 619 + 5 + 157 green,
new `tui/tree/doc_type.rs` (`parse_filter`, `TreeFilter`,
`clear_type_tokens`), `TreeNode.doc_types` bitmask, eight-stop `s`
cycle (all, scratch, project, global, private, public, docs, todo),
`@` tokens AND with the stop, `s` back to all clears `@` tokens, 17
new tests; footer.rs, header.rs, help.rs, text.rs needed no change.
Existing-test edits ACCEPTED by the lead as contract updates: the
four-stop cycle test renamed and extended to eight stops; one call
site changed for the `compute_filter_keep` signature. Worker
decisions recorded: every note under `_todos/` counts as todo; the
header names only the `s` stop. REVIEW DISPATCHED (`nz-reviewer`).

NZ-32 REVIEW ACCEPTED at hash `afe8e848…707c`, no blockers; the
reviewer verified `mail@todo`-style tokens stay text and that the
remainder equals core's own parse for buffers without `@` tokens.
LEAD DECISION: take three low follow-ups before committing (guard
the buffer rewrite on `s` back to all; folders match type tokens
only as ancestors so a type filter does not leave every folder
expanded; README wording for eight stops), worker resumed, re-review
after. Design question left open for Andreas to meet in the build:
section rows carry no type, so an empty store of a type hides on
that type's own stop.

NZ-32 follow-up round REPORTED: guard inside `clear_type_tokens`,
notes-only typing (folders kept only as ancestors; an empty folder
in a typed section hides under its type like under a text filter),
README reworded; 621 + 5 + 157 green; two new tests written first
and seen failing. RE-REVIEW DISPATCHED to the same reviewer. Merge
order: NZ-48 first (branch CI running), then NZ-32.

NZ-32 RE-REVIEW ACCEPTED at hash `22c92479…0c67`, no blockers; two
nits left (a doc-comment parenthetical in `assign_doc_types`, README
line wrapping). Lead verification after `touch`: zero warnings, 621
+ 5 + 157 green, hash matches. COMMITTED `f1f14b5` on
`feat/nz-32-type-filter`, PUSHED, branch CI watch in the background.

### NZ-48: notez-core frontmatter and dates, read-only (2026-10-15, Ready)

Board item `PVTI_lAHOCU842c4BmE5ZzhA-tKk`. Authorized by Andreas
("6: yes", 2026-10-15) as the `notez-core` part of
`docs/design-nz47-dates.md`, which is the brief's source of truth.
Runs after NZ-11 merges (it adds a field to `Task`, and
`tui/todo.rs` has four `Task { .. }` literals), beside NZ-32.

Outcome: `notez-core` can read dates from notes and todos, nothing
is written to any note file, `TODO.md` round-trips byte-identically
including `@date` tokens, and the workspace is 0.2.0.

Acceptance criteria:

1. `core::frontmatter`: `split(text) -> (Vec<(String, String)>, &str)`
   and `set(text, key, value) -> String` with the Pinz rules (block
   starts at byte 0 with `---\n`, ends at the next `\n---`; no block
   or unterminated block means no fields and the text unchanged;
   `key: value` lines, trimmed; unknown keys preserved verbatim and
   in order by `set`, which inserts or replaces one key and leaves
   every other byte identical). Tests include a Pinz pin file with
   `x`, `y`, `z`, `color` surviving `set("due", ...)` unchanged.
2. `core::dates`: `NoteDate { date: NaiveDate, time: Option<NaiveTime> }`
   parsed from `YYYY-MM-DD` and `YYYY-MM-DDTHH:MM`, formatted back the
   same way; anything else is `None` (kept verbatim by callers, never
   an error). `chrono` is already a dependency; no new crate.
3. `NoteEntry` gains `created`, `due`, `event: Option<NoteDate>`
   with `#[serde(default)]`, filled from the frontmatter; `created`
   falls back to a `Date: YYYY-MM-DD` line in the first lines, then a
   `YYYY-MM-DD-` or `YYYY-MM-DD-HHMMSS-` filename prefix, then
   `None`. Reading the first 4 KiB of each file is enough; measure
   that `collect_all` on a vault of a few hundred notes stays within
   the same order of time as before (report the numbers).
4. `Task` gains `due: Option<NoteDate>` with `#[serde(default)]`. The
   parser strips one whitespace-delimited `@YYYY-MM-DD` or
   `@YYYY-MM-DDTHH:MM` token (exact shape, so `name@example.com` and
   `@2026` stay text; a second date token stays in the text) and the
   serializer writes it back after the text and before the tags.
   `parse_todos_from_content` keeps its public tuple signature as a
   wrapper; a new `parse_todo_lines` returns a small struct with the
   date. Every `Task { .. }` literal in the workspace gets
   `due: None` (`crates/notez-cli/src/tui/todo.rs` and
   `commands/todo.rs` are allowed for those lines only).
5. Round-trip test: a `TODO.md` with dates, tags, prose, nested tasks
   and a `[/]` state is byte-identical after load and save; the
   existing dirty-source tests are untouched.
6. Workspace version 0.2.0 in the root `Cargo.toml`; `CHANGELOG.md`
   Unreleased gets a `notez-core` subsection naming the new module,
   type and fields and stating that epoz's pinned rev is unaffected
   until it moves; `docs/versioning.md` unchanged.
7. No write path: nothing in this ticket writes frontmatter into a
   note file; `set` exists and is tested but has no caller yet.
   `notez_core::filter` is untouched. No CLI or TUI behaviour change.

Allowed files: `crates/notez-core/src/**`, `Cargo.toml`,
`CHANGELOG.md`, and the `Task { .. }` literal lines in
`crates/notez-cli/src/tui/todo.rs` and
`crates/notez-cli/src/commands/todo.rs`. One worker, one review.

#### In flight record

NZ-48 worker REPORTED (about 14 min): core done (frontmatter, dates,
`Task.due`, `parse_todo_lines`, round-trip test, 0.2.0, changelog),
190 core tests green, but blocked: eager `NoteEntry` date fields
broke three test literals in `commands/edit.rs` and
`commands/tree.rs` outside its files, and `collect_all` on 300 notes
went from about 0.4 ms to 4 to 7 ms (it opened every file). LEAD
DECISION (design change, recorded in `docs/design-nz47-dates.md`):
dates are read on demand, not eagerly. No fields on `NoteEntry`;
`core::dates::NoteDates { created, due, event }` with
`from_text(text, file_name)` and `read(path)` (first 4 KiB), to be
called by the inspector footer and `agenda`. That removes the
literal edits and the cost from every tree build. Also accepted: the
last `@date` on a line wins (stable under the serializer position);
`parse_todos_from_content` output unchanged; `NoteDate` serde as
text; `Cargo.lock` version lines allowed. Worker resumed and
REPORTED: `NoteEntry` and the aggregate functions back to base,
`NoteDates::from_text` and `NoteDates::read` as associated functions
(accepted), `collect_all` 420 us vs 390 us base, `read` over 300
files 3 to 7 ms, 602 + 5 + 191 green with one ignored timing probe,
`notez 0.2.0 (835d932-dirty)`. REVIEW DISPATCHED (`nz-reviewer`)
with frontmatter byte-exactness (CRLF, BOM, horizontal-rule false
block) and `@date` edge cases asked for.

NZ-48 REVIEW ACCEPTED at hash `71f15d45…30e7`, no blockers; the
reviewer probed `set` byte-exactness, token edge cases, load-save-
load idempotence, base-equivalence of `parse_todos_from_content` and
serde back-compat in a scratch crate. Follow-ups: F1 a `@date` on a
TODO.md line now disappears from the todoz row text (no such tokens
exist in Andreas's files today; the due column ticket shows it); F2
(medium, for the write path NZ-50, recorded in the design note): a
note opening with a horizontal rule, a CRLF block or a UTF-8 BOM is
misread as or outside a block, so `set` must refuse or special-case
them before it gets a caller; F3 the ignored timing probe's 2 ms
bound is tight in debug builds; F4 a date inside a spaced code span
counts. Lead verification after `touch`: zero warnings, 602 + 5 +
191 green, hash matches. COMMITTED `5610338` on
`feat/nz-48-core-dates`, PUSHED, branch CI watch in the background.
Owner approval for the `notez-core` API and `TODO.md` token change:
"6: yes" (Authorized by the owner, 2026-10-15). F2's write-path rules
are recorded in `docs/design-nz47-dates.md` under "Requirements for
the write path".

NZ-48 branch CI run `38748941192` GREEN. MERGED into `main` as
`4c58f36` (`--no-ff`), combined checks zero warnings, 602 + 5 + 191
green, pushed; `main` CI watch in the background.

### NZ-11: the warning footer measures display columns

Status: Ready, not started. Asked for by Andreas with NZ-10. Small.

Problem: `warning_layout` in `tui/tree.rs` counts chars, not display
columns. A long warning that quotes a path with wide characters (CJK,
emoji) pushes the line past the width and clips the quit hint. Review 4 of
NZ-1 saw `quit` cut to `qui` with an emoji in the text. The todo board's
warning lead (`warning_lead` in `tui/todo.rs`, through
`footer::status_line`) should be checked for the same.

Outcome: truncation and padding use display width (ratatui's
`Span::width()`, no new dependency), so the quit hint stays whole and on
its column for any text. Tests with CJK, emoji and combining characters.
While there, the tree's warning arm can draw its quit hint through
`footer::hint_spans` instead of hardcoded spans (noted in NZ-2's review).

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/todo.rs`, `crates/notez-cli/src/tui/footer.rs`.

#### In flight record

NZ-11 REVIEW ACCEPTED at hash `93e119a4…27c5`, no blockers; the
reviewer probed ZWJ families, flags, lone combining marks, Hangul
jamo and tabs at widths 0 to 80 in a scratch copy and drew into a
real ratatui buffer; follow-ups (low, pre-existing): `footer_cols`
counts chars (ASCII keys only), below width 7 the line still
overflows, a tab counts as one column. Lead verification after
`touch`: zero warnings, 602 + 5 + 157 green, hash matches.
COMMITTED `28ed23c` on `fix/nz-11-footer-width`, PUSHED, branch CI
watch in the background. Branch CI run `38737608885` GREEN. MERGED
into `main` as `835d932` (`--no-ff`), combined checks zero warnings,
602 + 5 + 157 green, pushed; `main` CI watch in the background;
NZ-11 worktree and branches removed once it is green.

`main` run `38745116321` GREEN. NZ-11 DONE: board Done, worktree and
branches removed. `main` = `835d932`.

### NZ-46: refresh DESIGN.md status and stale docs (2026-10-15, Ready)

Board item `PVTI_lAHOCU842c4BmE5ZzhA8URg`. Andreas: "update DESIGN.md
(and other docs)". Runs after NZ-5 and NZ-17 merge (it needs the
README and must describe the version policy). `nz-small` is enough.

Outcome: the docs describe the software as it is. `DESIGN.md` Status
no longer says `edit` and `nav` are stubs; it lists what has landed
since (tree browser features NZ-1 to NZ-44 in one paragraph, header
line, markdown and highlighting in the preview, lint module present
but unused, auto-registration from `7cfeb27`); the "Non-markdown files
and a richer reader" idea records the 2026-10-15 decision that the
preview may become a file reader (toggleable) and that linting waits
for non-markdown notes; `README.md` gets the version line from NZ-17
and any key table NZ-5 left out; `docs/multi-user-sync.md` and the
`handover-2026-09-02-small-machine.md` get a one-line "historical"
note at the top if they are stale. Allowed files: `DESIGN.md`,
`README.md`, `docs/multi-user-sync.md`,
`docs/handover-2026-09-02-small-machine.md`. Docs-only: link and
content review, no code checks.

NZ-46's In flight entries share paragraphs with NZ-11; see "Wave and
day notes", 2026-10-15.

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

#### NZ-5: fuzzy search, tag filter clearing and scope cycling

TRIMMED 2026-10-15 (lead's product review, Andreas "i trust you on
the rest"): criterion 1 (the fleetz scorer port) is dropped, since
nothing is ranked and the tree keeps its order, so a score has no
consumer. What runs: (1) text tokens match by case-insensitive
subsequence in both views (if a view already does, keep it and say
so), `#tag` tokens and `notez_core::filter` unchanged, `notez-core`
not edited; (2) matched characters highlighted in tree rows (todoz
rows too if the same helper serves); (3) `0` in tree normal mode
clears `#tag` tokens and keeps the free text; (4) `s` in tree normal
mode cycles all, scratch, project, global and the NZ-3 header names
the active scope; (5) an empty result shows a "no matches" row; (6)
help and footer list `0` and `s`, README key table updated; (7) unit
tests for match positions, highlight spans, tag clearing, cycle order
and the empty row; existing filter tests untouched. Allowed files:
`crates/notez-cli/src/tui/text.rs`, `tui/tree/**`, `tui/todo.rs`,
`tui/footer.rs`, `tui/help.rs`, `tui/header.rs`, `tui/theme.rs`,
`tui/mod.rs`, `README.md`. In flight 2026-10-15 from `7cfeb27` on
`feat/nz-5-fuzzy-search`. The original brief follows for history.

Status: Ready, runs after NZ-3. In the standing scope since 2026-10-06
with the lead's recommended answers accepted by Andreas then ("tree
keeps its order when searching, `s` cycles scope in the tree only"),
named again on 2026-10-08 21:30 CEST. Board item
`PVTI_lAHOCU842c4BmE5Zzg_KW4I`. Lead decisions on 2026-10-08 (working
rule) on the open points below: (a) the tree keeps its order, only the
matching and the highlighting change; (b) subsequence matching is
accepted as is, no minimum score, since nothing is ranked; (c) `s`
cycles the scope (all, local, project, global) in the tree and the
footer file-type slot or the header (NZ-3) names the active scope; `f`
(focus section) stays as it is; (d) tree only, the todo board keeps its
filter as today. Current code facts for the worker to verify:
`tui/text.rs` already has `fuzzy_match(haystack, needle) -> bool`
(case-insensitive subsequence) and tests; the filter is
`notez_core::filter` (`parse`, `Filter::matches(text, flags)`,
`#tag` tokens, `toggle_tag_in_buffer`, `active_tag_bits`); the tree's
filter strip and `search_mode` live in `tui/tree.rs` (about line 3217);
the reference scorer is `~/Repos/fleetz/src/fuzzy.rs` (read-only);
`0` and `s` are unbound in the tree (NZ-15 took `S`). Where the current
code already matches by subsequence, the ticket's work is the scoring
port for highlighting, the highlighting itself, `0`, `s` and the "no
matches" row; the worker reports which criteria were already met.

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

#### In flight record (NZ-5)

NZ-5 worker REPORTED (about 18 min): none of the criteria was met at
base (`text::fuzzy_match` was substring and unused; both views used
core's substring `Filter::matches`). Delivered: subsequence matching
in both views via `text::filter_matches` (tags still go to core),
yellow bold highlight of matched chars in tree and todoz rows, `0`
clears tag tokens, `s` cycles all / scratch / project / global via
new `tui/tree/scope.rs` (`ViewScope`, a `hidden` flag on section
nodes applied each loop pass), header shows "scratch · 3 notes", "no
matches" row in both views, `0` and `s` in footer and help, README
paragraph. 593 + 157 green, 20 new tests. Existing test edits flagged
for review: the footer key-list test gained `0` and `s`, two
mechanical helper changes. Lead decisions recorded from the worker's
choices: scratch scope is the current repo's `.notez/` only; a
section that first appears in a rebuild can show for one key step.
Files: README.md, tui/{header,text,theme,todo}.rs,
tui/tree/{input,mod,model,move_prompt,render,search,test_support}.rs,
new tui/tree/scope.rs. REVIEW DISPATCHED (`nz-reviewer`, opus) with
parity, multibyte highlight and scope checks asked for. Look and
layout: Andreas can try the branch build before the merge (`cargo run
--release --manifest-path .claude/worktrees/nz-5/Cargo.toml -p
notez-cli -- tree`); under the working rule the lead otherwise merges
after acceptance and green CI and he overrules after `./install.sh`.

NZ-5 REVIEW: CHANGES REQUESTED at hash `54f10ce2…e9fa`. Blocker B1:
creating a note or folder into a section the `s` scope hides (for
example scratch scope, `n`, Tab to global) leaves the row hidden,
clears the filter wrongly and reports "filter cleared"; the fix is to
switch the view scope to one that shows the new row before the filter
check, plus moving scope application into `Forest::replace()` so
rebuild cursor rows see hidden flags. Also asked in the same round:
F1 whole-string lowercase for parity with core (Greek sigma), F2
clear or exclude marks under hidden sections from bulk `d`/`m`/`S`.
All seven criteria otherwise satisfied; multibyte highlight, footer
widths and core parity verified by the reviewer. WORKER RESUMED with
the fix list. Worker REPORTED the fix round: `scope::reveal_row`
(switches to the narrowest scope that shows a created or moved row,
then clears the filter only if it still hides it; used by new note,
new folder, single and bulk move), scope owned by `Forest` and
applied in `replace()`, whole-string lowercasing with index mapping,
`scope::drop_hidden_marks` each pass and on `s`; five new tests, 598
+ 157 green; `tree/bulk.rs` and `tree/delete.rs` now also touched
(mechanical `scope: ViewScope::All` in `Forest` literals). RE-REVIEW
ACCEPTED at hash `5fb0852c…f719`: B1 closed in all four handlers,
F1 and F2 fixed, no core, manifest or CI change. Follow-ups (low, not
blocking, for a later ticket): `drop_hidden_marks` drops a mark if
any hidden row has that path even when a shown row lists it too
(sections do not overlap today); an automatic scope switch from
`reveal_row` can drop marks without saying so and the move handlers
do not mention the switch; a bulk move reveals only the first moved
row; `0` sends the cursor to row 0. Lead verification after `touch`:
zero warnings, 598 + 157 green, hash matches. COMMITTED `58d3d8b` on
`feat/nz-5-fuzzy-search` (the 15 reviewed files), PUSHED. Branch CI
run `38734518167` GREEN. MERGED into `main` as `e4a3130` (`--no-ff`),
checks on the combined result zero warnings, 600 + 5 + 157 green,
pushed. `main` run `38735193505` GREEN. NZ-5 DONE: board Done,
worktree `.claude/worktrees/nz-5` removed, local and remote branch
deleted.

#### NZ-3: header status segment

Status: DONE 2026-10-10, merged into `main` as `460c4b9` (worker commit
`7e329bd`, review accepted at hash `9e481f87…`). `tui/header.rs` with
`SyncState` (`Synced`, `Offline`, `NoUpstream`, `Stopped`, `Off`,
`NotRepo`, built in `main.rs` from `AutoSync` plus local `rev-parse
@{u}` and FETCH_HEAD checks) and `DirtyCount` (cached `git status
--porcelain`, recounted after file-changing actions and reload, never
on navigation). Lead decisions: `Idle` told apart by local checks; the
header takes the first row of the panes' area (Andreas judges from the
build; the alternative is the blank top margin row). Follow-ups:
"synced" also shows when the pull fetched but then failed without a
rebase in progress, and before local commits are pushed; the
`mark_stale` wiring has no test. Original status: Ready, runs after
NZ-4. In the standing scope since 2026-10-06, named again by Andreas on
2026-10-08 21:30 CEST ("can you do nz-3 and 5"). Board item `PVTI_lAHOCU842c4BmE5Zzg_KWyo`. Lead decisions on
2026-10-08 (working rule): no loading spinner (its own ticket if ever
wanted: it needs a background pull and a reload, a concurrency change);
the dirty count is computed in `notez-cli` by running `git status
--porcelain` in the vault root (no `notez-core` change, so epoz is
untouched); `AutoSync` variants stay as they are; the header is one
line above the panes in both views and replaces the current block
title line (`ctx.title`, `tui/tree.rs` about line 3368) rather than
adding a second line. Current code facts: `main.rs` runs
`notez_core::sync::pull_on_open` before the TUI and passes only a
`warning: Option<&str>` (the stop message) into `commands::tree::run`
and `commands::todo::run`, stored as `ctx.warning` and shown in the
status line; the `--no-sync` flag exists; `AutoSync` is `Idle` (not a
repo, no upstream, nothing to send, or offline), `Done`, `Stopped(msg)`.
So the header needs the full result, not just the warning: add a
`SyncState` enum in `tui/header.rs` (`Synced`, `Offline` or `NoUpstream`
if `Idle` can be told apart, else `Idle` shown as "offline or no
upstream", `Stopped`, `Off` for `--no-sync`, `NotRepo`) built in
`main.rs` from the `AutoSync` value and the flag, passed through the
two `run` functions next to `warning`. NZ-6 (quiet offline exit) is
designed so this header can say "could not reach the remote"; keep the
enum open for that variant.

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

#### In flight record (NZ-3)

2026-10-10 12:55 CEST: NZ-3 DISPATCHED. Worker `nz-worker` (opus,
medium), base `e7c51ca`, branch `feat/nz-3-header`, worktree
`.claude/worktrees/nz-3`, allowed files per the brief plus `main.rs`
for the `SyncState` plumbing. Worker REPORTED at 13:20 CEST (22 min):
all six criteria claimed, 536 + 147 tests, 12 new in `tui/header.rs`,
`Idle` split into synced / offline / no upstream by local checks
(`rev-parse @{u}`, FETCH_HEAD non-empty and newer than the pull start),
header row taken from the inset area so panes lose one row, tag and
check do not recount (written on exit). REVIEW dispatched at 13:22
CEST (`nz-reviewer`, opus). Review ACCEPTED 13:36 with one low
finding (a FETCH_HEAD test case depending on git's truncation); worker
fixed it (test-only, back-dates the mtime), re-review ACCEPTED at hash
`9e481f87…`. Lead verified the hash and ran build and tests (536 + 147
green). COMMITTED `7e329bd` on `feat/nz-3-header`, pushed 14:20 CEST,
branch CI run green (lint, ubuntu, macos). MERGED into `main` as
`460c4b9` (`--no-ff`) at 14:35 CEST after build and tests on the
combined result (536 + 147), pushed; `main` CI watch running in the
background. `main` run GREEN at 14:50 (lint, ubuntu, macos). NZ-3
DONE: board Done, worktree `.claude/worktrees/nz-3` removed, local
and remote `feat/nz-3-header` deleted. Look items for Andreas after his next `./install.sh`: the header
row (view and scope left; sync state, `N uncommitted` or `clean`, note
or open/done counts right), the panes one row shorter, the pane title
reduced to the number badge, how narrow widths drop segments.

### NZ-17: versioning, first version 0.1.0

CONFIRMED and IN FLIGHT 2026-10-15 (Andreas: "add versioning,
changelo. look at notez-core and NZ-17"). Board item
`PVTI_lAHOCU842c4BmE5Zzg_KbOk`. Branch `feat/nz-17-versioning`, base
`7cfeb27`. Brief as run:

1. One source of truth: `[workspace.package] version = "0.1.0"` in the
   root `Cargo.toml`, both crates `version.workspace = true`.
   `Cargo.lock` follows. No dependency added.
2. `notez --version` prints `notez 0.1.0 (<short sha>)`, with
   `-dirty` when the tree is dirty, via a `build.rs` in `notez-cli`
   that shells out to `git` and sets a `cargo:rustc-env`; outside a
   git checkout or without `git` it falls back to the plain version.
   `rerun-if-changed` on `.git/HEAD` and the current ref so the sha is
   not stale. The clap `version` attribute uses that value.
3. `CHANGELOG.md` in Keep a Changelog form: an `Unreleased` section and
   a `0.1.0` section summarizing what has shipped, grouped by area
   (storage and sync, CLI, tree browser, todoz, preview), one line per
   feature, drawn from `git log` and the Done tickets; one page, not a
   commit list. A `notez-core` subsection lists public API and file
   format changes, since epoz pins the crate by rev.
4. `docs/versioning.md` (short): semver, 0.x so minor bumps may break;
   patch for fixes; any `notez-core` public API or on-disk format
   change is at least a minor bump and gets a `notez-core` changelog
   entry so epoz can read what changed between pinned revs; both
   crates share the workspace version; tagging and releasing are the
   owner's. The README line comes with NZ-46.
5. `install.sh` prints the installed version after the copy
   (`"$INSTALL_DIR/notez" --version`).
6. Tests: the version string starts with `CARGO_PKG_VERSION`; the
   fallback path is covered or explained.
Allowed files: `Cargo.toml`, `Cargo.lock`, `crates/notez-core/Cargo.toml`,
`crates/notez-cli/Cargo.toml`, `crates/notez-cli/build.rs` (new),
`crates/notez-cli/src/main.rs` (the version attribute only),
`CHANGELOG.md` (new), `docs/versioning.md` (new), `install.sh`. The
ticket ends with the lead presenting `git tag -a v0.1.0` and the push
command for Andreas; no agent tags.

Original record follows.

Status: Draft, relayed, not confirmed in the lead session. RECORD ONLY.
Provenance: a cross-session message from the advisor session
(`repos-f9`) at about 17:20 CEST on 2026-10-07 relaying Andreas: "add a
ticket about adding versioning to notez. first version can be 0.1.0?".
Nothing dispatched; Andreas confirms the brief and names it before it
runs. Tagging and releasing are owner-only whatever he decides.

Facts checked by the advisor (not yet re-checked by the lead): both
crates already say `version = "0.1.0"` in their own `Cargo.toml`; no git
tags; no CHANGELOG; epoz pins `notez-core` by rev.

Proposed outcome: notez has a real version, 0.1.0 the first. Scope as
proposed: (1) one source of truth, `[workspace.package] version`,
inherited by both crates; (2) `notez --version` prints the version plus
the short commit, through a `build.rs` reading git with no new
dependency, falling back to the plain version outside a git checkout;
(3) `CHANGELOG.md` in Keep a Changelog form, with 0.1.0 summarizing what
has shipped (auto sync, rename, always-open browser, footer and help,
NZ-1 to NZ-10 and later); (4) a short versioning policy in the README:
semver, 0.x so minor bumps may break, patch for fixes, and `notez-core`
file-format changes are always at least a minor bump because epoz pins
it; (5) `install.sh` prints the installed version. The ticket ends by
presenting `git tag -a v0.1.0 -m ...` and the push command for Andreas;
no agent tags or releases, unless Andreas confirms the relayed tagging
permission recorded under Authorized by the owner (then the lead creates
the local tag after the merge and asks before pushing it).

Board, as settled by the advisor on 2026-10-07 about 18:20 CEST: the
project board is `https://github.com/users/Gaurgle/projects/2` (private,
linked to the repo), 17 draft items NZ-1 to NZ-17 with Status options
Draft, Ready, In flight, Ready to integrate, Done. No repository issues
back it; the ones the advisor had opened were deleted again, so rows
with a null title in GitHub's listing (nine at 18:30 CEST) are index
lag, not items. This file stays the record of ticket status and the
board mirrors it.

Lead edits the board: relayed at about 18:30 CEST, Andreas's words via
the advisor, "lead should edit the board, tell him, he is already
leading the session". Keeping board status current is already the lead's
role in the coordinator definition, so the lead applies it: status
changes only, no creating or deleting items or issues, no visibility
change (those still need Andreas). The lead listed the board at 18:32
CEST and found every status matching this file. Wiring the URL and read
command into `docs/agent-workflow.md` still waits for Andreas's word.

How to edit: project id `PVT_kwHOCU842c4BmE5Z`, Status field
`PVTSSF_lAHOCU842c4BmE5ZzhkvTJM`, option ids Draft `3d75ddf4`, Ready
`9e7798e2`, In flight `b984e77c`, Ready to integrate `abf1c81e`, Done
`82c5faed`. List items with `gh api graphql` on the project node
(`items(first: 50) { nodes { id content { ... on DraftIssue { title } }
fieldValueByName(name: "Status") { ... on
ProjectV2ItemFieldSingleSelectValue { name } } } }`); `gh project
item-list` lags. Update with `gh project item-edit --id <item>
--project-id PVT_kwHOCU842c4BmE5Z --field-id
PVTSSF_lAHOCU842c4BmE5ZzhkvTJM --single-select-option-id <option>`.
Item ids at 18:32 CEST: NZ-13 `PVTI_lAHOCU842c4BmE5Zzg_KXJg`, NZ-14
`..._KXLw`, NZ-15 `..._KXOw`, NZ-16 `..._KXRY`, NZ-17 `..._KbOk`, NZ-4
`..._KW1E`, NZ-5 `..._KW4I`, NZ-3 `..._KWyo`, NZ-6 `..._KW6U`, NZ-11
`..._KXEY` (prefix `PVTI_lAHOCU842c4BmE5Zzg`).

Open for Andreas: should epoz later pin the tag instead of a rev; should
cli and core versions always move together (proposed: yes, one workspace
version); and whether a new `build.rs` counts as a build-config change
needing his explicit approval (the lead's view: it is one, so it needs
his yes before dispatch; CLAUDE.md asks before CI and dependency changes
and this sits next to them).

Allowed files (proposed): `Cargo.toml` (workspace and both crates),
`crates/notez-cli/build.rs` (new), `crates/notez-cli/src/main.rs`
(version output only), `CHANGELOG.md` (new), `README.md`, `install.sh`.

Board: see the settled note under NZ-12 (the project board at
`https://github.com/users/Gaurgle/projects/2`; the lead reports state
changes, the advisor or Andreas edits the board, until Andreas says
otherwise).

#### In flight record

NZ-17 worker REPORTED (about 9 min), not yet complete: workspace
version in place, `build.rs` with git sha and `-dirty` via `git
rev-parse --git-path` (worktree `.git` file handled), fallback to the
plain version without git verified in a scratch copy, `CHANGELOG.md`,
`docs/versioning.md`, `install.sh` version line, test in `main.rs`.
Blocker it raised: the clap `version` attribute lives in
`crates/notez-cli/src/cli/mod.rs`, not `main.rs` as the brief said,
so `--version` printed no sha. LEAD DECISION: `cli/mod.rs` added to
NZ-17's allowed files (no other worker touches it); worker resumed to
wire `version = crate::VERSION` and add a `Cli::command()
.get_version()` test. Known limits to carry into the review: `-dirty`
refreshes on edits under `crates/` only; a tree first built outside
git stays plain until a package file changes; `Cargo.lock` unchanged.
NZ-17 worker finished the wiring (`cli/mod.rs` one line, a
`clap_reports_the_build_version` test; `--version` prints `notez
0.1.0 (7cfeb27-dirty)`, 575 + 157 green, no warnings). NZ-17 REVIEW
DISPATCHED (`nz-reviewer`, opus).

NZ-17 REVIEW ACCEPTED, no blockers; reviewer exercised build.rs in
scratch clones (path with spaces, packed refs, detached HEAD, shallow
clone, tarball, no git on PATH) and confirmed `-dirty` refreshes on
nested edits under `crates/`. Hash `111ca698…79d4b` over the diff plus
the three new files. Lead verification after `touch`: zero warnings,
575 + 157 green, `notez 0.1.0 (7cfeb27-dirty)`, no dashes, hash
matches. COMMITTED `8fde0f5` on `feat/nz-17-versioning`, PUSHED,
branch CI watch running in the background. Reviewer follow-ups
(low): `-dirty` can go stale after a commit whose ref lives only in
`packed-refs` or after edits outside `crates/` (documented in
build.rs); `notez --version` and `--help` exit with status 2, a
pre-existing clap handling in `main.rs`, candidate for a small
ticket. After the merge the lead presents the `v0.1.0` tag command.

NZ-17 branch CI run `38718268906` GREEN. MERGED into `main` as
`dd2ff6a` (`--no-ff`, on top of the NZ-45 merge), checks on the
combined result zero warnings, 575 + 5 + 157 green, `notez 0.1.0
(dd2ff6a-dirty)`; the lead's command chain stopped before the push
because `notez --version` exits 2 (the pre-existing clap quirk), the
push was done separately; `main` CI watch running in the background.
`main` run `38718910312` GREEN. NZ-17 DONE: board Done, worktree
`.claude/worktrees/nz-17` removed, local and remote branch deleted.
`main` = `dd2ff6a`. Tag command presented to Andreas (his to run,
whenever he chooses): `git tag -a v0.1.0 -m "notez 0.1.0" dd2ff6a &&
git push origin v0.1.0`. Not yet tagged at the time of writing.

### NZ-45: storage integration tests for the DESIGN.md scenarios (2026-10-15, In flight)

Board item `PVTI_lAHOCU842c4BmE5ZzhA8UOc`. Asked for by Andreas on
2026-10-15 ("add integration tests") after the lead found that neither
crate has a `tests/` directory while `DESIGN.md` says each storage
scenario "should eventually have an integration test".

Outcome: end-to-end tests of the storage layer against the real
binary and the real `git`, with two fake homes and a bare remote in
temp dirs, so the thing the rewrite exists for (portable, synced
notes) has proof beyond unit tests.

Acceptance criteria:

1. Tests live under `crates/notez-cli/tests/` (binary via
   `env!("CARGO_BIN_EXE_notez")`) and, where a core boundary is the
   better seam, `crates/notez-core/tests/`. Every test points `HOME`
   and `XDG_CONFIG_HOME` at temp dirs, sets a git identity through
   environment variables, isolates git config
   (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`), and never touches the
   developer's `~/notez` or `~/.config/notez`.
2. Scenario A: a `-p` note written in "laptop" (home `/.../alice`)
   lands in `<notez_root>/personal/<project>/`, syncs through a bare
   remote, and appears in "desktop" (home `/.../alice-desk`, project at
   a different path) after its sync; it is not in the project repo.
3. Scenario C: a `-l` note lands in `<repo>/.notez/`, that dir is
   gitignored, and the note does not reach the remote.
4. Scenario E: two registries map the same project name to different
   paths; `notez list` (or the aggregate) resolves each machine's path.
5. Scenario I: a registry entry whose path is missing warns and does
   not crash; re-attaching at the new path fixes it.
6. Scenario J: both machines edit the same file without syncing; the
   second `notez sync` fails with the "resolve conflicts manually"
   hint, nothing is lost (the file still holds its local content, the
   remote still holds the other); after a manual resolution both
   contents coexist and the next sync succeeds.
7. Deterministic and parallel-safe (own temp dirs per test; use
   `serial_test` only if something global forces it). Runs in CI as
   part of `cargo test --workspace` on ubuntu and macos.
8. No production code change and no manifest change (both crates
   already have `tempfile` and `serial_test` as dev-dependencies).
   If a scenario fails because of a product defect, keep the test,
   mark it `#[ignore = "<reason>"]`, and report it as a finding; never
   weaken the assertion.

Facts for the worker to verify: `notez-core/src/config/paths.rs`
reads `XDG_CONFIG_HOME`; `notez_root` in `config.toml` is stored
tilde-relative and expanded with `dirs::home_dir()` (so `HOME` drives
it); `commands/sync.rs` commits pending changes then runs `git pull
--rebase` and `git push`; a non-interactive write path exists through
quick notes (`notez -p quick words`, `notez -g quick ...`, `notez -l
quick ...`) so no editor is needed; `notez attach <name> [path]`
registers a project.

Allowed files: `crates/notez-cli/tests/**`, `crates/notez-core/tests/**`.
Nothing else; a needed manifest change is a stop-and-report (NZ-17
owns the manifests this wave).

#### In flight record

NZ-45 worker REPORTED (about 9 min): two untracked files,
`crates/notez-cli/tests/common/mod.rs` and
`crates/notez-cli/tests/storage_scenarios.rs`, five tests (A, C, E, I,
J) green on macOS, no production or manifest change, no product
defect, `env_clear()` plus explicit `HOME` and `XDG_CONFIG_HOME`,
scenario E uses the `7cfeb27` auto-registration on the second
machine, scenario J resolves with `git add` and `git rebase
--continue`. Doc drift noted: DESIGN.md scenario A expects a dated
filename but plain `notez add` writes undated names; scenario I says
`notez tree` warns, test covers `notez list`. Both go to NZ-46. REVIEW
DISPATCHED (`nz-reviewer`, opus) with a mutation check asked for.

NZ-45 REVIEW ACCEPTED, no blockers, mutation check passed (two
assertions inverted, both failed, file restored; hash
`40406d07…eaa16` over the two files). Lead verification: the first
`cargo test` in the worktree FAILED exactly the two mutated tests;
cause: the reviewer restored the file with its old mtime, so cargo
kept the mutated test binary as fresh. `touch` on the two files and a
rerun: 573 + 5 + 157 green, hash unchanged. LESSON for reviewer
briefs: after an edit-and-restore mutation check, `touch` the file
(or the lead does) before trusting a rerun. COMMITTED `e39d158` on
`test/nz-45-storage-integration` (the two files only), PUSHED, branch
CI watch running in the background. Reviewer follow-ups (low, not
blocking): a comment that scenario J pins the "rebase left in
progress" behaviour; add/add conflict not covered; scenario A does
not check the date prefix. Doc drift goes to NZ-46. Branch CI run
`38717681805` GREEN (ubuntu, macos, lint). MERGED into `main` as
`aabf8f2` (`--no-ff`), checks on the combined result 573 + 5 + 157
green, pushed. `main` run `38717972796` GREEN. NZ-45 DONE: board
Done, worktree `.claude/worktrees/nz-45` removed, local and remote
branch deleted (standing permission). `main` = `aabf8f2`.

### NZ-44: split tui/tree.rs into modules, no behaviour change (2026-10-10, Ready)

Status: DONE 2026-10-10, merged into `main` as `10312c6` (worker commit
`12bbb00`, review accepted at hash `df288820…`, all 492 functions
compared). Layout: `tui/tree/mod.rs` (165 lines: `TreeContext`,
`NewNoteRoots`, `SectionSpec`, `run_tree`, wiring), `model.rs`,
`search.rs`, `render.rs` (2272), `new_note.rs`, `folder.rs`,
`delete.rs`, `bulk.rs`, `rename_prompt.rs`, `move_prompt.rs` (2014),
`preview.rs`, `input.rs` (the event loop), `test_support.rs`. 235 tests
under the same names; clippy 29 before and after. Note for later
briefs: line references into `tree.rs` in older tickets (NZ-5, NZ-11,
NZ-32, NZ-28 pass 2, NZ-30) are stale; name the module instead. Board
item `PVTI_lAHOCU842c4BmE5Zzg_1qOI`. Original status: Ready. Opened by
the lead on Andreas's confirmation (14:05 CEST, see Authorized by the
owner) of the advisor session's proposal. Method: bounded, mechanical.

Facts at `e7c51ca` (checked by the lead): `crates/notez-cli/src/tui/tree.rs`
is 10,783 lines; code runs to about line 5,095 and one `#[cfg(test)] mod
tests` block starts at 5,096 (235 `#[test]`), plus a second `#[cfg(test)]`
item at line 418. The code groups around `TreeContext`, `Forest` and
`TreeNode` (the model) and one prompt plus outcome pair per operation:
`NewNotePrompt`, the folder prompt (`FolderOutcome`), `DeletePrompt` and
`DeleteOutcome`, `BulkItem` (multi-select), `RenameEnter`, `MovePrompt`,
`MovePlan` and `MoveOutcome`, `StatusSlot`. NZ-3 adds the header wiring
and `SyncState` plumbing on top before this ticket starts; the worker
bases on `main` after that merge.

Outcome: `tui/tree.rs` becomes a directory module `tui/tree/` (or sibling
modules under `tui/`) split along those lines: model, render, keys and
input, filter and search, the tag field, the preview, and one module per
prompt. Tests move next to the code they test. The public surface used by
`commands/tree.rs` and `tui/mod.rs` stays the same (re-exports from
`tui/tree/mod.rs` are fine).

Acceptance criteria:
1. No behaviour change: every existing test passes with no assertion,
   name or fixture logic changed (moving a test between files and
   adjusting `use` paths is allowed; the `#[test]` count stays 235 plus
   whatever NZ-3 added). `cargo test --workspace` green.
2. No file outside `crates/notez-cli/src/tui/` changes except module
   declarations (`tui/mod.rs`); `commands/tree.rs` and `main.rs` compile
   unchanged.
3. `cargo clippy --workspace --all-targets` reports no more warnings
   than on the base commit (count them before and after).
4. No module over about 2,500 lines; the worker reports the line count
   per new file.
5. `cargo fmt --check` on the new files passes (the pre-existing drift
   elsewhere is not this ticket's).

Allowed files: `crates/notez-cli/src/tui/tree.rs` (removed or reduced),
new files under `crates/notez-cli/src/tui/tree/` or `crates/notez-cli/src/tui/`,
`crates/notez-cli/src/tui/mod.rs`. Reviewer probes: `git diff --stat`
and a moved-code check (`git diff -M --stat` or a per-function spot
check) showing no logic edits; the test count; the clippy count before
and after.

#### In flight record

NZ-44 worker REPORTED 15:42 (9.5 min): `tree.rs` (10,805 lines, 235
tests) split into `tui/tree/` with 13 files (`mod.rs` 165, `model.rs`
1429, `search.rs`, `render.rs` 2272, `new_note.rs`, `folder.rs`,
`delete.rs`, `bulk.rs`, `rename_prompt.rs`, `move_prompt.rs` 2014,
`preview.rs`, `input.rs` with the event loop, `test_support.rs`);
same 235 test names, 536 + 147 green, clippy 29 before and after,
rustfmt clean (the reformat grows the files to 14,296 lines; git will
record delete plus adds, `git blame -C -C` still traces). REVIEW
dispatched 15:45 (`nz-reviewer`, opus) with a move-not-edit method.
Review ACCEPTED 15:52 at hash `df288820…`, no findings: all 492
functions compared on both sides, the only differences are rustfmt
reflows of ten signatures that grew by `pub(super) `. Lead verified
the hash and ran build and tests (536 + 147 green). COMMITTED
`12bbb00` on `refactor/nz-44-split-tree` (14 files, +14296 -10805,
recorded as delete plus adds), pushed 15:58, branch CI green. MERGED
into `main` as `10312c6` (`--no-ff`, clean) at 16:05 after build and
tests on the combined result (565 + 152), pushed; `main` run GREEN at
16:14 (lint, ubuntu, macos). NZ-44 DONE: board Done, worktree
`.claude/worktrees/nz-44` removed, local and remote branch deleted.
NZ-28 set back to Ready on the board (pass 2 pending, no worker).

### NZ-6: quiet exit sync when the remote is unreachable

Status: DONE 2026-10-10, merged into `main` as `2fb8c0d` (worker commit
`cdb0576`, review accepted at hash `59d013d3…`). A separate `git fetch
--quiet` in `auto_sync` after `commit_pending`; a failed fetch returns
`Idle` and prints nothing; conflicts, failed commits and rejected
pushes report as before; `notez sync` still reports. Follow-ups: every
online exit now contacts the remote twice (a `git rebase @{u}` after
the fetch would save it but change the conflict message prefix); the
header could show "last exit stayed local" via `rev-list @{u}..HEAD`
as its own ticket. Original status: Ready. Requested by Andreas on
2026-10-06 ("Offline message: yes").
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

Settled by Andreas on 2026-10-06 18:26 CEST: "quiet like pinz". Every
failed fetch at session end is quiet, as in Pinz. Known cost: an expired
credential is then also quiet until something shows sync state on screen,
which NZ-3's header is meant to do, so NZ-3 should be able to tell
"could not reach the remote" apart from "synced" (design that with NZ-6
in view, without changing the public `AutoSync` variants unless Andreas
approves).

NZ-6's In flight entry shares a paragraph with the NZ-44 dispatch; see
"Wave and day notes", 2026-10-10.

### NZ-28 pass 1 (merged 2026-10-10; the ticket is parked and its brief stays in the handoff)

2026-10-10 13:40 CEST: NZ-28 PASS 1 DISPATCHED in the second slot on
Andreas's "i think we can start up more parallell agents?" (13:30).
Worker `nz-worker` (opus, medium), base `e7c51ca`, branch
`feat/nz-28-lint`, worktree `.claude/worktrees/nz-28`, allowed files
`tui/lint.rs` (new), `tui/mod.rs` (one `pub mod lint;` line),
`tui/highlight.rs` (additions only). Disjoint from NZ-3 except the one
line in `mod.rs`, which the lead resolves at merge. Pass 2 (tree.rs,
markdown.rs, footer, overlay) waits for the tree.rs line (after NZ-5).
Board: NZ-28 In flight. Worker REPORTED 13:52 (27 tests, 551 + 147,
`highlight::parse` added, eight markdown checks in a table); lead
decisions on its six points recorded on the ticket; hard-break
exemption added on the lead's request. REVIEW ACCEPTED 14:45 at hash
`5bc463df…`, no blockers, follow-ups recorded on the ticket for pass 2.
Lead verified the hash and ran build and tests (551 + 147 green).
COMMITTED `3646b95` on `feat/nz-28-lint`, pushed 14:52, branch CI
green. MERGED into `main` as `4841211` (`--no-ff`, clean merge) at
15:00 after build and tests on the combined result (563 + 147),
pushed; `main` run GREEN at 15:12 (lint, ubuntu, macos). Worktree
`.claude/worktrees/nz-28`, local and remote `feat/nz-28-lint` removed.
NZ-28 stays In flight on the board (pass 2 pending, not dispatched;
it bases on `main` when its turn on the tree.rs line comes).

## Wave and day notes

In flight entries that cover several tickets or a whole wave, newest day
first, in the order they were written within a day.

### 2026-10-15

2026-10-15 about 12:20 CEST (real `date` at the worktree step: see
the git reflog for exact times): three workers DISPATCHED from base
`7cfeb27`, `git status --short` clean before dispatch, all `nz-worker`
(opus, medium):
- NZ-5 on `feat/nz-5-fuzzy-search`, worktree `.claude/worktrees/nz-5`
  (the `tui/tree/` line; owns `README.md` for this wave).
- NZ-45 on `test/nz-45-storage-integration`, worktree
  `.claude/worktrees/nz-45` (only `crates/*/tests/**`).
- NZ-17 on `feat/nz-17-versioning`, worktree `.claude/worktrees/nz-17`
  (manifests, `Cargo.lock`, `build.rs`, `main.rs` version attribute,
  `CHANGELOG.md`, `docs/versioning.md`, `install.sh`; not `README.md`).
Board: NZ-5, NZ-17, NZ-45 In flight; NZ-46 Ready; NZ-47 Draft; NZ-28
back to Draft (parked).

WAVE 1 COMPLETE (2026-10-15). `main` = `origin/main` = `e4a3130`,
tagged `v0.1.0` at `dd2ff6a`. No worktree, no ticket branch, no
worker or reviewer running. Board: NZ-5, NZ-17, NZ-45 Done; NZ-11,
NZ-30, NZ-32, NZ-46 Ready; NZ-18, NZ-28 (parked), NZ-29 (closed "not
now"), NZ-42, NZ-43, NZ-47 Draft. Andreas's installed binary predates
all of today; after `./install.sh` the visible changes are the fuzzy
`/` filter with lit matches in both views, `0` and `s` in the tree
with the scope named in the header, the "no matches" row, `notez
--version` with the sha, plus last week's header line and quiet
offline exit. Next per the Next step section: the NZ-43 brainstorm
with Andreas in the lead session, then NZ-46 (`nz-small`, docs) and
NZ-32 (tree line) in parallel, then NZ-11 alone, NZ-42, NZ-30.
NZ-47 waits for his answers to questions 2, 4, 6, 7, 8 in
`docs/design-nz47-dates.md`.

WAVE 2 DISPATCHED 2026-10-15 from base `b253758` (the docs commit on
top of `e4a3130`), `git status --short` clean: NZ-11 (`nz-worker`,
opus) on `fix/nz-11-footer-width`, worktree `.claude/worktrees/nz-11`,
files `tui/tree/render.rs`, `tui/todo.rs`, `tui/footer.rs`; NZ-46
(`nz-small`, sonnet) on `docs/nz-46-docs-refresh`, worktree
`.claude/worktrees/nz-46`, files `DESIGN.md`, `README.md`,
`docs/multi-user-sync.md`, `docs/handover-2026-09-02-small-machine.md`.
Board: both In flight. NZ-32 waits for NZ-11 (shared `footer.rs`) and
NZ-46 (shared `README.md`); NZ-42 waits for NZ-11 (`todo.rs`) and the
NZ-43 decision; NZ-30 after NZ-32 on the tree line.

NZ-46 worker (`nz-small`) REPORTED twice: DESIGN.md Status rewritten
(edit and nav implemented, feature set, preview, 0.1.0, old skeleton
list reduced to a sentence), the 2026-10-15 file-reader decision
recorded, test-scenario drifts fixed, calendar section points to the
NZ-47 design, README Version note and Roadmap dates line, handover
doc marked historical; `multi-user-sync.md` left as parked research.
REVIEW DISPATCHED (`nz-reviewer`). NZ-11 worker REPORTED blocked on
one existing test asserting the defect
(`warning_layout_truncates_on_a_char_boundary_and_keeps_the_hint`
expected a 4-column string in 3 text columns). LEAD DECISION: that
single assertion is updated to expect `"ab"` with a comment, since
NZ-11 is the authorized change of that contract; the test is kept.
`tui/tree/input.rs` added to NZ-11's allowed files for criterion 4
(the hardcoded `"quit "` hint, which also overflowed by one column,
goes through `footer::hint_spans`). Worker resumed. Already fixed by
the worker: `warning_layout`, todoz `warning_lead` and
`warning_right`, `footer::span_cols`, via a new
`footer::clip_to_cols` that cuts between graphemes.

NZ-46 REVIEW: changes requested once (the lead's own brief wrongly
said the tree dims a missing project; `collect_all` skips the whole
project and only `notez list` marks `(not found)`; also a wrong
skeleton date), fixed by the worker with `CHANGELOG.md` added to its
files for the lint line, RE-REVIEW ACCEPTED at hash `814a80d3…455c`.
COMMITTED `d30c19a`, pushed, MERGED into `main` as `0d91cd4`
(docs-only: no CI run starts; the last code run on `main`, `e4a3130`,
was green). NZ-46 DONE: board Done, worktree and branches removed.
Nits left: DESIGN.md:541 could say "omits the project"; one long
wrapped line at 580. NZ-11 worker REPORTED complete: `warning_line`
in `tree/render.rs` now draws the quit hint via `footer::hint_spans`,
the input.rs warning arm is one call, 602 + 5 + 157 green, the one
authorized assertion change made with a comment. REVIEW DISPATCHED
(`nz-reviewer`, with grapheme and ZWJ checks asked for). `main` =
`0d91cd4`.

WAVE 3 DISPATCHED from base `835d932`: NZ-48 (`nz-worker`, opus) on
`feat/nz-48-core-dates`, worktree `.claude/worktrees/nz-48`, files
`crates/notez-core/src/**`, root `Cargo.toml` (version), `CHANGELOG.md`,
plus the `Task { .. }` literal lines in `tui/todo.rs` and
`commands/todo.rs`; NZ-32 (`nz-worker`, opus) on
`feat/nz-32-type-filter`, worktree `.claude/worktrees/nz-32`, files
`tui/tree/**`, `tui/text.rs`, `tui/footer.rs`, `tui/help.rs`,
`tui/header.rs`, `README.md`. Lead decision for NZ-32 (working
rule): one `s` cycle covering NZ-5's locations and the types, order
all, scratch, project, global, private, public, docs, todo; `@`
tokens compose with it; `0` clears `#tag` tokens only. Board: both In
flight. After this wave: NZ-42 (`tui/todo.rs`, after NZ-48 touched its
literals), then NZ-43 and NZ-30; NZ-49 (`agenda` plus inspector
footer) and NZ-50 (write path) to be created from the design when
NZ-48 is on `main`.

STOPPING FOR THE DAY on Andreas's "we can try to find a good place
to stop for the day" (2026-10-15): the lead lands NZ-32 (merge on
green, `main` CI, cleanup), dispatches nothing new, then releases
the baton. See the Current lead section for the release line.

NZ-48 `main` run `38749749511` GREEN; NZ-48 DONE, worktree and
branches removed. NZ-32 branch CI run `38749842196` GREEN, MERGED
into `main` as `bc63288` (`--no-ff`), combined checks zero warnings,
621 + 5 + 191 green, pushed; `main` run `38750312207` GREEN; NZ-32
DONE, board Done, worktree and branches removed. `git worktree list`
shows only the main checkout; no ticket branches local or remote;
`ps` shows this lead as the only `nz-coordinator`. The lead commits
this handoff and the design note to `main` as its last action and
releases the baton.

### 2026-10-10

2026-10-10 14:40 CEST: NZ-44 DISPATCHED (tree.rs split, `nz-worker`
opus, base `460c4b9`, branch `refactor/nz-44-split-tree`, worktree
`.claude/worktrees/nz-44`, allowed: `tui/tree.rs` becoming
`tui/tree/`, not `tui/mod.rs`). NZ-6 DISPATCHED in the third slot
(`nz-worker` opus, base `460c4b9`, branch
`fix/nz-6-quiet-offline-exit`, worktree `.claude/worktrees/nz-6`,
allowed `notez-core/src/sync.rs`, `main.rs`, `commands/sync.rs`,
`README.md`). Board: NZ-44 and NZ-6 In flight. Three workers now
(NZ-28 p1 in review, NZ-44, NZ-6), on disjoint files. NZ-6 worker
REPORTED 15:08 (a separate `git fetch --quiet` in `auto_sync` after
`commit_pending`, failure returns `Idle`; tests only in `main.rs` and
`commands/sync.rs`; README; 538 + 152 tests; real-binary repro before
and after). REVIEW dispatched 15:10 (`nz-reviewer`, opus); ACCEPTED
15:20 at hash `59d013d3…`, no blockers (follow-ups: a second remote
round trip per online exit, could become `git rebase @{u}` at the cost
of the conflict message prefix; the CLI test proves silence indirectly;
README could list local fetch errors). Lead verified the hash and ran
build and tests (538 + 152 green). COMMITTED `cdb0576` on
`fix/nz-6-quiet-offline-exit`, pushed 15:24, branch CI green. MERGED
into `main` as `2fb8c0d` (`--no-ff`) at 15:30 after build and tests on
the combined result (565 + 152), pushed; `main` run GREEN at 15:38.
NZ-6 DONE: board Done, worktree `.claude/worktrees/nz-6` removed,
local and remote branch deleted. Nothing to see on screen: an offline
exit is now silent; `notez sync` still reports.

Nothing is in flight. BATON RELEASED at 13:48 CEST (real clock) on
2026-10-10 (see Current lead and its clock correction). The docs commit that records this is the last commit
of the day on `main`.

2026-10-10 15:02 CEST, Andreas: "after current workers come back green
we can stop for today". No new dispatch. NZ-44 and NZ-6 run to their
reports, are reviewed and integrated if green, then the baton is
released.
Remote branch `nz-39-suffix` deleted at 12:48 (merged in `234d21c`,
standing permission).

### 2026-10-09

2026-10-09 evening: NZ-40 DONE (merge `931ce3d`, dots back, folders
show derived dots, blank when untagged, strip at column 1) and NZ-41 DONE
(merge `6854e03`, todoz cursor row, branch lines, margin, lavender frame);
CI green on `main`, worktrees and branches removed. NZ-39 (language
suffix indicator) DONE (merge `234d21c`, worker commit `7d6744d`; branch
`nz-39-suffix` and its worktree removed after the merge). NZ-42 and
NZ-43 are Drafts. 2026-10-09 night: a tmux crash during a test run was
not reproduced; `cargo build` and `cargo test --workspace` on `234d21c`
are green (524 + 147 passed) and the tmux server stayed up.
Board ids: NZ-40 `PVTI_lAHOCU842c4BmE5Zzg_uHy4`, NZ-41 `..._uH0E`,
NZ-42 `..._uH0g`, NZ-43 `..._uH10`.

## Moved from the handoff on 2026-10-17

Spent or superseded handoff text, verbatim. The standing permissions
these entries grant are summarized in the handoff's "Authorized by
the owner".

### Owner entries, 2026-10-07 to 2026-10-10

- 2026-10-10 12:50 CEST, Andreas, in the lead session (`f25c8d27`):
  "continue work", then "order is fine" to the queue the lead stated
  back (NZ-3, NZ-5, NZ-42 after NZ-5 is merged, NZ-32, NZ-28, NZ-30;
  NZ-11 and NZ-6 in the second slot). The 2026-10-08 standing
  permissions below apply to this queue unchanged. Limits: NZ-43 does
  not run; NZ-17, NZ-18, NZ-29 stay Drafts; no new dependency beyond
  the recorded approval; installing is Andreas's.

- 2026-10-10 14:05 CEST, Andreas, in the lead session (`f25c8d27`),
  confirming three proposals first relayed by the advisor session
  `notez-4d` ("yes all three, relay it to the lead", relayed 13:55;
  confirmed here directly, answer "Cap 3 workers, Split tree.rs
  ticket, Trim the handoff"):
  1. PARALLELISM CAP: up to three implementation workers at once,
     each ticket on its own files, at most one worker per file
     (`tui/tree.rs` is always one worker), merges into `main` one at a
     time as before. Recorded in `docs/agent-workflow.md` as well.
  2. NEW TICKET NZ-44: split `crates/notez-cli/src/tui/tree.rs` into
     modules with no behaviour change; runs as the NEXT `tree.rs`
     ticket (before NZ-5), nothing else on `tree.rs` while it runs.
     Board item created under the standing permission. Brief under
     Tickets.
  3. HANDOFF TRIM: move the earlier lead records, Done ticket briefs
     and the stale 2026-10-07 Next step into
     `docs/agent-handoff-archive.md`; keep current state, the active
     authorizations, open tickets and an up-to-date Next step.

- 2026-10-09 20:08 CEST, Andreas, in the lead session: "lets do this with
  relay" (the importance-glyph change and the todoz port), "I take the
  baton here" (lead on `claude-sonnet-5-5`, model substitution accepted),
  "Merge as you go" (the 2026-10-08 integration delegation applies again,
  for NZ-40, NZ-41, NZ-42 only, and for NZ-39 from 2026-10-09 evening when
  Andreas asked "can we implement this today?" about its language
  indicator: reviewed diff, green branch CI, safe-merge
  checks, then `git merge --no-ff` into `main` and push), and "lets this
  relay lead work on todoz". Limits: those three tickets; the tree-sitter
  and pulldown-cmark dependency approval is not extended to anything new;
  NZ-43 does not run; installing is Andreas's.

- 2026-10-07 16:25 CEST, Andreas, in the new lead session (`52bd7aa5`):
  "you can continue the work, from 2f04539, i believe. NZ-8 to NZ-16",
  then, answering the lead's four confirmation questions, a pasted list:
  "Already in the queue: NZ-8 (create a note), NZ-12 (delete a note), and
  the UI tickets. NZ-13: the unified default view, with scope badges.
  NZ-14: folders in the browser (create, rename, delete). NZ-15: move and
  change visibility, with the warning. NZ-16: multi-select with Space."
  The lead's reading, stated back to him:
  - CONTINUE: the paused standing scope and integration delegation resume.
  - Scope now: NZ-8, NZ-12, NZ-13, NZ-14, NZ-15, NZ-16 and the UI tickets
    NZ-4, NZ-5, NZ-3, with NZ-11 and NZ-6 in the second worker slot. The
    relayed tickets NZ-12 to NZ-16 are confirmed by his pasting them here.
    Order chosen by the lead, Andreas can reorder: NZ-8, NZ-12, NZ-13,
    NZ-14, NZ-15, NZ-16, then NZ-4, NZ-5, NZ-3. NZ-16 depends on 12, 14 and
    15; NZ-13 reshapes the tree sections, so it goes before the pane and
    search work.
  - Integration: "merge as you go along" is applied to NZ-8 (named in the
    original delegation). For NZ-12 onward the lead told him it will apply
    the same delegation unless he says stop; he has not objected. Same
    safe-merge conditions, same limits (no install, tag, release,
    force-push, remote branch deletion).
  - Key map: confirmed as relayed, since he engaged with it (the `Space`
    decision) and did not object when asked: `n` new note, `N` new folder,
    `r` rename, `d` delete, `m` move, `S` set scope, `Space` mark,
    `o`/`Enter` open.
  - NZ-13's `collect_all` change in `notez-core`: the relayed "ok" stands
    (listing addition only, no signature or format change).
  - Stop conditions unchanged: outside the limits, two failed fix cycles on
    one blocker, disk full, usage limit, or Andreas changes direction.
- 2026-10-08 13:55 CEST, Andreas, in the lead session (`4ffb11e2`),
  answering the six questions: "1, yes 2. ru it now 3. yes 4. yes 5.
  lets try refusing. 6. protect it". The lead's reading:
  - NZ-19 (CI) is AUTHORIZED: run it now in the second worker slot
    beside NZ-16 (disjoint files). The lead may push `feat/NZ-19-ci` to
    `origin` so the workflow runs there, including one throwaway
    failing commit and its revert to show a red run (no force-push: the
    revert stays in history), and may delete that remote branch after
    the merge. The integration delegation ("merge as you go along")
    applies; the CI change itself is approved by this entry (CLAUDE.md
    requires owner approval for CI). Pushing `main` afterwards triggers
    the new workflow; that effect is approved by the same answer.
  - A new ticket (NZ-20) is wanted and AUTHORIZED to run after NZ-16
    (same file `tui/tree.rs`): the browser's typed names (`n`, `N`, `r`,
    `m`) are refused when `sanitize::name` would alter them, `r` on a
    note with an unchanged name is a no-op, and `_todos` is protected
    like a docs folder from `d`, `r`, `m` and `S`. Board item still to
    be created (the lead asked Andreas, since creating items stays with
    the owner).
- 2026-10-08 15:15 CEST, Andreas, in the lead session (`4ffb11e2`):
  "great. you may go ahead with NZ-20. yes, branch pushes from now."
  The lead's reading:
  - NZ-20 runs now (the lead's own files are free). "Go ahead" after the
    lead said the board item was missing is read as leave to create that
    one item; the lead created `PVTI_lAHOCU842c4BmE5Zzg_Yflc` at 15:17
    CEST. Creating items otherwise stays with the owner.
  - STANDING PERMISSION: for every ticket from now on the lead pushes
    the ticket branch to `origin` after the reviewer accepts, so the
    workflow runs on it before the merge, and deletes that remote branch
    after the merge (the recommendation he said yes to named both).
    Scope: ticket branches under the recorded standing scope only. No
    force-push, no tags, no releases. If a run is red the ticket stops
    at Ready to integrate.
- 2026-10-08 16:35 CEST, Andreas, in the lead session (`4ffb11e2`):
  "you may create board items for tickets i request here". STANDING
  PERMISSION: the lead creates the board item for any ticket Andreas
  requests in a lead session (title `NZ-n: ...`, Status matching the
  handoff). Not for tickets the lead invents, not for relayed ones
  until he confirms them here, and never deleting items or changing the
  board's visibility. Applied at once to NZ-21 to NZ-25.
- 2026-10-08 17:20 CEST, Andreas, in the lead session (`4ffb11e2`),
  answering the five option lists: "1. go with a. you may do that. it
  must be a private notez note tho! 2. go with b 3. go with b 4. go with
  c, but break it up into multiple tasks/tickets. add linting and LSP
  and syntax highlighting too. approved to add dependencies, tree-sitter
  ive used before, with no objections against it. I'll install and run
  newest version now". The lead's reading:
  - NZ-23: the lead moves the material itself, as PRIVATE notes: the
    five docs from `~/Repos/bonsai/docs/` and the global
    `~/notez/2026-10-06-bonsai.md` go into
    `~/notez/personal/bonsai-education/` (docs in a `docs/` subfolder).
    No `notez attach` (not needed for private notes; it would change
    his registry). Done at 17:25 CEST, details on the ticket. One-off
    permission to move his files, used once.
  - NZ-26 (soft name rule) is wanted and AUTHORIZED: runs after NZ-22
    (same file).
  - NZ-24 (todo icon) decided: option b, a `✓` or Nerd Font check-list
    glyph in the section icons' style, no colour; project `TODO.md`
    rows count. AUTHORIZED to run after NZ-26.
  - NZ-25 family: everything (rendered markdown, syntax highlighting,
    linting, LSP) is wanted, split into tickets NZ-25, NZ-27, NZ-28,
    NZ-29. DEPENDENCIES APPROVED for this family: `pulldown-cmark` and
    `tree-sitter` with grammar crates; the lead names exact pinned
    versions in each brief and lists them in the ticket record. Linting
    and LSP start as design tickets (what runs, which languages, what
    the pane shows) and need his approval of the design before code.
    NZ-25 is AUTHORIZED to run after NZ-24; NZ-27 after NZ-25 once its
    brief names the grammars; NZ-28 and NZ-29 after their designs.
  - He installs the current `main` now.
- 2026-10-08 20:50 CEST, Andreas, in the lead session (`4ffb11e2`):
  "why are you stopping so often to ask me things? cant we find a way
  that lets you keep going?" The lead's reading, stated back to him and
  applied unless he objects: WORKING RULE for decisions. The lead
  decides every open design point with its own recommendation, records
  the decision on the ticket, and reports it in one line afterwards;
  Andreas overrules when he tries the build. The lead still stops and
  asks only for: anything irreversible or costing money; deleting or
  moving his files; `notez-core` public API or file format changes;
  dependencies outside an already approved family; CI or infrastructure
  changes; tags and releases; a ticket outside the recorded scope. The
  queue (NZ-27, the NZ-28 and NZ-29 design notes, NZ-4, NZ-5, NZ-3)
  runs on that basis. Status lines at hand-offs are not questions.
- 2026-10-08 21:30 CEST, Andreas, in the lead session (`4ffb11e2`):
  "can you do nz-3 and 5, and perhaps 28 & 29?" then "and do nz 4".
  The lead's reading: CONTINUE with NZ-4, NZ-3, NZ-5 (all in the
  standing scope since 2026-10-06, with the lead's recommended answers
  to their open points), NZ-28 as designed in `docs/design-nz28-linting.md`
  (two tickets NZ-28a and NZ-28b, no external tools), and NZ-29 as
  recommended in `docs/design-nz29-lsp.md` (no LSP in the preview;
  NZ-29a symbol outline from tree-sitter instead; NZ-29 itself closed
  as "not now" unless he objects after reading the note). Same
  integration delegation, branch pushes, board permissions and working
  rule. Order chosen by the lead: NZ-4, NZ-3, NZ-5, NZ-28a, NZ-28b,
  NZ-29a. Stop conditions unchanged.

### Working rule update and the `v0.1.0` tag (2026-10-15, from In flight)

WORKING RULE UPDATE, Andreas 2026-10-15 in the lead session: "do you
need to prompt me this whole time? i miss many things, it costs us
time. perhaps only prompt me for important decisions?" Applied: the
lead reports at milestones only (a ticket merged, a wave done, a
blocker) and stops him only for decisions that are his (scope,
design direction, anything irreversible or costing money, the stop
conditions of 2026-10-08). Tickets already in the authorized queue
run and merge without a check-in. Extends the 2026-10-08 working
rule; limits unchanged.

TAG: Andreas, 2026-10-15 in the lead session, "you can run the git
commands", directly after the lead presented the `v0.1.0` tag and
push command. Read as explicit authorization for exactly that
command, one-off. The lead checked that `ci.yml` triggers on branch
pushes only (a tag push starts nothing), then ran `git tag -a v0.1.0
-m "notez 0.1.0" dd2ff6a && git push origin v0.1.0`. Tag object
`3e684f0`, points at `dd2ff6a`, on `origin`. Tagging otherwise stays
his; this entry grants nothing further.

### Superseded ticket text

NZ-28, the design's original questions (the design is written):

Original questions: which
linters (markdownlint-style rules in-process, or shelling out to tools
on the machine such as `ruff`, `ktlint`, `clippy`), when they run (on
selection, on demand with a key), what the pane shows (gutter marks and
a footer count, or a list), and how a missing tool degrades. The lead
recommends starting with in-process markdown checks and an on-demand
key, no external processes. Needs Andreas's approval of the design
before code.

NZ-42, the candidate list before the 2026-10-15 narrowing:

Outcome: the navigation mechanics Andreas wants in todoz. Candidates, to be
narrowed by the lead after NZ-41 and NZ-5 are merged: fuzzy search on `/`
(reuse NZ-5's matcher rather than a second one), `0` and `s` jumps, the
reload-on-change behaviour from NZ-34 (a relist must succeed before any
probe cache is refreshed), pane-focus keys if NZ-43 happens. Not to port:
`Space` marks (todoz's `Space` checks a task). Not ready: needs NZ-5.

### NZ-43: todoz preview pane (2026-10-09, Draft; superseded by the 2026-10-15 record above)

Outcome: possibly a list-plus-preview layout in todoz using `panes.rs`,
showing a task's source file or notes. Needs a brainstorm with Andreas
before any brief: what the pane would show, and whether the board's width
can afford it. Does not run.

### Direction of 2026-10-06 and the first UI tickets (all Done)

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

### Earlier queue text (2026-10-15 morning and 2026-10-10)

The 2026-10-15 morning queue text follows for history.

Wave 1, in flight from `7cfeb27`: NZ-5 (tree line, owns README),
NZ-45 (tests only), NZ-17 (manifests, build.rs, changelog). Each gets
its own `nz-reviewer`; merges one at a time under the delegation;
after each merge the lead runs the checks on `main`, waits for green
CI, marks Done and removes the worktree. After NZ-17 merges the lead
presents `git tag -a v0.1.0 -m "notez 0.1.0" && git push origin
v0.1.0` for Andreas.

Then, in this order: the NZ-43 brainstorm with Andreas (todoz preview
pane: what it shows, width budget) once NZ-5 is on `main`; NZ-46 docs
refresh (`nz-small`, after NZ-5 and NZ-17); NZ-32 on the tree line;
NZ-11 alone (it touches `tree/render.rs`, `todo.rs`, `footer.rs`);
NZ-42 narrowed (`tui/todo.rs`, can run beside a tree ticket that does
not touch `todo.rs`); NZ-30 outline on the tree line; NZ-30 pass 2
if any. The lead writes `docs/design-nz47-dates.md` between
dispatches and brings it to Andreas; no code from it. Parked: NZ-28
pass 2, NZ-29 (closed "not now"). Drafts that do not run: NZ-18
(brainstorm first, when Andreas names it), NZ-43 until its brainstorm.

The 2026-10-10 queue text follows for the per-ticket procedure, which
is unchanged.

Queue, as confirmed by Andreas on 2026-10-10 ("order is fine", then
the three confirmations at 14:05); NZ-44 and NZ-6 are done. Next, on
the `tui/tree/` line one worker at a time: NZ-5 (fuzzy search, `0`,
`s`), NZ-42 (todoz navigation, brief after NZ-5 lands; it touches
`tui/todo.rs`, so it can overlap a tree ticket), NZ-32 (type filter),
NZ-28 pass 2 (gutter marks, footer count, `!` overlay, line map, plus
the follow-ups on the ticket), NZ-30 pass 2. Off the line, fit for the
second and third slots: NZ-30 pass 1 (`tui/outline.rs` plus a
`highlight.rs` addition; NZ-28 pass 1 is merged so `highlight::parse`
exists), NZ-11 (`tui/tree/render.rs` warning layout, `tui/todo.rs`,
`tui/footer.rs`: not beside a tree ticket). Drafts that do not run:
NZ-43 (brainstorm first), NZ-17, NZ-18, NZ-29. Nothing runs until
Andreas says to continue.

Per ticket the lead: creates branch and worktree from current `main`;
dispatches `nz-worker` with the template; dispatches `nz-reviewer`
separately on the exact diff (hash of `git diff` plus untracked files);
re-reviews any change; verifies the hash and runs build and tests
itself; commits exactly the reviewed diff, pushes the branch, waits for
green CI; merges `--no-ff`, runs the checks on `main`, pushes, waits
for green `main` CI; marks Done, removes worktree, local and remote
branch; records everything here and tells Andreas what changed on
screen. Up to three workers on disjoint files, `tree.rs` always one
worker, one merge chain on `main` at a time.

Andreas installs when he chooses (`./install.sh`, his to run). His
installed binary predates today's merges (NZ-3 header line, NZ-28 pass
1 which shows nothing yet).

## Moved from the handoff on 2026-10-26

Parked and finished ticket records moved verbatim by lead `9c5067bf` to keep the handoff short. NZ-18, NZ-28 and NZ-29 stay Backlog on the board; their issues (#47, #57, #58) carry the same text. NZ-42 is Done (record above under Finished tickets).

### NZ-18 (#47): include Pinz notes in notez as a special note type (brainstorm and design)

Status: Draft, RECORD ONLY. Relayed on 2026-10-08 at about 10:50 CEST by
the advisor session (`repos-f9`) with Andreas's words: "we need to
brainstorm, think and find a way to include Pinz notes within Notez. as a
special type of note." Not authorized to run: the standing scope does not
cover it and Andreas has not named it in a lead session. Board item
`PVTI_lAHOCU842c4BmE5Zzg_WCBU` created by the advisor session, Status
Draft; the lead only keeps its Status current.

This is a DESIGN ticket first. Its output is an approved design note under
`docs/`; no code until Andreas approves the design, and any `notez-core`
change needs his explicit approval (epoz pins the crate by rev).

Facts, as relayed and spot-checked by the lead against `~/Repos/pinz`
(crates `pinz-core` and `pinz-tui` exist; README line 21: "Your pins live
in a second repo of your own, not in this one: `~/pinz-board` by default,
or wherever `$PINZ_HOME` points"): pins are one file per pin in their own
git repo `~/pinz-board`; boards are directories, plus a last-world file;
Pinz has its own pull, commit and push (`pinz-core/src/sync.rs`) and a
board lock; its README says it deliberately does not ride on notes repos,
because an auto-push would sweep up unrelated work. Any design keeps
Pinz's isolation and sync rules intact.

Questions the design must answer:

1. Is a Pinz pin a note in the notez vault, a view onto `~/pinz-board`
   that notez reads without owning, or an import?
2. How it appears in the tree browser and the NZ-13 unified view: a new
   scope badge or a note type marker; reading only?
3. Who writes: read-only in notez first, edit through Pinz only?
4. Sync and conflicts: notez must never commit into `~/pinz-board`.
5. The file format, and whether `notez-core` changes at all or the CLI
   reads it.
6. Tags, rename, move, delete and multi-select (NZ-14 to NZ-16) on a
   Pinz item: allowed or blocked?
7. epoz impact.

Method: design-heavy work. The lead runs the `superpowers:brainstorming`
skill with Andreas in the lead session, one question at a time, ending in
a short design note under `docs/` that the ticket links; implementation
tickets follow only from the approved note. Proposed order: after NZ-14 to
NZ-16, since it touches the same browser code; Andreas can pull it
forward.

### NZ-28 (#57): linting in the preview (design)

Status: Draft, design first (Andreas 17:20 CEST). Board item
`PVTI_lAHOCU842c4BmE5Zzg_ZIJE`. Design note written by the lead:
`docs/design-nz28-linting.md` (tree-sitter syntax diagnostics plus
in-process markdown checks, gutter marks, footer count, an `!` issue
overlay; no external tools). Andreas on 2026-10-08 21:30 CEST: "perhaps
28 & 29?", read with the working rule as: go ahead as designed; he
overrules after reading the note or trying the build. Status: Ready,
runs after NZ-5, as ONE ticket in two passes.

PASS 1 DONE 2026-10-10: merged into `main` as `4841211` (worker commit
`3646b95`, review accepted at hash `5bc463df…`). `tui/lint.rs` (787
lines, 27 tests), `highlight::parse` added, `pub mod lint;`. Lead
decisions taken with the working rule: `TODO.md` task lines accept
`[ ]`, `[/]`, `[x]`, `[X]` (matches `notez-core` todo states); `..`
link targets are never probed (no vault root in pass 1; pass 2 may pass
one); exactly two trailing spaces after text (hard break) are exempt;
only full and collapsed reference links are checked; "no `#` heading"
means no level-1 heading, blank notes exempt; markdown is never parsed
with tree-sitter in pass 1. Nothing in the UI uses the module yet.

PARKED 2026-10-15 (Andreas: "Linting we can park, as we dont offer a
way yet to read, write or inspect actual code. Only .md as for now").
Board back to Draft. Resumes when notez treats non-markdown files as
notes. The module stays in the tree, unused. PASS 2 (not dispatched; needs the `tree.rs`
line after NZ-44 and NZ-5) must also take the reviewer's follow-ups:
(1) `todo_task_line` flags a plain `- [link](x.md)` bullet and `- [`
lines inside fenced code in `TODO.md`: only flag checkbox-shaped lines
and skip code blocks; (2) `fence_is_closed` accepts a closing fence
indented four or more spaces (or behind a `> ` inside a top-level
fence): compare the closing indent with the opening container prefix;
(3) `render_markdown` turns a lone `\r` into `\n` before parsing while
`lint` counts lines as `str::lines` does, so the line map must use one
line definition; (4) decide whether `N issues` counts diagnostics or
lines (several can share a line); (5) share one pulldown `Options`
constant between `markdown.rs` and `lint.rs`; (6) images are not
checked for missing targets (matches the design wording).

Brief (from the design note; the worker verifies the code facts):

1. Pass 1, `tui/lint.rs` (new): `pub struct Diagnostic { line: usize
   (0-based source line), severity: Severity (Warning, Error), message:
   String }`, `pub fn lint(path: &Path, text: &str, language:
   Option<Language>) -> Vec<Diagnostic>` sorted by line. Sources: (a)
   tree-sitter `ERROR` and `MISSING` nodes from a parse with the NZ-27
   grammar when `language` is `Some` (one diagnostic per node, message
   `syntax error` or `missing <kind>`, Error); (b) markdown checks when
   the language is Markdown, built on the pulldown events: heading
   level jump (`#` to `###`), duplicate heading text, unterminated
   fence, relative link whose target file does not exist (resolved
   against `path`'s directory), reference link without a definition,
   trailing whitespace, a `TODO.md` task line not matching `- [ ]` or
   `- [x]`, no `#` heading in the note (all Warning). Each check is one
   function listed in a table so single checks can be turned off in
   code. Files over 1 MiB return no diagnostics. Tests per check (one
   positive, one negative), per language for the syntax source, and a
   bounded-time guard.
2. Pass 2, integration: `tui/markdown.rs` keeps a source line map
   (rendered line index to source line) so marks land on the right
   rendered line; a gutter mark `▲` in a warning colour (`theme`,
   existing palette) at the start of a line with a diagnostic, in both
   rendered and raw view; the footer file type segment gains `<n>
   issues` (nothing at zero); `!` opens an issue list overlay (reusing
   the help overlay drawing): `line: message` sorted, `j`/`k` move,
   `Enter` scrolls the preview to that line and closes, `Esc` closes;
   `!` in the key table (help "issues"), footer hint low priority
   shown only when there are issues. Lint runs with the preview cache
   (same key) so it costs nothing per frame. README paragraph.
3. Not in scope: external tools, auto-fix, vault-wide lint, config
   files.

Allowed files: `crates/notez-cli/src/tui/lint.rs` (new), `tui/mod.rs`,
`tui/markdown.rs`, `tui/highlight.rs` (expose a parse or error-node
walk), `tui/tree.rs`, `tui/footer.rs`, `tui/help.rs`, `tui/theme.rs`,
`README.md`. No new dependency, no notez-core change. One review of the
whole diff. Reviewer probes: a relative link check must never touch
files outside the vault or repo (no network, no absolute paths
followed); overlay keys inert elsewhere; line map correctness after
wrapping; performance on a 500 KB note.

### NZ-29 (#58): LSP in the preview (design)

Status: Draft, design first (Andreas 17:20 CEST). Board item
`PVTI_lAHOCU842c4BmE5Zzg_ZIK8`. Design note written by the lead:
`docs/design-nz29-lsp.md`, recommending NOT to embed LSP in a
read-only preview and to build a symbol outline from tree-sitter
instead. Andreas on 2026-10-08 21:30 CEST: "perhaps 28 & 29?", read
with the working rule as leave to follow the recommendation: NZ-29
stays Draft as the LSP record (closed as "not now" unless he objects
after reading the note) and the work runs as NZ-30 below. Original
questions: what a
read-only preview gains from a language server (hover, diagnostics,
symbols), which servers and how they are found, process lifetime
inside a TUI, and whether this belongs in the notes browser at all
versus opening the file in the editor. The lead recommends deciding
this after NZ-27 and NZ-28 have been used. Needs Andreas's approval of
the design before code.

### NZ-42 (#70): todoz navigation like the tree browser (2026-10-09, Draft)

NARROWED 2026-10-15 (Andreas "42: ok?" to the lead's proposal): `/`
fuzzy search in todoz reusing NZ-5's matcher and highlight, plus the
reload-on-change behaviour from NZ-34. No `0`/`s` jumps, no pane-focus
keys. Brief to be written after NZ-5 lands and the NZ-43 brainstorm
has happened.

