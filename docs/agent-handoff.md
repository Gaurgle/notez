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

## Authorized by the owner

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

Nothing is in flight. The In flight entries of finished tickets moved to
`docs/agent-log.md` on 2026-10-16; the two owner rulings recorded here
on 2026-10-15 stay.

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

## Tickets

Ticket briefs and records live here; the board at `projects/2` carries
the same Status per ticket. Status values: Draft (brief written, not yet
approved by Andreas), Ready, In flight, Ready to integrate, Done.

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

### NZ-47: dates as a first-class part of a note (2026-10-15, Draft, design only)

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

### NZ-51: TODO.md in the inspector renders like standalone todoz (2026-10-15, Draft)

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

### NZ-52: todoz discovers TODO.md stores the way notez discovers repos (2026-10-15, Draft)

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

### NZ-43: todoz preview pane (2026-10-15, Ready, design approved)

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

### NZ-42: todoz navigation like the tree browser (2026-10-09, Draft)

NARROWED 2026-10-15 (Andreas "42: ok?" to the lead's proposal): `/`
fuzzy search in todoz reusing NZ-5's matcher and highlight, plus the
reload-on-change behaviour from NZ-34. No `0`/`s` jumps, no pane-focus
keys. Brief to be written after NZ-5 lands and the NZ-43 brainstorm
has happened.

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

Updated 2026-10-15 at the stop by lead `8a79e020`. Nothing runs until
Andreas says to continue; when he does, the queue below is already
authorized (standing scope plus the 2026-10-15 entries) and needs no
new confirmation.

1. NZ-42, todoz `/` fuzzy search reusing NZ-5's `text::filter_matches`
   and highlight, plus reload-on-change (NZ-34 behaviour): files
   `tui/todo.rs`, `tui/help.rs`, `README.md`. Brief to write from the
   narrowed record under Tickets. One worker, one review.
2. NZ-49 (create the board item under the standing permission):
   `notez agenda [--days N] [--json]` listing due todos and dated
   notes across scopes via `NoteDates::read` and `Task.due`, overdue
   first, grouped by day, plus the inspector footer showing the
   selected note's dates next to the language badge. Files:
   `commands/agenda.rs` (new), `cli/mod.rs`, `main.rs`,
   `tui/tree/preview.rs` or wherever the preview footer is drawn,
   `README.md`. Can run beside NZ-42 (disjoint files). Design:
   `docs/design-nz47-dates.md`, consumers 1 and 3.
3. NZ-43, todoz preview pane (approved design under Tickets), after
   NZ-42 and NZ-49.
4. NZ-30, symbol outline, on the tree line after NZ-49's footer
   change.
5. NZ-50 (create when reached): the write path, todoz due column
   with the `D` prompt and `notez add --due/--on`, under the
   "Requirements for the write path" in the design note.
6. NZ-47d / NZ-18: Pinz as a note type, design brief first (pins
   editable, no git from notez in `~/pinz-board`, length warning
   with a proposed threshold); needs Andreas's approval of the brief
   and the Pinz-side "keep unknown keys" change before code.
7. Added at the stop, Drafts, position in the queue is Andreas's
   call (the lead suggests NZ-52 early, since it is about seeing
   your files, and NZ-51 beside NZ-43): NZ-51 `TODO.md` in the
   inspector renders like standalone todoz; NZ-52 todoz discovers
   `TODO.md` stores the way notez discovers repos. Both start with a
   short investigation, not code.

Per ticket the procedure is unchanged (see the 2026-10-10 paragraph
below): branch and worktree from current `main`, `nz-worker`, a
separate `nz-reviewer` on the exact diff, re-review after changes,
lead verification (touch a source file first), commit exactly the
reviewed diff, push, green branch CI, `--no-ff` merge, checks on
`main`, push, green `main` CI, Done, cleanup. Report at milestones
only; stop Andreas only for his decisions.

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

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
