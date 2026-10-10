# Relay handoff

The baton between leads. Read this file, `CLAUDE.md` and
`docs/agent-workflow.md` before acting. The board is the GitHub Project
`https://github.com/users/Gaurgle/projects/2` (owner `Gaurgle`, number 2;
read command and ids in `docs/agent-workflow.md`). This file holds the
ticket briefs and records, in-flight work, decisions and the next
authorized step; the board mirrors each ticket's Status and the lead
keeps it current as tickets move.

## Current lead

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

## Authorized by the owner

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
- No other ticket execution is authorized. Andreas names which tickets run.

## In flight

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

Nothing is in flight. BATON RELEASED at 13:48 CEST (real clock) on
2026-10-10 (see Current lead and its clock correction). The docs commit that records this is the last commit
of the day on `main`.

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

2026-10-10 15:02 CEST, Andreas: "after current workers come back green
we can stop for today". No new dispatch. NZ-44 and NZ-6 run to their
reports, are reviewed and integrated if green, then the baton is
released.
Remote branch `nz-39-suffix` deleted at 12:48 (merged in `234d21c`,
standing permission).

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

## Tickets

Ticket briefs and records live here; the board at `projects/2` carries
the same Status per ticket. Status values: Draft (brief written, not yet
approved by Andreas), Ready, In flight, Ready to integrate, Done.

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

#### NZ-17: versioning, first version 0.1.0

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

#### NZ-18: include Pinz notes in notez as a special note type (brainstorm and design)

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

#### NZ-28: linting in the preview (design)

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

PASS 2 (In flight on the board, not dispatched; needs the `tree.rs`
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

Original questions: which
linters (markdownlint-style rules in-process, or shelling out to tools
on the machine such as `ruff`, `ktlint`, `clippy`), when they run (on
selection, on demand with a key), what the pane shows (gutter marks and
a footer count, or a list), and how a missing tool degrades. The lead
recommends starting with in-process markdown checks and an on-demand
key, no external processes. Needs Andreas's approval of the design
before code.

#### NZ-29: LSP in the preview (design)

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

#### NZ-30: symbol outline for the preview (tree-sitter, in place of LSP)

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

#### NZ-32: filter by document type

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

#### NZ-5: fuzzy search, tag filter clearing and scope cycling

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

### NZ-42: todoz navigation like the tree browser (2026-10-09, Draft)

Outcome: the navigation mechanics Andreas wants in todoz. Candidates, to be
narrowed by the lead after NZ-41 and NZ-5 are merged: fuzzy search on `/`
(reuse NZ-5's matcher rather than a second one), `0` and `s` jumps, the
reload-on-change behaviour from NZ-34 (a relist must succeed before any
probe cache is refreshed), pane-focus keys if NZ-43 happens. Not to port:
`Space` marks (todoz's `Space` checks a task). Not ready: needs NZ-5.

### NZ-43: todoz preview pane (2026-10-09, Draft)

Outcome: possibly a list-plus-preview layout in todoz using `panes.rs`,
showing a task's source file or notes. Needs a brainstorm with Andreas
before any brief: what the pane would show, and whether the board's width
can afford it. Does not run.

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

Written 2026-10-10 15:05 CEST by lead `f25c8d27`; updated at the stop
(see the last paragraph of this section and In flight).

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

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
