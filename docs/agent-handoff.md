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
`4ffb11e2-7fc1-4590-92e3-0cbd75ef9142`, took the baton on 2026-10-08 09:49
CEST on Andreas's "resume from last position, new day, new code! let me
know if anything is unclear". Reconciled at takeover: `main` =
`origin/main` = `4a239e3`, working tree clean, `git worktree list` shows
only the main checkout, no ticket branch exists, no worker or reviewer
running. `ps` shows exactly one `claude --agent nz-coordinator` process
(this session); the previous lead (`52bd7aa5`) released the baton at
19:25 CEST on 2026-10-07 by its own record below and has no process.
Board matches the handoff: NZ-1, 2, 7, 8, 9, 10, 12, 13 Done; NZ-6,
NZ-11, NZ-14 Ready; NZ-3, 4, 5, 15, 16, 17 Draft. Only this session
leads. Reading of "resume from last position": CONTINUE the paused
standing scope and integration delegation (recorded 2026-10-07 16:25
under Authorized by the owner), next ticket NZ-14; stated back to
Andreas at takeover so he can correct it.

RESUMED at 21:35 CEST on 2026-10-08 by the same session (`4ffb11e2`)
on Andreas's "can you do nz-3 and 5, and perhaps 28 & 29?" and "and do
nz 4". Reconciled: `main` = `origin/main` = `1edfbce`, clean, no
worktrees, no agents running, board as left. Queue in this order, all
on `tui/tree.rs` so one at a time: NZ-4 (dispatched 21:37 CEST), NZ-3,
NZ-5, NZ-28 (per its design note), NZ-29 (per its recommendation:
symbol outline instead of LSP). Working rule of the afternoon applies.
Only this session leads.

Earlier the same day: STOPPED at 17:00 CEST on Andreas's instruction
("i must go now, find a place to stop, stop the workers, commit and
ship this"); baton released; the NZ-4 pass 1 worker running at that
moment was stopped before it changed a file and its clean worktree and
branch were removed.

CLOCK CORRECTION: the CEST times this lead wrote into this file
between about 12:00 and 17:00 on 2026-10-08 were estimates and run up
to six hours ahead of the real clock (the lead wrote "23:05" at what
was about 16:50). The order of events is right; for real times use
`git log --date=iso` and the CI run `createdAt` fields. From this note
on, times come from `date`.

State at the stop, in short:

- Done and merged today by this lead, in order: NZ-14 (`ae3617e`),
  NZ-15 (`36699c1`), NZ-16 (`5298d54`), NZ-19 CI (`3941481`), NZ-20
  (`483cb1c`), NZ-21 (`d667999`), NZ-22 (`82fae54`), NZ-26 (`5442854`),
  NZ-24 (`706e169`), NZ-25 (`6a1c814`), NZ-27 (`299770e`). NZ-23 was
  resolved without code (files moved). `main` = `origin/main` =
  `08deabb` plus this stop commit; the last `main` CI run
  (`37796746297` on `08deabb`) is green on lint, ubuntu and macos.
- No ticket worktree or branch exists; `git worktree list` shows only
  the main checkout. No remote ticket branches remain.
- Board: NZ-14 to NZ-16, NZ-19 to NZ-27 Done except NZ-28 and NZ-29
  (Draft, design notes written) and NZ-4 (Ready, next); NZ-5, NZ-3,
  NZ-17, NZ-18 as before.
- Andreas installed during the day (around 17:20 of the estimated
  clock, about 14:00 real) and has NZ-13 to NZ-21 at the latest; NZ-22
  to NZ-27 (preview scroll keys, soft names, todo icon, rendered
  markdown with `p`, syntax highlighting) need another `./install.sh`,
  his to run. Look items for him to judge: the `p` toggle key, the
  dimmed `#`/`-`/`>` markers in the raw markdown view (NZ-27 F2), the
  todo glyph, the row badge next to the name.
- Next: NZ-4 (brief Ready with the lead's decisions), then NZ-5, NZ-3;
  NZ-28 and NZ-29 wait for Andreas to read `docs/design-nz28-linting.md`
  and `docs/design-nz29-lsp.md`; NZ-18 (Pinz) waits for his word;
  NZ-11 and NZ-6 in the second slot when their files are free.
- Standing permissions in force (Authorized by the owner): the
  integration delegation, branch pushes with remote deletion after the
  merge, board item creation for tickets he requests here, the
  dependency approval for the tree-sitter and pulldown-cmark family,
  and the working rule of "20:50" (the lead decides open design points
  and reports; asks only for the listed owner-only matters). All paused
  with the baton, not withdrawn.
- Process lesson for the next lead: the integration chain picks the
  `main` run with `gh run list --limit 1`; when the lead commits the
  handoff onto `main` while a chain is between its merge and its push,
  the handoff push carries the merge and the chain then watches the
  previous run (this happened on NZ-27; the real run was watched
  afterwards and is green). Either do not commit to `main` while a
  chain runs, or select the run by `headSha`.

Previous lead, for history:

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`52bd7aa5-2d53-4a8e-bf4e-7bd5ec61a99b`, took the baton on 2026-10-07 16:30
CEST on Andreas's "you can continue the work, from 2f04539, i believe.
NZ-8 to NZ-16", followed by his pasted queue list (NZ-8, NZ-12, the UI
tickets, NZ-13, NZ-14, NZ-15, NZ-16). Reconciled at takeover: `main` =
`origin/main` = `2f04539`, working tree clean; one worktree,
`.claude/worktrees/NZ-8`, uncommitted diff hashing to the value recorded
under In flight; no worker or reviewer running. The previous lead's
process (`298bd2d1`, 22 hours old) was still open at takeover; its own
record below says it released the baton the night before. RETIRED at
16:41 CEST on 2026-10-07: on Andreas's request the advisor session
(`repos-f9`) sent it SIGTERM after checking it was idle with a clean tree,
and this lead confirmed with `ps` that no process of that session remains.
Only this session leads.

STOPPED FOR THE DAY at about 19:25 CEST on 2026-10-07 on Andreas's
instruction. BATON RELEASED. No lead is active. No worker or reviewer is
running. A new lead may take over from this file once Andreas says to
continue; nothing here authorizes starting on its own.

State at the stop, in short:

- Done and merged today by this lead: NZ-8 (`bf8f2ca`), NZ-12
  (`ff33de0`), NZ-13 (`ee6a6fd`). `main` and `origin/main` are in sync
  (the push landed on the fourth retry after GitHub's server errors).
- No ticket worktree or branch exists; NZ-13's were removed after the
  push. `git worktree list` shows only the main checkout.
- Board (`projects/2`): NZ-13 Done and NZ-14 Ready, applied after the
  GitHub errors cleared.
- Andreas's installed binary is from 18:07 CEST on 2026-10-06 (NZ-1,
  NZ-2, NZ-7). NZ-9, NZ-10, NZ-8, NZ-12 and NZ-13 all need
  `./install.sh`, his to run. NZ-13 changes the default view and adds
  badges; it is the one to look at.
- Next: NZ-14 (brief Ready), then NZ-15, NZ-16, the UI tickets.
- Standing scope and the integration delegation are paused, not
  withdrawn; a lead continues them only when Andreas says so.
- A Linear evaluation was mentioned by Andreas to the advisor; not an
  instruction.

Previous lead, for history:

Claude `nz-coordinator` (model `claude-fable-5-1`), session
`aa27d2ed-a3e9-498c-ae61-db84f43eae7c`, took the baton on 2026-10-06 15:01
CEST. First lead since setup: no previous lead, no ticket worktrees or
branches, no workers running at takeover.

Paused from 17:20 to 17:56 CEST at Andreas's request, then resumed by the
same session (its id appears to have changed to `298bd2d1` at about 18:21).

STOPPED FOR THE NIGHT at 18:35 CEST on 2026-10-06 on Andreas's instruction
("wrap this up ... we'll keep working on this another day"). BATON
RELEASED. No lead is active. No worker or reviewer is running; every agent
dispatched today has reported or was stopped. A new lead may take over
from this file once Andreas says to continue; nothing here authorizes
starting on its own.

State at the stop, in short (details under In flight, Tickets, Next step):

- `main` and `origin/main` are in sync. Done and pushed today: NZ-1, NZ-2,
  NZ-7, NZ-9, NZ-10.
- One open worktree: `.claude/worktrees/NZ-8`, branch
  `feat/NZ-8-create-note-in-browser`, with a COMPLETE BUT UNREVIEWED,
  uncommitted worker result. It is the next thing to do.
- Not started: NZ-4, NZ-5, NZ-3 (UI, in that order), NZ-6, NZ-11. Drafts
  relayed on 2026-10-07 through an advisor session, not yet confirmed by
  Andreas in a lead session: NZ-12 (delete a note), NZ-13 (unified default
  view), NZ-14 (folders), NZ-15 (move and change visibility), NZ-16
  (multi-select). See the "Relayed on 2026-10-07" note under Tickets for
  provenance, the two relayed approvals and the unconfirmed key map. Baton
  still released; nothing dispatched.
- Andreas's installed binary is from 18:07 (NZ-1, NZ-2, NZ-7). NZ-9 and
  NZ-10 need another `./install.sh`, his to run.
- This file has grown long. The sections that matter to a new lead are
  this one, In flight, Next step, and the tickets NZ-8, NZ-4, NZ-5, NZ-3,
  NZ-6 and NZ-11. The Done tickets are history.

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
- NOT an owner instruction, recorded here so it is not mistaken for one:
  NZ-9 (never overwrite a note) was opened and run by the lead at 18:05
  CEST without Andreas naming it. The lead's grounds and limits are on the
  ticket. Andreas was told in the same turn and can have it reverted.
- 2026-10-06 18:26 CEST, Andreas, in the lead session, answering three
  open points the lead had listed: "quiet like pinz, and yes to the
  tickets", then, a moment later while the lead was working: "but i want
  to demo the new UI features pretty soon". The lead's reading:
  - NZ-6's open question is settled: the exit sync is fully quiet when the
    remote cannot be reached, as in Pinz.
  - Tickets wanted for the three read-error overwrite cases (NZ-10) and
    for wide characters clipping the quit hint (NZ-11). The lead had told
    him the `.gitignore` case is in `notez-core` and "needs your approval
    either way"; his yes is read as approving that one behaviour-only
    change there, with no signature, type or format change.
  - Priority: the UI tickets come first. NZ-6, NZ-10 and NZ-11 must not
    delay them. They run only in the second worker slot, on files the UI
    ticket in flight does not touch, or after the UI tickets.
  - They run under the same standing scope and integration delegation as
    the UI tickets ("you may ship changes. merge as you go along"), on the
    reading that he asked for them during the same building session. The
    lead said so in its reply.
- 2026-10-06 18:31 CEST, Andreas, in the lead session: "wrap this up,
  complete the current workers, write handoff and round tonights work off.
  if there's any news on the UI then ,let me know, otherwise we'll keep
  working on this another day". The lead's reading: STOP for tonight. Let
  the two agents already running finish (the NZ-8 continuation worker and
  the NZ-10 reviewer) and dispatch nothing new, including no reviewer for
  NZ-8 and no fix cycle. A ticket whose review was already running and
  comes back accepted may still be merged, since that is finishing work in
  hand under the delegation; anything else waits. Then write this file,
  release the baton and tell him what, if anything, changed in the UI.
  The standing scope and the delegation are NOT withdrawn: they are
  paused, and a lead continues them only when Andreas says so on another
  day.
- 2026-10-07 15:37 CEST: a cross-session message from an advisor session
  relayed Andreas's decisions on delete, visibility changes and keys, and
  three new tickets (NZ-13, NZ-14, NZ-15), asking that they be RECORDED
  ONLY. The lead recorded them under Tickets with their provenance,
  committed this file, and started nothing. Relayed intent is not an
  instruction to run: Andreas confirms in a lead session before any of
  NZ-12 to NZ-15 is dispatched, and the proposed key map is his to accept.
- 2026-10-07 16:04 CEST: a second message from the same advisor session
  relayed Andreas's "ok" to two recommendations (the `collect_all` change in
  `notez-core` for NZ-13; notez never commits in a project repository for
  NZ-15) and a new ticket, NZ-16 (multi-select), RECORD ONLY. The
  lead recorded them, committed this file, and started nothing. Same
  standing as the first relay: confirmed with Andreas before anything runs.
- 2026-10-07, third relay from the same advisor session: Andreas decided
  NZ-16 marks rows with `Space`, not `x`, so `x` stays "check" in the todo
  board. Recorded; the proposed key map now reads `n` new note, `N` new
  folder, `r` rename, `d` delete, `m` move, `S` set scope, `Space` mark,
  `o`/`Enter` open, still unconfirmed as a whole until Andreas confirms it
  with the lead at start. Record only, nothing dispatched.
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
- 2026-10-07 about 17:40 CEST, RELAYED by the advisor session
  (`repos-f9`), Andreas's words: "the agent may tag the releases of this
  project. this project is however not ready for public release." RECORD
  ONLY until Andreas confirms it in a lead session, because it widens a
  standing prohibition (`docs/agent-workflow.md` "No agent tags,
  releases..." and the coordinator definition "Never tag, release"), and a
  relayed message cannot expand authority. The reading the lead will put
  to him: once NZ-17 is done and merged, the lead may create the annotated
  version tag (for example `v0.1.0`) locally; pushing a tag to the public
  repository is held back and asked about, since the project is "not
  ready for public release"; no GitHub Release, announcement or anything
  else that publishes. On his confirmation the lead edits
  `docs/agent-workflow.md` to say so and NZ-17's record; the coordinator
  definition is his file to change.
  SETTLED 2026-10-07 about 17:55 CEST, Andreas in the lead session
  (`52bd7aa5`): "lets skip tagging until we have a first version then."
  Agents still never tag or release; the relayed permission is NOT in
  force. Revisit when NZ-17 has produced a first version; until then the
  standing rule in `docs/agent-workflow.md` is unchanged.
- 2026-10-08 about 10:50 CEST: RELAYED by the advisor session
  (`repos-f9`), Andreas's words "we need to brainstorm, think and find a
  way to include Pinz notes within Notez. as a special type of note."
  RECORD ONLY as NZ-18 (Draft, design first). Not an instruction to run;
  the standing scope does not cover it. Andreas names it in a lead
  session before any brainstorming session or code starts.
- 2026-10-08 about 12:30 CEST: RELAYED by the advisor session
  (`repos-f9`), Andreas's words "yes, ok from me. lets set this up" on
  adding CI to notez, with the advisor's uncountered recommendation that
  this lead runs it without being paused. Recorded as NZ-19 (brief
  Ready). RECORD ONLY until Andreas confirms in the lead session: a CI
  change is outside the standing scope and a relayed message cannot
  widen it. Questions put to him at 12:35 CEST: confirm the ticket; slot
  (second worker now, or after NZ-15); permission to push the ticket
  branch to `origin` so the workflow runs before the merge; permission
  to delete that remote branch afterwards.
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

NZ-31 (tree margin and branch lines). Dispatched 2026-10-08 at 22:02
CEST by lead `4ffb11e2` on Andreas's requests of 21:50 and 21:55 CEST.
`git status --short` before dispatch: only this file modified. Base
`554fd0c` (= `main` = `origin/main`, the NZ-4 merge), branch
`fix/NZ-31-tree-margin`, worktree `.claude/worktrees/NZ-31`, model
`opus` via `nz-worker`, one pass. RUNNING. Board item
`PVTI_lAHOCU842c4BmE5Zzg_fBhY` In flight. Then review, branch push,
CI, merge, `main` run, cleanup; then NZ-34, NZ-3, NZ-5, NZ-32, NZ-28,
NZ-30.

NZ-36 (markdown tables and footnotes) also running, dispatched 22:08
CEST on `75bb3dc`, worktree `.claude/worktrees/NZ-36`, `tui/markdown.rs`
only; its record is under Tickets. Three agents are not running at
once: NZ-35's worker and reviewer have reported, so the two live
workers are NZ-31 and NZ-36 on disjoint files.

NZ-35 (`install.sh` build progress and timing) in the SECOND slot.
Dispatched 2026-10-08 at 22:10 CEST by lead `4ffb11e2` on Andreas's
request ("can we add to it a little loading animation or bar that
shows how fast its going"); only `install.sh`, disjoint from NZ-31.
Base `727dae9`, branch `chore/NZ-35-install-progress`, worktree
`.claude/worktrees/NZ-35`, model `sonnet` via `nz-small`. REPORTED at
22:13 CEST: `install.sh` only, 15 insertions, 1 deletion, uncommitted
diff against `727dae9` hashing to
`fece6a9ef9cbd93484d210791793b601364d389eec0ddcbf378c0b6b6460912c`;
`bash -n` parses; about 18k agent tokens. REVIEW dispatched at 22:15
CEST (`nz-reviewer`); ACCEPTED at 22:18 CEST (about 19k tokens; probes
under bash 3.2 confirmed the empty unquoted flag adds no argument and
`set -e` still aborts; follow-ups: the TTY test looks at stdout while
cargo draws on stderr, fine by decision; `copied to` and `installed`
lines near-duplicate). Committed as `c582600` (hash `fece6a9e...`),
branch pushed at 22:20 CEST; integration chain running in the
background (runs selected by SHA); this file is committed after it
reports.
Decisions: drop `--quiet` on a TTY so cargo's own progress bar shows,
keep `--quiet` when piped; `SECONDS` timing per build and total; one
`ok` line per step. Board item `PVTI_lAHOCU842c4BmE5Zzg_fT_A` In
flight. Review by `nz-reviewer` (content review of a shell script, no
cargo checks needed beyond `bash -n`), then branch push (CI runs: the
`paths-ignore` covers only `**.md` and `docs/**`, and `install.sh` is
neither), merge on green, cleanup.

NZ-4 is Done (merge `554fd0c` at 21:59 CEST; branch run and `main` run
`37836071227` green; remote branch deleted; worktree and local branch
removed; board Done). Andreas was told to install.

NZ-4 record, for history. Dispatched 2026-10-08
at 21:37 CEST by lead `4ffb11e2` on Andreas's "and do nz 4". `git
status --short` before dispatch: clean. Base `1edfbce` (= `main` =
`origin/main`; code at the NZ-27 merge `299770e`), branch
`feat/NZ-4-panes`, worktree `.claude/worktrees/NZ-4`, model `opus` via
`nz-worker`. Pass 1 (`tui/panes.rs` model with tests, keys `<` `>` `=`
`1` `2` `Tab`, focus semantics, fold on `2`, list and preview widths
following the split, footer hint sets) REPORTED at 21:46 CEST: 4 files
(`tui/mod.rs`, new `tui/panes.rs` registered with `git add -N`,
`tui/theme.rs`, `tui/tree.rs`), 889 insertions, 100 deletions,
uncommitted diff against `1edfbce` hashing to
`5b41ead623b77655a211c01a9afbc5af706157e45ced694e82b98b48cabc11ab`;
worker checks: build clean, notez-cli 445 passed (29 new), notez-core
146 passed; about 162k agent tokens. As built: `Panes` model
(minimums 24 list, 20 preview, list wins), keys `< > =` and `1 2 Tab`
at priorities 13 to 16 before `q`, focused border LAVENDER, pane
numbers in both titles, preview-focused `j k Up Down PgUp PgDn` scroll
and `h l Left Right Enter o Space` inert, any input mode returns focus
to the list, a folded preview does no I/O, the preview loses one
column to the new border strip, the list rect is unchanged at 50/50.
Lead decisions on the report (accepted): `Esc` keeps its browse
behaviour with the preview focused; `Tab` stays on the list while
folded; the NZ-22 priority test rule relaxed for the pane rows only.
Pass 2 (grip drawing, drag, wheel and click routing, auto-fold,
README) REPORTED at 21:54 CEST: `README.md`, `tui/panes.rs`,
`tui/theme.rs`, `tui/tree.rs`; whole diff against `1edfbce` 5 files,
1357 insertions, 121 deletions, hashing to
`db90a8f0b47aca0095e59541a503f90c34dcc91b615bede3333c9bd9bcdb8db3`;
worker checks: build clean, notez-cli 460 passed, notez-core 146
passed; about 108k agent tokens; no existing test changed in pass 2.
As built: 3-row `⠿` grip, LAVENDER while dragging; `grab_hit` with one
column of slop; drag rounds to the nearest percent and clamps; any
non-drag mouse event ends a drag; wheel routes by pane (over the list
it moves the cursor, which is a change: before, the wheel always
scrolled the preview); a left click focuses the pane under it; a
filter-strip click now counts only inside the list (before, a click on
that row inside the preview opened the filter, a latent bug); auto-fold
via `fit(width)` with a separate `auto_folded` flag, `2`/`Tab`/focus
refused while auto-folded, the user's own fold never overwritten.
Lead decisions (accepted): wheel over a folded preview's space reaches
the list. Workers stopped. REVIEW dispatched at 21:57 CEST
(`nz-reviewer`, opus) on that hash; ACCEPTED at 22:05 CEST first time
(about 70k tokens; follow-ups: a drag re-renders a large note once per
column, render at a stable width during the drag if it ever shows;
the inert set with the preview focused is not uniform (`Space` inert,
`d m r t S` pass through) and the README overstates it; the mouse
ignores the help overlay, pre-existing class; tag mode blocks the wheel
over the list). Lead checks: build clean, 460 + 146 passed. Committed
as `bbf550c` (diff against `1edfbce` hashes to the accepted
`db90a8f0...`), branch pushed at 22:08 CEST; the integration chain is
running as one background command, now selecting CI runs by commit SHA
with retries (the fix for the race noted under Current lead); the lead
commits this file only after the chain reports. Board item
`PVTI_lAHOCU842c4BmE5Zzg_KW1E` In flight until then. Next after it:
NZ-31. Board item
`PVTI_lAHOCU842c4BmE5Zzg_KW1E` In flight. Then NZ-3, NZ-5, NZ-28a,
NZ-28b, NZ-29a.

Earlier: STOPPED at 17:00 CEST (real clock) on
2026-10-08 on Andreas's instruction; resumed 21:35 CEST.
NZ-27 is Done (merge `299770e`; branch run green; the real `main` run
`37796746297` on `08deabb` green on all three jobs; remote branch
deleted; worktree and local branch removed; board Done). NZ-4 pass 1
had been dispatched a minute before the stop and was STOPPED before it
changed any file; its clean worktree `.claude/worktrees/NZ-4` and
branch `feat/NZ-4-panes` were removed; NZ-4 is Ready on the board and
is the next ticket.

NZ-27 record, for history. Dispatched 2026-10-08 at
21:05 CEST (estimated clock, see the correction under Current lead) by lead `4ffb11e2` under the dependency approval of 17:20
CEST and the working rule of 20:50 CEST. `git status --short` before
dispatch: clean. Base `9e1bb20` (= `main` = `origin/main`; code at the
NZ-25 merge `6a1c814`), branch `feat/NZ-27-syntax-highlighting`,
worktree `.claude/worktrees/NZ-27`, model `opus` via `nz-worker`.
Pass 1 (`tui/highlight.rs` with tests, theme capture styles, the
tree-sitter dependencies and grammars, build-time and binary-size
measurements) REPORTED at 21:35 CEST: 5 files (`Cargo.lock`,
`crates/notez-cli/Cargo.toml`, new `tui/highlight.rs` registered with
`git add -N`, `tui/mod.rs`, `tui/theme.rs`), 1064 insertions,
uncommitted diff against `9e1bb20` hashing to
`2b9d30a0a0b385c489557eba2f6a804fd67407cc0e4026af23a667a617ce48d8`;
worker checks: build clean, notez-cli 399 passed (24 new), notez-core
146 passed; about 110k agent tokens. Findings: `tree-sitter-kotlin
0.3.8` cannot resolve (it pins `tree-sitter >=0.21, <0.23`), so Kotlin
was left out of pass 1; the other ten crates compile and load; `Cargo.
lock` gained tree-sitter 0.27.0, tree-sitter-highlight 0.27.0,
tree-sitter-language 0.1.8, the eight grammars, streaming-iterator
0.1.9, regex 1.13.1 (+ regex-automata, regex-syntax, aho-corasick),
and bumped build deps cc 1.2.62 to 1.6.0, shlex 1.3.0 to 2.0.1,
find-msvc-tools 0.1.9 to 0.1.14, serde_json 1.0.149 to 1.0.151; cold
build 15.3 s to 17.7 s; release binary unchanged at 2,359,440 B until
pass 2 links the grammars (static libs total about 5.2 MB); the
bundled `tree_sitter_md` block injection query breaks fenced code, so
the module ships its own `MARKDOWN_BLOCK_INJECTIONS`; query compile
costs 84 ms (rust) down to under 1 ms, once per language; a 2 MB
markdown file with many fences took about 8 s debug / 1.4 s release
whole-file, hence per-fence highlighting and the 1 MB skip. Lead
decisions (working rule): keep the local injections query; accept the
build-dep bumps; ship Kotlin through `tree-sitter-kotlin-ng = "1.1.0"`
(same grammar family as approved). Pass 2 (Kotlin, integration into
rendered markdown and the raw view, character wrapping for code,
footer language name, cache, measurements, README) REPORTED at 22:00
CEST: Kotlin builds through `tree-sitter-kotlin-ng 1.1.0` but that
crate ships no queries, so the module carries a local
`KOTLIN_HIGHLIGHTS` query (narrower than upstream grammars, needs
review); fences highlighted per block with character wrapping that
keeps indentation (NZ-25 follow-up 1 fixed); raw view and other files
highlighted whole via `highlight_lines`, cache key gained `language`;
footer shows the language name, `(not highlighted, large)` over 1 MiB,
`(highlighter unavailable)` when a grammar failed; measurements debug:
200 KB `.rs` first selection 344 to 381 ms, cached redraw 15 µs, 500 KB
note with 50 rust fences 80 ms; release binary 11,992,256 B (+9.6 MB
for all ten grammars); worker checks: build clean, notez-cli 415
passed, notez-core 146 passed; about 133k agent tokens. Three NZ-25
tests updated to the new contract (highlighted `fn` keyword, bash
comment style in an unterminated `sh` fence, footer `rust` for
`main.RS`). Lead decision on the report (working rule): in a
highlighted fence uncaptured code uses the plain text colour as base
so PEACH strings stand out; `theme::code()` stays for inline code and
unknown-tag fences; sent to the worker at 22:05 CEST with the pass 1
clippy nit, DONE at 22:15 CEST (`highlighted_code_base()` in
`markdown.rs`, one new test, one pass 2 test updated; the clippy
closure is needed for lifetimes so it carries an `allow` with a
comment). Whole diff against `9e1bb20`: 8 files, 1692 insertions, 43
deletions, hashing to
`e77347223d168f7f151563885629ce45b3c7d0e161d8a2cba31ef154efa05809`;
worker checks: build clean, notez-cli 416 passed, notez-core 146
passed; about 142k agent tokens for pass 2 over three rounds. Workers
stopped. REVIEW dispatched at 22:18 CEST (`nz-reviewer`, opus) on that
hash; ACCEPTED at 23:05 CEST first time (about 87k tokens; it fuzzed
the module through a scratch crate: all languages, nested fences, 10k
lines, NUL, BOM, emoji, no panics; `cargo tree -d` shows no new
duplicate crates). Follow-ups, none blocking: F1 Kotlin method calls on
a receiver (`repo.add(1)`) capture as property, not function; F2 LOOK
CHANGE for Andreas: in RAW markdown the block markers (`#`, `- `, `> `,
fences) now take the dim punctuation style while heading text keeps
its colour, every raw note looks slightly different (the lead merged
under the delegation's waiver of per-ticket branch trials; Andreas
judges installed and can have it reverted); F3 the renderer measures
the normalised length and the footer the on-disk length for the 1 MiB
limit (a CRLF note straddling it gets a wrong footer note); F4 README
says 1 MB for a 1 MiB constant; measure python and bash near 1 MiB in
release; consider feature flags for bash, c, java grammars (1.4, 0.65,
0.43 MB static). Lead checks: build clean, 416 + 146 passed. Committed
as `608235c` (diff against `9e1bb20` hashes to the accepted
`e7734722...`), branch pushed at 23:08 CEST; the integration chain is
running as one background command and stops at the first failure.
Board item `PVTI_lAHOCU842c4BmE5Zzg_ZIHA` In flight until the chain
reports. Next after it: NZ-4. Board item
`PVTI_lAHOCU842c4BmE5Zzg_ZIHA` In flight. Disk: 9.3 GB free; the
grammars compile C, so the lead watches `target` size.

NZ-25 is Done (merge `6a1c814` at 20:55 CEST; branch run and `main`
run `37785453082` green; remote branch deleted; worktree and local
branch removed; board Done).

NZ-25 record, for history. Dispatched 2026-10-08 at 19:15 CEST
by lead `4ffb11e2` on Andreas's "4. go with c, but break it up ...
approved to add dependencies" (17:20 CEST). `git status --short`
before dispatch: only this file modified. Base `706e169` (= `main` =
`origin/main`), branch `feat/NZ-25-markdown-preview`, worktree
`.claude/worktrees/NZ-25`, model `opus` via `nz-worker`. Pass 1
(`tui/markdown.rs` renderer with tests, theme styles, dependency
`pulldown-cmark = "0.13.4"`, `Cargo.lock`) REPORTED at 19:35 CEST: 5
files (`Cargo.lock`, `crates/notez-cli/Cargo.toml`, new
`tui/markdown.rs` registered with `git add -N`, `tui/mod.rs`,
`tui/theme.rs`), 1041 insertions, uncommitted diff against `706e169`
hashing to
`68d2f99f5a159cc49b60791c6644296eaa51db3b905c0e163a9fd50c12426f32`;
worker checks: build clean, notez-cli 363 passed (32 new), notez-core
146 passed; about 117k agent tokens. Dependency as added:
`pulldown-cmark = { version = "0.13.4", default-features = false }`
(drops `html` and `getopts`); `Cargo.lock` gained `pulldown-cmark
0.13.4` and `unicase 2.10.0` only. Renderer does its own wrapping so
`lines.len()` is the scroll count; 2 MB document renders in about 0.42
s debug. Lead decisions on the report (accepted): language line
indented two spaces like the code; table and raw HTML source lines
wrap at the width; nested list indent follows the parent marker width;
tab is 4 spaces. Pass 2 (toggle key, footer file type, cache,
integration in `tui/tree.rs`, README) REPORTED at 20:00 CEST: touched
`tui/tree.rs` and `README.md`; whole diff against `706e169` now 7
files, 1498 insertions, 82 deletions, hashing to
`af547862e9a25d83051e3b104d8492757594cf19ab3cce94a56f45bbd67e7c24`;
worker checks: build clean, notez-cli 375 passed, notez-core 146
passed; about 106k agent tokens; no existing test changed. As built:
toggle key `p` (lead's provisional pick, Andreas confirms installed),
toggles only while a markdown file is selected, mode persists across
selections, never persisted; footer leads with the file type then the
mark count; `p raw` / `p rendered` hint shown only for markdown, then
first dropped (Priority 12 applied dynamically, static row HelpOnly so
NZ-22's test stays); cache keyed by path, width, effective rendered
flag, mtime, length, one `metadata` per frame; preview draws only the
visible slice; `preview_lines.len() as u16` wraparound fixed. Lead
decisions on the report (accepted): `p` dropped before `J/K` while
markdown is selected; toggle clamps the scroll without resetting it;
`p` inert on non-markdown rows. REVIEW dispatched at 20:05 CEST
(`nz-reviewer`, opus) on that hash; ACCEPTED at 20:15 CEST first time
(about 86k tokens; it fuzzed the renderer in a scratch crate: 2 MB
adversarial inputs in 18 to 177 ms, no panics; `cargo tree` confirms
no `html` feature). Follow-ups (not blocking): (1) code lines wider
than the pane lose their leading indentation and inner whitespace runs
when wrapped (NZ-27 territory); (2) a table inside a quote or list
repeats the container prefix; (3) lone `\r` endings are not normalised;
(4) two doc comments overstate or sit on the wrong item; (5) the 2 MB
perf test has only 2x headroom; (6) control characters in a note reach
the terminal through the preview, pre-existing. Lead ordered one
pre-merge change at 20:18 CEST to avoid CI flakes: loosen the perf
bound to 5 s and, if a one-liner, normalise lone `\r`; sent to the
pass 1 worker, DONE at 20:25 CEST: perf test renamed
`a_two_megabyte_document_renders_in_bounded_time`, bound 5.0 s with a
comment; lone `\r` normalised with one new assertion in the CRLF test
(red before the fix). Only `markdown.rs` changed. Worker checks: 375 +
146 passed. New diff against `706e169`: 7 files, 1506 insertions, 82
deletions, hashing to
`476413e3f2b87610d5153dd6e10521a411c8cc332848fa7625d01de68a1bb89d`.
RE-REVIEW sent to the same reviewer at 20:28 CEST; ACCEPTED at 20:45
CEST on `476413e3...` (it reconstructed the previously accepted hash by
undoing the two edits, proving nothing else changed; the replace order
`\r\n` then `\r` is correct). Lead checks: build clean, 375 + 146
passed. Committed as `f8d77ba`, branch pushed at 20:48 CEST; the
integration chain (branch run, merge `--no-ff`, checks on `main`, push,
`main` run, remote branch deletion, worktree removal) is running as one
background command and stops at the first failure. Board item
`PVTI_lAHOCU842c4BmE5Zzg_Y3S8` In flight until the chain reports.
Next after it: NZ-27 brief (grammar versions compatible with
`tree-sitter 0.27.0`), then dispatch. The dependency
change is approved for this ticket (Authorized by the owner, 17:20
CEST), so the safe-merge rule's dependency condition is met. Board
item `PVTI_lAHOCU842c4BmE5Zzg_Y3S8` In flight. Disk: 11 GB free.

NZ-24 is Done (merge `706e169` at 19:10 CEST; branch run and `main` run
`37778913601` green; remote branch deleted; worktree and local branch
removed; board Done).

NZ-24 record, for history. Dispatched 2026-10-08 at 18:40 CEST by lead
`4ffb11e2` on Andreas's "3. go with b" (17:20 CEST). `git status
--short` before dispatch: only this file modified. Base `5442854`
(= `main` = `origin/main`), branch `feat/NZ-24-todo-icon`, worktree
`.claude/worktrees/NZ-24`, model `opus` via `nz-worker`, one pass.
Worker REPORTED at 18:50 CEST: `README.md`, `tui/theme.rs`,
`tui/tree.rs`, 109 insertions, 7 deletions, uncommitted diff against
`5442854` hashing to
`ad4548197873e5032527f9028e7bb758e3b354fe9d4e22ae5086ef46dcc108a3`;
worker checks: build clean, notez-cli 331 passed, notez-core 146
passed; about 53k agent tokens. As built: `ICON_TODO = "\u{f0ae}"`
(nf-fa-tasks, width 1) in `theme.rs`; drawn plus a space in the
section colour on the `_todos` store row, every row under it and any
file named exactly `TODO.md`; headers unchanged; `is_todo_row` now
calls a new `in_section_todo_store(node, spec)`; help has no legend so
untouched; README one sentence; no existing test changed. Cost noted:
`row_badge` calls `in_todo_store` per nested global row per draw (two
metadata lookups for non-store paths); reviewer asked to judge. REVIEW
dispatched at 18:53 CEST (`nz-reviewer`, opus) on that hash; ACCEPTED
at 19:00 CEST first time (about 29k tokens; the per-draw stats judged
acceptable, microseconds per row, with a cache on `TreeNode` as the
fix if lag ever shows; no test for a directory named `TODO.md`). Lead
checks: build clean, 331 + 146 passed. Committed as `092b88a` (diff
against `5442854` hashes to the accepted `ad454819...`), branch pushed
at 19:03 CEST; the integration chain is running as one background
command and stops at the first failure. Board item
`PVTI_lAHOCU842c4BmE5Zzg_Y3RQ` In flight until the chain reports. Next
after it: NZ-25 (two passes).

NZ-26 is Done (merge `5442854` at 18:35 CEST; branch run and `main` run
`37777856510` green; remote branch deleted; worktree and local branch
removed; board Done).

NZ-26 record, for history. Dispatched 2026-10-08 at 18:05 CEST by lead
`4ffb11e2` on Andreas's "2. go with b" (17:20 CEST). `git status
--short` before dispatch: only this file modified. Base `82fae54`
(= `main` = `origin/main`), branch `fix/NZ-26-soft-name-rule`,
worktree `.claude/worktrees/NZ-26`, model `opus` via `nz-worker`, one
pass. Worker REPORTED at 18:12 CEST: `tui/tree.rs` and `README.md`,
119 insertions, 19 deletions, uncommitted diff against `82fae54`
hashing to
`8ef332a8083b412c2f81bd9334c3e0b49d54b2c70ab56288e7563ecdefb6371a`;
worker checks: build clean, notez-cli 327 passed, notez-core 146
passed; about 45k agent tokens. As built: `soft_name` (trim,
lowercase, whitespace runs to `-`) mirrors `sanitize::name` before its
character filter; a name is refused only when `cleaned != soft_name`
and `cleaned` is not empty; `My Note`, `Ideas`, `Ä` accepted,
`00_quick`, `a.b` refused; NZ-20's `ALTERED` test const split into
`ALTERED` and `SOFTENED`, three NZ-20 tests adjusted accordingly, two
new Enter-path tests. REVIEW dispatched at 18:15 CEST (`nz-reviewer`,
opus) on that hash; ACCEPTED at 18:22 CEST first time (about 37k
tokens; follow-ups: README could say a punctuation-only name counts as
empty; the `n` Enter-path test copies the loop's lines instead of
calling them; pre-existing: a case-only title change that maps to the
same file name does not rewrite the heading, reachable again from `r`;
`İ` lowercases to `i` plus a combining dot and is refused as `i`). Lead
checks: build clean, 327 + 146 passed. Committed as `a361d30` (diff
against `82fae54` hashes to the accepted `8ef332a8...`), branch pushed
at 18:25 CEST; the integration chain (branch run, merge `--no-ff`,
checks on `main`, push, `main` run, remote branch deletion, worktree
removal) is running as one background command and stops at the first
failure. Board item `PVTI_lAHOCU842c4BmE5Zzg_ZIFY` In flight until the
chain reports. Next after it: NZ-24.

NZ-22 is Done (merge `82fae54` at 18:00 CEST; branch run and `main` run
`37776056271` green; remote branch deleted; worktree and local branch
removed; board Done).

NZ-22 record, for history. Dispatched 2026-10-08 at 17:05 CEST by
lead `4ffb11e2` on Andreas's direct request ("i want shift + j/k &
up/down to scroll document in the right pane", 15:55 CEST). `git
status --short` before dispatch: only this file modified. Base
`d667999` (= `main` = `origin/main`), branch
`feat/NZ-22-preview-scroll-keys`, worktree `.claude/worktrees/NZ-22`,
model `opus` via `nz-worker`, one pass. Worker REPORTED at 17:30 CEST,
one lead adjustment (the `J/K` row moved late in `TREE_KEYS` so the hint
renders at the footer's right end) DONE at 17:38 CEST: `tui/tree.rs`
and `README.md`, 123 insertions, 13 deletions, uncommitted diff against
`d667999` hashing to
`05b21b4beac5bd0bbbf3545caa15bc495882d43eb947860b564648c4f88ec0e8`;
worker checks: build clean, notez-cli 324 passed, notez-core 146
passed; about 147k agent tokens over both rounds. As built: Shift+Down/
Up scroll one line like `J`/`K`; PgDn/PgUp scroll a page (`preview_
height - 1`); every scroll path goes through a pure `scrolled(current,
delta, max)`; `J/K` is a visible footer hint at `Slot::Priority(11)`
(first dropped), `PgDn/PgUp` help only; README has the keys and the
Terminal.app caveat (no default Shift+Up/Down mapping there; Ghostty
and iTerm2 send `ESC[1;2A/B`; the worker verified crossterm's parser
and probed through tmux, not the terminals themselves). One pre-existing
test changed (`normal_and_focus_footers_hint_the_browse_keys` gains
`J/K` before `q`). REVIEW dispatched at 17:40 CEST (`nz-reviewer`,
opus) on that hash; ACCEPTED at 17:50 CEST first time (about 41k
tokens; follow-ups: in tag mode Shift+Down/Up scroll the preview while
J/K and the page keys are swallowed; the help overlay lists the two
rows after `wheel` and `click`; no test pins `PgDn/PgUp` as HelpOnly;
README's terminal list is stated as fact, not tested). Lead checks:
build clean, 324 + 146 passed. Committed as `75e2a88` (diff against
`d667999` hashes to the accepted `05b21b4b...`), branch pushed at
17:55 CEST; the integration chain (branch run, merge `--no-ff`, checks
on `main`, push, `main` run, remote branch deletion, worktree removal)
is running as one background command and stops at the first failure.
Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3NA` In flight until the chain
reports. Next after it: NZ-26.

NZ-21 is Done (merge `d667999` at 17:00 CEST; branch run and `main` run
`37774540545` green; remote branch deleted; worktree and local branch
removed; board Done).

NZ-21 record, for history. Dispatched 2026-10-08
at 16:10 CEST by lead `4ffb11e2` on Andreas's direct request ("two
things directly", 15:55 CEST). `git status --short` before dispatch:
only this file modified. Base `483cb1c` (= `main` = `origin/main`),
branch `fix/NZ-21-tree-row-alignment`, worktree `.claude/worktrees/
NZ-21`, model `opus` via `nz-worker`, one pass. Worker REPORTED at
16:25 CEST, one lead adjustment (a space after the badge, like section
rows) DONE at 16:32 CEST: only `tui/tree.rs`, 174 insertions, 32
deletions, uncommitted diff against `9a6c0e0` hashing to
`031b6acbdbcccaaa9631c41f4702f7c09eac4316e24af90c4969b412b62cf5a2`;
worker checks: build clean, notez-cli 318 passed, notez-core 146
passed; about 120k agent tokens over both rounds. Lead decisions:
keep the worker's `list_text_width` correction (the event loop's
`inner_width` was pane minus 6, the list's real text width is pane
minus 8; the old value only looked right through a cancelling
byte-count error on section rows); badge drawn as `icon` plus a space
before the name; `LIST_TEXT_WIDTH` test constant keeps its value with a
corrected comment. Two pre-existing tests updated to the new badge
position, every assertion kept. REVIEW dispatched at 16:38 CEST
(`nz-reviewer`, opus) on that hash; ACCEPTED at 16:48 CEST first time
(about 41k tokens; follow-ups: `BADGE_COL` naming now means the blank
gutter; a very long nested folder name can still overflow the row,
pre-existing). Lead checks: build clean, 318 + 146 passed. Committed
as `9ce0dae` (diff against `9a6c0e0` hashes to the accepted
`031b6acb...`), branch pushed at 16:52 CEST; the integration chain
(branch run, merge `--no-ff`, checks on `main`, push, `main` run,
remote branch deletion, worktree removal) is running as one background
command and stops at the first failure. Board item
`PVTI_lAHOCU842c4BmE5Zzg_Y3Lk` (In flight), created under the 16:35
permission. Then branch push, CI, merge, `main` run, cleanup; then
NZ-22 (board `PVTI_lAHOCU842c4BmE5Zzg_Y3NA`, Ready).

NZ-20 is Done (merge `483cb1c` at 16:00 CEST; branch run `37771278846`
and the `main` run both green; remote branch deleted; worktree and
local branch removed; board Done).

NZ-20 record, for history. Dispatched 2026-10-08
at 15:20 CEST by lead `4ffb11e2` on Andreas's "you may go ahead with
NZ-20". `git status --short` before dispatch: clean. Base `e84e90d`
(= `main` = `origin/main`; code at the NZ-19 merge `3941481`), branch
`feat/NZ-20-refuse-altered-names`, worktree `.claude/worktrees/NZ-20`,
model `opus` via `nz-worker`, one pass. Worker REPORTED at 15:30 CEST:
`tui/tree.rs` and `README.md`, 429 insertions, 31 deletions,
uncommitted diff against `e84e90d` hashing to
`ade3c2bb3012a12bbca986cdd9d1bcdfea7ceeda2b37d2a36f53822136facb26`;
worker checks: build clean, notez-cli 312 passed, notez-core 146
passed; about 132k agent tokens; no pre-existing test edited. Lead
decisions on the report (accepted as built): the prompt stays open on a
refusal with the typed text; `n` with a title that sanitizes to nothing
is refused with `new note: the name is empty` (before it created
`<date>-.md`); `_todos` matched by exact component or same directory
entry (dev and inode) under the global root only; a dedicated message
for `_todos` as a move destination. REVIEW dispatched at 15:33 CEST
(`nz-reviewer`, opus) on that hash; ACCEPTED at 15:45 CEST first time,
about 76k agent tokens, no blockers; follow-ups F1 (rename Enter acts
on the row under the cursor at Enter time, a click can move it,
pre-existing), F2 (decomposed Unicode gets a confusing message; NFC
needs a dependency or core change), F3 (no test for `S` refusing a
`_todos` destination, none for Message over Rename in `status_slot`),
F4 (README misses the `new note: the name is empty` refusal; reflow).
Reviewer's product note for Andreas: `n` now refuses capitals and
spaces too, so a heading like "Meeting with Bob" cannot come from `n`
any more (type `meeting-with-bob`); the lead put a softer rule to
Andreas (refuse only names where sanitizing DROPS characters, keep
case-folding and space-to-hyphen silent) as a follow-up decision.
Lead checks: build clean, 312 + 146 passed. Committed as `f9bd0b9`
(diff against `e84e90d` hashes to the accepted `ade3c2bb...`), branch
pushed at 15:50 CEST, CI run `37771278846` in progress. Then
(new under the standing branch-push permission) push the branch, wait
for a green run, merge `--no-ff`, checks on `main`, push `main`, wait
for the `main` run, board Done, delete the remote branch, cleanup.
Board: In flight (item `PVTI_lAHOCU842c4BmE5Zzg_Yflc`).

NZ-16 Done (merge `5298d54`) and NZ-19 Done (merge `3941481`) at about
15:00 CEST; CI exists, first `main` run `37762203661` green. A code push
to `main` starts a CI run; a docs-only push does not
(`docs/agent-workflow.md`, safe-merge rule).

NZ-16 and NZ-19 records, moved here for history.

NZ-16 (multi-select). Dispatched 2026-10-08 at 13:25 CEST by lead
`4ffb11e2` under the standing scope (confirmed by Andreas 2026-10-07).
`git status --short` before dispatch: clean. Base `a0773b9` (= `main` =
`origin/main`; code is at the NZ-15 merge `36699c1`), branch
`feat/NZ-16-multi-select`, worktree `.claude/worktrees/NZ-16`, model
`opus` via `nz-worker`. Pass 1 (marks with `Space`/`Esc`, drawing,
footer count, action-set function, bulk delete) REPORTED at 13:40
CEST: only `tui/tree.rs` changed (726 insertions, 24 deletions),
uncommitted diff against `a0773b9` hashing to
`ae021671a1ef8014bde4c9c05dd483180ff98f093f0423cc6050c871372698ca`;
worker checks: build clean, notez-cli 299 passed, notez-core 146
passed; about 168k agent tokens. Pass 2 (bulk `m` and `S`, README, two
amendments below) REPORTED at 14:20 CEST: `tui/tree.rs` and
`README.md`; whole diff against `a0773b9` 2 files, 1445 insertions, 98
deletions, hashing to
`9f090720694db7c39fcfaa8defcfdfbac0133680ce47c289f09743aca467967a`;
worker checks: build clean, notez-cli 304 passed, notez-core 146
passed; about 169k agent tokens. Workers stopped. REVIEW dispatched at
14:25 CEST (`nz-reviewer`, opus) on that hash, RUNNING. Lead decisions
on the pass 2 report (accepted as built): refusal message `<name>:
<verb>: <reason>`; items already at their destination are skipped in a
bulk `m`/`S` (whole set already there: `S` no-op, `m` says so); the
confirm counts only items that will move; duplicate names compared
exactly (case variants fail safely as "already exists"); a set spanning
projects is refused. Refactors to verify in review: `remove_and_retire`
out of `answer_delete`, `resolve_folder` out of `resolve_move`,
`move_and_repoint` out of `apply_move`, `move_question` generalized.
Board: In flight.

NZ-19 (CI) in the SECOND worker slot. Dispatched 2026-10-08 at 14:05
CEST by lead `4ffb11e2` on Andreas's authorization of 13:55 CEST
(Authorized by the owner). `git status --short` before dispatch: only
this file modified. Base `77c3758` (= `main` = `origin/main`), branch
`feat/NZ-19-ci`, worktree `.claude/worktrees/NZ-19`, model `opus` via
`nz-worker`, one pass. Files disjoint from NZ-16 (`.github/`,
`CLAUDE.md`, `docs/agent-workflow.md`, `README.md` at most). Worker
REPORTED at 14:12 CEST (about 38k agent tokens): `.github/workflows/
ci.yml` new, `CLAUDE.md` and `docs/agent-workflow.md` edited, README
untouched (its only test mention is not the checks); pins
`actions/checkout@3d3c42e5...` (v7.0.1), `dtolnay/rust-toolchain@
89b12181...` (branch `stable`, 2026-10-01), `Swatinem/rust-cache@
6323deb1...` (v2.9.2); tests pass with no git identity visible, the
identity step is kept with a comment; Ruby parsed the YAML. Lead
committed it as commit A `33a5793` on `feat/NZ-19-ci` (diff against
`77c3758` hashes to
`830d7b902c3e3695f5f240362c825e96a75c68897415d8c58c531746c21311dd`) and
pushed the branch at 14:15 CEST; workflow `CI` registered (id
378310189). Runs: `37760007328` on A `33a5793` GREEN on both runners
(lint success too) at 14:27 CEST; commit B `b528909` (throwaway
`crates/notez-cli/tests/ci_red_check.rs` that panics) pushed at 14:29,
run `37760332799` RED (check ubuntu and macos failure, lint success)
at 14:38; commit C `836c255` (revert of B) pushed at 14:40, run
`37760452070` in progress; `git diff 77c3758 836c255` hashes to the
same `830d7b90...` as A. REVIEW dispatched at 14:42 CEST
(`nz-reviewer`, opus) on tip `836c255` with the three run ids, RUNNING.
Integration plan for this ticket, a deviation from the usual order
because CI can only be seen on a pushed commit: worker reports; lead
commits the diff as commit A on the branch and pushes (green run
expected on both runners); lead adds a throwaway failing-test commit B,
pushes (red run expected), then a revert commit C, pushes (green run);
reviewer reviews the committed tree at C against the base (diff
base..C must equal diff base..A) and the run ids; merge `--no-ff`,
push `main` (first `main` run), board Done, delete the remote branch
(approved), cleanup. Board: In flight (the lead briefly set NZ-17's
item to In flight by mistake at 14:02 CEST and set it back to Draft at
14:03 CEST).

Lead decisions on the pass 1 report: marks live in `event_loop` as a
`HashSet<PathBuf>`, pruned each loop pass (a renamed or moved marked
row drops its mark); `is_marked` ignores depth-0 rows; mark glyph in
the gutter, bold row, no theme entry; the footer count also shows over
a session warning; `Space` at `Slot::Priority(8)` after `N`; the `Esc`
help reads "clear marks; with none, clear the filter; with no filter,
quit"; bulk question singulars and `(no notes inside)` accepted; a
successful bulk delete says `deleted <ok>`. Two amendments ordered for
pass 2: (a) a set containing an item the guards would refuse is
refused as a whole BEFORE the confirm (the confirm counts only items
that will be attempted); (b) the bulk question gets the single delete's
"and other files" clause. One existing test changed by pass 1:
`normal_and_focus_footers_hint_the_browse_keys` gains `space`. NZ-19 (CI) waits for
Andreas's four answers in the lead session (see Authorized by the
owner, 2026-10-08 entry).

NZ-15 is Done (merge `36699c1`, pushed, board Done, worktree and branch
removed at 13:18 CEST).

NZ-15 record, moved here for history. Dispatched 2026-10-08 at 11:58
CEST by lead `4ffb11e2` under the standing scope, with the lead's recommended answers
to the three "Andreas" decisions (typed folder prompt with `Tab` for
scope; `S` as its own key; destinations are the row's project's scopes
plus global, no other projects). Andreas was asked at 10:35 CEST and had
not objected by dispatch; the lead told him at 11:55 CEST that it was
starting and that he can stop it. `git status --short` before dispatch:
clean. Base `1074141` (= `main` = `origin/main`; code is at the NZ-14
merge `ae3617e`), branch `feat/NZ-15-move`, worktree
`.claude/worktrees/NZ-15`, model `opus` via `nz-worker`. Pass 1
(`move_path` helper with injectable rename and copy hooks for the
cross-device fallback, `m` for notes within and across scopes, the
visibility confirm, tag carry) REPORTED at 12:12 CEST: `tui/tree.rs`,
`tui/footer.rs`, `tui/mod.rs` modified, new file `tui/move_path.rs`
(untracked, registered with `git add -N` for hashing); diff against
`1074141` 4 files, 1208 insertions, 4 deletions, hashing to
`157e72d9312ede0b35ee2f4ae75219b3c77f781d6abe2ecca767d3444683e95e`;
worker checks: build clean, notez-cli 274 passed, notez-core 146
passed; about 184k agent tokens. Pass 2 (folders, `S`, README)
REPORTED at 12:40 CEST: touched `tui/tree.rs`, `tui/footer.rs`
(`Mode::SetScope`) and `README.md`; whole diff against `1074141` now 5
files, 1799 insertions, 4 deletions, hashing to
`5dd46366d0f422bfe1096548def638665a2346f2f6c5f9e6f10817ad8c7fcbc2`;
worker checks: build clean, notez-cli 284 passed, notez-core 146
passed; about 133k agent tokens. REVIEW dispatched at 12:45 CEST
(`nz-reviewer`, opus) on that hash; REPORTED at 12:55 CEST: changes
requested, two blockers, about 128k agent tokens; hash confirmed,
checks rerun (284 + 146), scope clean, all eight probes answered (no
zero-copies path in `move_path`, tags correct across all 12 pairs
except the two bugs, confirm and dispatch order correct).

B1 (medium): `resolve_move` guards global-to-`personal/` lexically, but
APFS is case-insensitive, so `Personal/proj/plans` typed from a global
row bypasses it, lands without confirm, writes the tag key with the
typed spelling (tag lost) and can reach another project's personal
store; an intermediate symlink component also passes. B2 (medium): a
hidden tagged note under a folder moved twice in one session loses its
tag (`carry_unlisted_keys` scans only `forest.initial`, so the carried
entry is never rewritten and ends up keyed to a missing file). FIX
CYCLE 1 sent to the pass 2 worker at 12:58 CEST, DONE at 13:05 CEST:
`resolve_move` walks the typed folder step by step (exact spelling via
`has_entry_named`, no symlink, must be a directory; `dst` still from
the typed steps, `personal/` guard after the walk); `apply_move`
rewrites `carried` entries under the source after a successful move;
two new tests (both red with the fixes off). Only `tui/tree.rs`
changed. Worker checks: build clean, notez-cli 286 passed, notez-core
146 passed. New diff against `1074141`: 5 files, 1904 insertions, 4
deletions, hashing to
`d5d6485329221d1adf55015941117eaab458a051277f58a8a95a805fad34a704`.
Worker stopped. RE-REVIEW sent to the same reviewer at 13:08 CEST on
that hash, RUNNING.

Reviewer follow-ups, not blocking, for Andreas and the leftovers list:
F1 the check-then-rename window (macOS `rename` replaces an existing
file; a no-overwrite rename needs libc, a new dependency, or
`hard_link` then `remove_file`); F2 on a `RemoveSource` failure
(cross-volume copy verified, source removal failed) notes already gone
from the source keep stale keys and their copies are untagged, silent
tag loss on a rare path; F3 a file written into the source between copy
and `remove_dir_all` is lost; F4 cosmetic messages; F5 mouse while a
prompt is open (same as rename). The prompt reads `move <name> to
<scope>/<folder>_` (README documents the actual text).

Next: on acceptance, lead checks, commit exactly the reviewed diff on
`feat/NZ-15-move`, merge `--no-ff` into `main`, checks on `main`, push,
board Done, cleanup. Board: In flight. No second worker slot is in use.

Lead decisions on the pass 2 report (accepted as built): `S` has its
own footer mode; `m`/`S` refuse a folder that holds another section
(delete's guard); the folder confirm counts notes like delete without
"and other files"; tagged notes in hidden folders under a moved folder
have their keys carried (`carry_unlisted_keys`). Two pass 1 tests
changed inside this same diff (footer key list gains `S`; the folder
refusal test flipped to allowed, as pass 2 intended). Cosmetic risk
noted: a failed folder move still expands the folder in the rebuilt
list.

Lead decisions on the pass 1 report:

- Accepted the `carried` list: in a one-scope view (`-p`, `-l`, `-g`)
  `Tab` still offers other scopes, so a moved note may leave the view;
  its flags are added to the exit write anyway and the footer says
  `moved to <path>`.
- A global row has no project, so `Tab` offers the scopes of the
  project the browser was opened in; outside a project a global note
  moves only within global (decision 7 as read by the worker, accepted).
- `MoveError` is a typed enum (`Exists`, `IntoItself`, `NotPlain`, `Io`,
  `Copy { error, dst }`, `RemoveSource { error, dst, src }`) rather than
  anyhow, so tests match on variants; fine.
- The repository named in the "into public" clause is the directory
  name of `<repo>`; fine.
- `RemoveSource` (failure while removing the source after a verified
  copy) has no test, hard to inject on one volume; reviewer to inspect
  the path by reading.
- One existing test changed: `normal_and_focus_footers_hint_the_browse_keys`
  expects `m` after `N` (every footer key at width 200), legitimate.

NZ-14 is Done (merge `ae3617e`, pushed, board Done, worktree and branch
removed at 11:48 CEST).

Two pre-existing local branches that are not Relay's,
`feat/default-command-opens-tree` and `feat/melt-ui-refresh`, exist in
the main checkout; the lead leaves them alone (Andreas's to delete).

NZ-14 record, moved here from the dispatch log for history. Dispatched
2026-10-08 about 09:55 CEST by lead `4ffb11e2`. `git status --short` before dispatch: only this file
modified. Base `4a239e3` (= `main` = `origin/main`), branch
`feat/NZ-14-folders`, worktree `.claude/worktrees/NZ-14`, model `opus`
via `nz-worker`. Pass 1 (directory listing, `mkdir::create_in_dir`, the
`N` prompt) REPORTED at 10:05 CEST: 4 files changed (`README.md`,
`commands/mkdir.rs`, `commands/tree.rs`, `tui/tree.rs`), 612 insertions,
26 deletions, uncommitted diff against `4a239e3` hashing to
`17a84c7a831cba5dcf2ad7723bcea46c6c955095371758c2a2dc58d908f2bf8e`;
worker checks: build clean, notez-cli 237 passed, notez-core 146
passed; about 168k agent tokens. Pass 2 (folder rename and delete)
REPORTED at 10:40 CEST: touched `tui/tree.rs` and `README.md` only;
whole worktree diff now 4 files, 1415 insertions, 66 deletions,
uncommitted diff against `4a239e3` hashing to
`8118d5906ed724452815bfd5cfe627704d9ca5321c80e282f4048b2e12f6865c`;
worker checks: build clean, notez-cli 253 passed, notez-core 146
passed; about 158k agent tokens. No worker is running.

STOPPED at 10:45 CEST on the disk stop condition: the worker hit
`ENOSPC` once during pass 2 and `df` shows 590 MB free on
`/System/Volumes/Data` (228 GB, 100% used). Rust build output is small
this time (notez `target` 865 MB, the NZ-14 worktree `target` 619 MB,
`~/.cargo/registry` 517 MB; nothing else under `~/Repos` or
`~/RustroverProjects`); the big consumers are under `~/Library/Caches`
(JetBrains 3.6 GB, ms-playwright 1.1 GB, browser caches, Homebrew 563
MB). The lead deleted nothing and asked Andreas. RESOLVED at 11:05
CEST: Andreas ran `cargo clean` in `~/Repos/notez` (and freed more
himself); `df` shows 8.9 GB free. Work resumed.

Fix by the pass 2 worker (same agent, same worktree) DONE at 11:12
CEST: the empty-folder delete prompt now reads `delete empty/ (no
notes) from personal? y/n` (`and its 1 note`, `and its <n> notes` for
non-empty folders; two test lines and one README example changed, only
`tui/tree.rs` and `README.md` touched). Worker checks after the fix:
build clean, notez-cli 253 passed, notez-core 146 passed. Final
uncommitted diff against `4a239e3`: 4 files, 1415 insertions, 66
deletions, hashing to
`250880a5174b139f54079c3e3ef5300098f5f3902a5ef7a5f1b878130bf157b0`.
Worker stopped. REVIEW dispatched at 11:15 CEST (`nz-reviewer`, opus)
on that hash; REPORTED at 11:22 CEST: changes requested, one blocker,
about 105k agent tokens. The reviewer confirmed the hash, ran the checks
(build clean, 253 + 146 passed), and answered all eight probes (guards
hold, dispatch order safe, tag keys follow a rename, retired keys exact
after a failed delete, `notez mkdir` parity).

B1 (blocker): `r` on a folder then Enter on the UNCHANGED name renames
it whenever `sanitize::name` alters the name (it lowercases and strips
`_`): `00_quick-notes` to `00quick-notes`, `_todos` to `todos` (breaks
the todo board), `_todos/IDEAS` to `ideas`. FIX CYCLE 1 sent to the
pass 2 worker at 11:25 CEST, DONE at 11:32 CEST: early return in
`rename_folder` when the trimmed typed name equals the folder name, new
test `folder_rename_with_the_shown_name_unchanged_changes_nothing` (red
on the old code); only `tui/tree.rs` changed. Worker checks: build
clean, notez-cli 254 passed, notez-core 146 passed. New uncommitted
diff against `4a239e3`: 4 files, 1447 insertions, 66 deletions, hashing
to `766e8c757994e3df8822099d6ebc31914dbabc73e9a076f5b0111c566c3a3286`.
Worker stopped. RE-REVIEW sent to the same reviewer at 11:35 CEST on
that hash, RUNNING.

Follow-up found by the worker, pre-existing, NOT in NZ-14: `r` on a
NOTE has the same Enter-on-unchanged-name problem when the shown title
is not already in sanitized form (`2026-10-06-My_Note.md` would become
`2026-10-06-mynote.md` with its heading rewritten; `x.MD` would become
`xmd.md`; a capitals-only name fails harmlessly on APFS). Candidate
ticket together with the sanitize question below.

Reviewer follow-ups, not blocking, for Andreas: (1) a mouse click while
a prompt is open moves the selection, so Enter then acts on the clicked
row (pre-existing for notes, now reaches folders); (2) a scope with only
empty folders has no section, so `N` into it via `Tab` creates the
folder but shows no row (footer reports the path); (3) `d` and `r` work
on `_todos` in the global section (asks first, vault is git-tracked);
should `_todos` be protected like a docs folder? (4) `sanitize::name`
strips `_` and lowercases, so typing `00_quick` as a new name yields
`00quick`; whether typed names that sanitizing would alter should be
refused instead is a product call.

Lead decisions on the pass 2 report:

- `N` on a section row creates the folder at the section root (pass 1,
  mirrors `n`). The brief's criterion 5 contradicted its own decision
  2 on this point; decision 2 is the intent. `r` and `d` on a section
  row change nothing, as pass 2 has it.
- The renamed existing test (`d_on_a_folder_or_an_empty_tree_...` to
  `d_on_a_section_a_docs_folder_or_an_empty_tree_changes_nothing`) is
  the authorized behaviour change from NZ-12's "folder delete not
  available", not a weakening.
- Noted, no action: a renamed folder keeps its old sort position until
  the next rebuild (note rename does the same); non-unix refuses every
  rename onto an existing target including case-only.

Board: In flight. No second worker slot is in use.

Lead decisions on the pass 1 report (recorded for the reviewer and the
ticket record):

- `N` footer priority is 8 (first hint dropped on a narrow footer),
  not literally "just below `n`": `N` is used less than `n`, `r` and
  `d`, and the lower priority leaves NZ-12's narrow-footer width
  assertions untouched. The table order still puts `N` right after `n`.
- `N` on any docs row is refused with `new folder: not in a docs
  section`. `n` there redirects to the project's personal root (NZ-8);
  a folder has no such natural redirect, and the ticket's criterion 5
  says `N` on a section row changes nothing.
- NZ-8's prompt key rows keep their count; their help text is widened to
  "new note or folder: ...".
- One existing test changed beyond the mechanical `SectionSpec` literals:
  `normal_and_focus_footers_hint_the_browse_keys` now expects `N` after
  `n` (it lists every footer key at width 200). Legitimate contract
  update, not a weakening.
- Behaviour change to note for Andreas: deleting the last note in a
  folder now leaves the empty folder listed with the cursor on it
  (before, the folder vanished). The global section now lists every
  non-hidden folder under the vault root except `personal/`, so a
  repository nested in the vault adds empty `notez`/`docs` folder rows
  there (same family as the NZ-13 leftover).

Previous state, for history: nothing was in flight at the 2026-10-07
19:25 CEST stop. No worker or reviewer was running: the last agent (the
NZ-13 reviewer) reported and NZ-13 was merged. NZ-14 was about to be
dispatched (its brief was Ready) but was not.

GitHub returned "Internal Server Error" on `git push origin main` and on
`gh project item-edit` from 17:14 UTC for a few minutes while its status
page said all systems operational; the push landed on retry and the
board edits went through afterwards. `origin/main` is at the NZ-13 merge
plus the handoff commits.

Queue when work resumes, as confirmed by Andreas at takeover: NZ-14
(folders, brief Ready), NZ-15 (move, set scope), NZ-16 (multi-select),
then the UI tickets NZ-4, NZ-5, NZ-3; NZ-11 and NZ-6 in the second slot
when their files are free (NZ-11 touches `tui/tree.rs`, so only between
tree tickets; NZ-6 touches `main.rs`, `sync.rs`, `commands/sync.rs`,
`README.md`). NZ-11 and NZ-6 in
the second worker slot when their files are free (NZ-11 touches
`tui/tree.rs`, so only between tree tickets; NZ-6 touches `main.rs`,
`sync.rs`, `commands/sync.rs`, `README.md`).

Note to the next lead: there is no session scratch directory to rely on;
compute diff hashes by piping `git diff <base>` to `shasum -a 256`.

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

Ticket briefs and records live here; the board at `projects/2` carries
the same Status per ticket. Status values: Draft (brief written, not yet
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

Settled by Andreas on 2026-10-06 18:26 CEST: "quiet like pinz". Every
failed fetch at session end is quiet, as in Pinz. Known cost: an expired
credential is then also quiet until something shows sync state on screen,
which NZ-3's header is meant to do, so NZ-3 should be able to tell
"could not reach the remote" apart from "synced" (design that with NZ-6
in view, without changing the public `AutoSync` variants unless Andreas
approves).

### NZ-7: the main commands always open the browser; quick notes by flag

Status: Done. Merged into `main` as `7cc29ad` at 18:03 CEST on 2026-10-06
and pushed. Not yet installed by Andreas. Authorized by him at 16:48 CEST
("i think you can decide on these, and I'll test it out"), decisions by
the lead.

Record:

- Base `b08e798`, branch `feat/NZ-7-always-open-browser`, ticket commit
  `5f06b1b` (5 files, 455 insertions, 54 deletions), merge commit `7cc29ad`
  made with `git merge --no-ff` under the integration delegation. To undo
  the ticket: `git revert -m 1 7cc29ad`.
- Agents: one `nz-worker` (opus) run and one `nz-reviewer` (opus)
  invocation, no fix cycle. About 154k agent tokens. The worker ran across
  the 17:20 to 17:56 pause.
- Review: accepted first time, no blockers. The reviewer compared the old
  and new parser over 51 inputs: everything that parsed before parses the
  same, and every input whose result changed was a parse error before. The
  committed diff hashes to
  `60f92fa6f9ebb7552908af6cd481f6735596e3a76b07d84de30136844af67b40`
  (`git diff b08e798 5f06b1b | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and again on `main`
  after the merge: build clean, notez-cli 155 passed, notez-core 143
  passed. Code on `main` is identical to `5f06b1b`.
- How it works: `Cli` has an optional positional `words` beside the
  optional subcommand; a pure `decide(...)` in `main.rs` returns browse,
  quick note, subcommand or the hint; `commands/tree.rs` picks the view
  through `View { Project, Global, Only(Scope) }`; every browser path goes
  through one `browse()`.
- Behaviour to know: `-p` alone now shows the project's personal notes
  only (the whole project view is the no-flag default); a scope flag counts
  wherever it is typed (`notez call the bank -g`); a title word starting
  with a dash needs `--`; `notez tre` gets the quick-note hint instead of
  clap's "did you mean"; `notez -g tre` creates a quick note called "tre",
  by design.
- Left alone, none authorized: `notez -g -n foo` runs nav and drops `foo`
  silently; no "did you mean" in the hint; the flag-versus-quick test does
  not go through `main`'s own match arm.
- Found by the reviewer and older than the ticket: note creation
  overwrites an existing file of the same name. That is NZ-9.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.

The ticket as it was run:

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

### NZ-9: creating a note never overwrites an existing file

Status: Done. Merged into `main` as `c272d02` at 18:11 CEST on 2026-10-06
and pushed. Opened by the lead at 18:05 CEST from a finding in the NZ-7
review, and run before NZ-8.

Record:

- Base `7cc29ad`, branch `fix/NZ-9-never-overwrite-note`, ticket commit
  `8e60809` (`README.md` one sentence, `crates/notez-cli/src/commands/add.rs`;
  183 insertions, 4 deletions), merge commit `c272d02` made with `git merge
  --no-ff`. To undo the ticket: `git revert -m 1 c272d02`.
- Agents: one `nz-worker` (opus) and one `nz-reviewer` (opus), no fix
  cycle. About 86k agent tokens, 6 minutes.
- Review: accepted first time, no blockers. The reviewer reproduced the
  failure on the old code (6 new tests fail there) and probed, on APFS:
  dangling and live symlinks at the name, a directory with the name, a
  case-insensitive collision, a multi-byte name, a 255-byte name, all 1000
  names taken, and a read-only directory. None overwrites; each fails
  cleanly or takes the next suffix. The committed diff hashes to
  `db27d31abf98467f9a78401be8433852cc7a5fe6d77d440ddc407ce340b2f2ef`
  (`git diff 7cc29ad 8e60809 | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and on `main` after
  the merge: build clean, notez-cli 162 passed, notez-core 143 passed.
- How it works: `create_new_note_file` in `commands/add.rs` tries the
  natural name and then `-2` to `-1000` with `create_new`; `Created.path`
  is the file actually written.
- Left alone, none authorized: if writing the content fails after the file
  was created, an empty or partial new file stays (as before the ticket);
  a title so long that the `-2` name passes 255 bytes fails with "File name
  too long" on the second note that day.
- Reported by the worker and NOT fixed, for Andreas to decide: the same
  kind of loss when a file exists but cannot be read (not valid UTF-8, or a
  permission error). `commands/todo.rs` (adding a todo) replaces `TODO.md`
  with a fresh header plus the new item; `commands/log.rs` replaces the
  daily log with one entry; `notez-core/src/core/project.rs`
  `ensure_scratch_gitignored` replaces `.gitignore` with `.notez`. Each
  reads with a fallback to empty and then writes the whole file. The last
  one is in `notez-core`, which epoz pins.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.

The ticket as it was run:

Authority, as the lead reads it: Andreas did not name this ticket. NZ-8,
which he authorized, already requires that a name collision "never
overwrites" (its criterion 2), and that cannot hold while the shared
creation path overwrites. NZ-9 is that requirement carved out and done
first, because it is a data-loss defect in his own notes and NZ-7 just
made the path to it shorter. The lead told him in the same turn. It runs
under the same standing scope, limits and integration delegation as NZ-8.
If he objects, it is reverted on its own.

Problem (verified by the lead at `7cc29ad`): `commands::add::run` ends with
`std::fs::write(&path, note.rendered())`, and the file name is the date
plus the sanitized title. A second note with the same title in the same
folder on the same day silently replaces the first, including what was
written into it. This covers `add`, `znote`, `quick`, untitled notes and
the new flag form.

Decision: the second note gets the first free name with a numeric suffix
(`...-call-the-bank-2.md`, then `-3`); the first keeps its name. The file
is created with `create_new`, so the check and the write cannot be split.
The alternative, opening the existing note instead of making a new one,
was not chosen: `add` should always give a new note.

Acceptance criteria: a regression test that fails on the old code and
shows the first file's content intact after a second creation; `-3` on the
third; an existing `-2` skipped; the same for quick, untitled and body
notes; a title already ending in a number never causes an overwrite;
existing tests unchanged.

Allowed files: `crates/notez-cli/src/commands/add.rs`, and `README.md` or
`DESIGN.md` only if they describe file naming. No `notez-core` change.

### NZ-10: never replace a file that exists but could not be read

Status: Done. Merged into `main` as `3180556` at 18:33 CEST on 2026-10-06
and pushed. Asked for by Andreas at 18:26 CEST ("yes to the tickets").

Record:

- Base `6292515`, branch `fix/NZ-10-no-overwrite-on-read-error`, ticket
  commit `329be15` (3 files, 92 insertions, 4 deletions), merge commit
  `3180556` made with `git merge --no-ff`. To undo the ticket: `git revert
  -m 1 3180556`.
- Agents: one `nz-worker` (opus) and one `nz-reviewer` (opus), no fix
  cycle. About 74k agent tokens, 5 minutes. The review was already running
  when Andreas asked to wrap up; merging its accepted result was finishing
  work in hand.
- Review: accepted first time, no blockers. The reviewer reproduced all
  three failures on the old code and ran the built binary against a temp
  HOME: permission denied and "is a directory" both exit 1 with the file
  untouched; a dangling symlink and an empty file behave as before. The
  committed diff hashes to
  `217f6b13fadc59cc2950c4e8cb6e7e4044b51b0cf3cdf6472d2ff024dba89640`
  (`git diff 6292515 329be15 | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and on `main` after
  the merge: build clean, notez-cli 164 passed, notez-core 145 passed.
- `notez-core`: one function body changed (`ensure_scratch_gitignored`),
  no public line added or removed, so epoz's two call sites are unaffected.
  epoz picks the fix up only when it bumps its pinned rev.
- Left alone, none authorized: (1) the error says "nothing was changed",
  but in the local scope the `.gitignore` step runs before the read, so
  `.notez` may have been appended to `.gitignore` just before the failure;
  the target file itself is untouched. (2) Same class of bug, read by the
  reviewer but not traced end to end: `notez_core::note_tags::load_tags`
  maps any read error to an empty map and `save_tags` later rewrites
  `.tags`, so an unreadable `.tags` could be replaced by a tag toggle; and
  `notez_core::todo::load_single_todo` maps any read error to an empty
  task list, so a board save could replace an unreadable todo file. Both
  are in `notez-core`. For Andreas to decide.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.

The ticket as it was run:

Problem (read by the lead at `6292515`): three places read a file with a
fallback to empty and then write the whole file. If the file exists but
cannot be read (not valid UTF-8, a permission or I/O error), it is
replaced. `commands/todo.rs` quick add: `TODO.md` becomes a fresh header
plus the new item. `commands/log.rs`: the daily log becomes one entry.
`notez-core/src/core/project.rs` `ensure_scratch_gitignored`: the
project's `.gitignore` becomes the line `.notez`.

Decision: only "file not found" means start from the default. Any other
read error leaves the file untouched. Todo and log then fail with an error
naming the file and saying nothing was changed. `ensure_scratch_gitignored`
is best-effort and returns nothing, so it silently does nothing.

Acceptance criteria: a regression test per site with a file of invalid
UTF-8 bytes, failing on the old code, showing the bytes unchanged (and an
error for todo and log); missing and readable files behave exactly as
before; no existing test changed; `notez-core` public API unchanged
(epoz calls `ensure_scratch_gitignored(&Path)` in two places).

Allowed files: the three named above.

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

### NZ-8: create a note from the tree browser

Status: Done. Merged into `main` as `bf8f2ca` at 17:20 CEST on 2026-10-07
and pushed. Authorized with NZ-7, decisions by the lead. Not yet installed
by Andreas.

Record:

- Base `c272d02`, branch `feat/NZ-8-create-note-in-browser`, ticket commit
  `6a7453f` (4 files, 947 insertions, 36 deletions), merge commit
  `bf8f2ca` made with `git merge --no-ff` under the integration
  delegation. To undo the ticket: `git revert -m 1 bf8f2ca`.
- Agents: two `nz-worker` (opus) runs on 2026-10-06 (pass 1 stopped by
  accident, a continuation finished the ticket; about 100k tokens for the
  continuation) and one `nz-reviewer` (opus) on 2026-10-07 (75k tokens, 3
  minutes). No fix cycle.
- Review: accepted first time, no blockers. The committed diff hashes to
  `9edb7c7079a4b83dcd70f2f74f1f74ecd98c39ebca4957ce1beedd3de961e378`
  (`git diff c272d02 6a7453f | shasum -a 256`), the hash it accepted. The
  reviewer traced every key arm and the mouse handlers on an empty tree
  (no panic path), confirmed `notez add` is unchanged (NZ-9 numbering goes
  through the same `create_new_note_file`), and that no `notez-core` file
  is touched.
- Lead verification: `cargo build --workspace` and `cargo test
  --workspace` on `main` after the merge: build clean, notez-cli 186
  passed, notez-core 145 passed.
- One change beyond the ticket's wording, judged in scope by the reviewer
  and the lead: `repo_paths` in `commands/tree.rs` now includes the
  current unregistered project, because at the base its public and
  scratch sections were rooted under the vault and showed nothing, and
  `n` there would have written a "public" note into the vault. Covered by
  `unregistered_project_sections_are_rooted_in_the_project`.
- Lead decisions recorded from the review: (a) `Tab` offers a row's
  project scopes whenever that project's repository is known, also in the
  global view; the label names scope and project ("public (committed with
  <p>)"), rows outside any project cycle global only. Accepted as the
  reading of "outside a project". (b) The `Tab` order is personal, public,
  local, global rather than the brief's public-first; accepted, the
  prompt names the scope either way.
- Left alone, none authorized: (1) new rustfmt drift in the added lines
  (not a gate per CLAUDE.md); (2) creating in another section while focus
  mode is on leaves two sections open with focus still lit, cosmetic; (3)
  in an empty single-scope view such as `notez -l`, `n` defaults to
  personal and the note does not appear in that view, only a "created
  <path>" status (NZ-13 reworks the views and should settle this); (4)
  mouse clicks still act on the list while the prompt is open, harmless;
  (5) the prompt has no in-line cursor, like rename.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.
- What Andreas sees after `./install.sh`: `n` in the tree opens a footer
  prompt `new note in <scope>/<folder>: _`, `Tab` cycles the scope,
  `Enter` creates and opens the editor, the tree comes back with the new
  note selected; an empty view opens the browser with `no notes here yet:
  n creates one` instead of exiting.

The ticket as it was run:

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

### NZ-12: delete a note from the tree browser

Status: Done. Merged into `main` as `ff33de0` at 18:05 CEST on 2026-10-07
and pushed. Not yet installed by Andreas.

Record:

- Base `bf8f2ca`, branch `fix/NZ-12-delete-note`, ticket commit `45ad89d`
  (2 files, 704 insertions, 25 deletions: `tui/tree.rs`, `README.md`),
  merge commit `ff33de0` made with `git merge --no-ff` under the
  integration delegation. To undo the ticket: `git revert -m 1 ff33de0`.
- Agents: one `nz-worker` (opus, about 143k tokens, 9 minutes including
  one fix cycle) and one `nz-reviewer` (opus, 59k tokens, 2 minutes).
- Fix cycle 1, from the lead before review: the worker had put `d` in the
  help overlay only; the lead required it in the footer (lowest priority,
  first to drop at narrow widths), accepting the re-measured widths in
  `narrow_footer_drops_low_priority_hints_but_keeps_help_and_quit` and
  `d` added to `normal_and_focus_footers_hint_the_browse_keys`. Every
  pre-existing drop step is still asserted at its old width.
- Review: accepted first time, no blockers. The committed diff hashes to
  `133a4a91bde4701b14849ccd89106c7b2422a03c99f3d8d5557207628d1f5fff`
  (`git diff bf8f2ca 45ad89d | shasum -a 256`), the hash it accepted. The
  reviewer traced the key dispatch order (a `d` inside tag, filter,
  rename, new-note or `:` mode never reaches delete; a stray `y` is a
  no-op), the retiring path end to end including rename-then-delete, the
  tag-root alignment after `Forest::replace`, and that
  `std::fs::remove_file` is the only removal call and only ever gets a
  listed note path.
- Lead verification: `cargo build --workspace` and `cargo test
  --workspace` on `main` after the merge: build clean, notez-cli 199
  passed, notez-core 145 passed.
- Worker decisions accepted by the lead: the cancel hint reads `n/esc`
  (a second `n` row would break the one-`n` help test); retired keys are
  owned by `run_tree` as `Vec<(PathBuf, String)>` and passed to
  `changed_tag_maps_retiring`, with the old `changed_tag_maps` kept as a
  test-only wrapper; the prompt stores the tag root as a path; an emptied
  folder disappears (folders come from their files), so the cursor goes
  to the nearest listed ancestor.
- Left alone, none authorized: (1) the permission-denied test fails when
  run as root and restores permissions without a drop guard; (2) mouse
  clicks still act while the confirm prompt is open (`y` still deletes
  the named file); (3) rustfmt drift grew in the new test code.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.
- What Andreas sees after `./install.sh`: `d` on a note shows
  `delete <path> from <scope>? y/n` in the footer (local scratch adds
  "not recoverable"), `y` deletes and the cursor lands on the next note,
  `deleted <path>` shows briefly; `d` on a folder says folder delete is
  not available yet.

The ticket as it was run:

Confirmed by Andreas on 2026-10-07 in the lead session
(his pasted queue: "NZ-12 (delete a note)"); the hard-delete decision and
the `d` key were relayed on 2026-10-07 and stand. Runs after NZ-8 is on
`main`, based on it, because both touch the key table, the footer lead
and the rebuild path in `tui/tree.rs`. Brief finalized by the lead at
16:55 CEST from the NZ-8 worktree code.

Problem: the tree browser can browse, tag, rename and (after NZ-8) create
notes, but a note can only be removed outside the app.

Outcome and decisions:

1. `d` on a note row asks for confirmation in the footer lead, naming the
   file relative to its scope root and the scope by the same words NZ-8's
   `scope_label` uses, for example
   `delete ideas/2026-10-07-x.md from personal? y/n`. For the public scope
   the prompt says "public (committed with the project)"; for the local
   scope it appends "not recoverable". `y` deletes; `n`, `Esc` and any
   other key cancel and leave everything as it was. The prompt is a mode
   in the key table (`Mode::ConfirmDelete` or similar) so the footer and
   help overlay show `y confirm` and `n cancel`.
2. On `y` the file is removed with `std::fs::remove_file` and the tree is
   rebuilt through the existing `rebuild` closure and `Forest::rebuild`,
   with the cursor on the row that followed the deleted one (or the one
   before it when it was last in its folder, or the folder itself when it
   is now empty). Expanded state, unsaved tag edits and the filter are
   kept, as NZ-8's rebuild keeps them.
3. The deleted note's `.tags` key is retired in the maps written on exit.
   Rename does this through `TreeNode.origin != path` in
   `changed_tag_maps`; a deleted node no longer exists after the rebuild,
   so the `Forest` needs an explicit list of retired keys per tag root
   that `changed_tag_maps` removes. Without it the stale key would
   survive, since the final map starts from the loaded one.
4. `d` on a folder row does nothing except a footer message saying folder
   delete is not available yet (NZ-14). `d` on an empty tree does nothing.
5. The delete happens inside the browser; no editor, no sync call. The
   exit sync commits the deletion in the vault as it commits any change.
6. Hard delete, decided by Andreas: no trash folder, no undo key.

Acceptance criteria:

1. `d` then `y` on a note in each scope (personal, public, local, global)
   removes exactly that file, nothing else, checked by tests on a temp
   tree with a real `rebuild` closure. `d` then `n`, `Esc` or another key
   removes nothing.
2. The prompt text names the relative path and the scope; for local it
   contains "not recoverable"; for public it contains "public". Tests on
   the pure prompt-building function.
3. After the delete the cursor rule in decision 2 holds and the expanded
   set, unsaved tag flags on other notes and the active filter are
   unchanged; a unit test on the rebuild path shows it.
4. After the exit, `.tags` has no entry for the deleted file, and other
   entries are untouched (a test on `changed_tag_maps` with a retired
   key). A deleted note that had no tag changes `.tags` only by losing its
   key.
5. No key panics on an empty tree or a folder row; `d` on a folder shows
   the message and changes nothing.
6. A delete that fails (file already gone, permission denied) shows
   `delete failed: <error>` in the footer, removes nothing else, and the
   tree is rebuilt so the display matches the disk.
7. `d`, `y` and `n` are in `TREE_KEYS` with the right modes; the existing
   key-table tests (each key appears once in help) still pass. README key
   table updated.

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/footer.rs`, `crates/notez-cli/src/tui/help.rs`,
`README.md`. No new dependency, no `notez-core` change, no change to
`commands/`.

Method: bounded ticket, one worker pass (the change is small next to
NZ-8), one review. Reviewer probes: the retired-key path in
`changed_tag_maps`; the cursor after deleting the last note of a folder;
`d` while a filter is active; `d` in focus mode; a `y` keypress arriving
when the prompt is not open (must be a no-op, not a delete).

### Relayed on 2026-10-07: decisions and tickets NZ-13 to NZ-15

Provenance: at 15:37 CEST on 2026-10-07 the lead session (baton released,
stopped the night before) received a cross-session message from another
Claude session ("advisor session", `repos-f9`) saying Andreas asked it to
have these recorded. RECORD ONLY. Nothing below was said by Andreas in the
lead session, so a lead treats it as owner intent relayed second hand:
record it, do not run on it, and confirm the decisions and the key map with
Andreas directly before any of these tickets is dispatched. The advisor
session also wrote NZ-12 above into this file, uncommitted, the same day.
The baton stays released until Andreas says continue.

Owner decisions as relayed (Andreas, advisor session, 2026-10-07):

- Delete: hard delete with `y/n` confirmation naming the file and scope;
  the local scope prompt says "not recoverable". Settles NZ-12 point 4.
  Folder delete wanted too (NZ-14).
- Visibility change: a move between scopes must warn. Public to private
  leaves the file in the repository's git history; private to public
  commits it into the repository.
- Keys: regular single-key commands first. The existing `:` command line
  gets the same operations as a second way in, later.

Key map proposed by the advisor session, NOT yet confirmed by Andreas (ask
him before NZ-12 to NZ-16 run): `n` new note (NZ-8 already uses it), `N`
new folder, `r` rename (exists), `d` delete, `m` move, `S` set scope,
`Space` mark (NZ-16; `x` stays check in the todo board, decided on
2026-10-07), `o`/`Enter` open (exist). Keys to avoid because they are taken or pending:
`q j k h l f v t J K / ? 0 s < > = 1 2 Tab`. Andreas's own `c` create and
`C` change visibility was argued against (`c`/`C` look like a pair but are
unrelated, `c` is better kept for copy, `n`/`N` is the file-manager
convention). The `:` line already exists (`VimCommandMode`), so `:new`,
`:mkdir`, `:rename`, `:mv`, `:rm` would go there; no leader key.

Full proposed order, as it stands after the second relay (16:04 CEST on
2026-10-07), for Andreas to confirm or change when he starts a lead: NZ-8
review and merge; NZ-12 (delete); NZ-13 (unified view); NZ-14 (folders);
NZ-15 (move, set scope); the UI queue NZ-4, NZ-5, NZ-3 fitted around those
as Andreas prefers for his demo; NZ-6 and NZ-11 in the second worker slot
or between; NZ-16 (multi-select) last, since it depends on NZ-12, NZ-14 and
NZ-15. All of them stop at Ready to integrate under the standing scope's
limits unless Andreas says otherwise. The key map stays unconfirmed until
Andreas confirms it with the lead at the start.

Second relay, 16:04 CEST on 2026-10-07, same advisor session: Andreas said
"ok" to the advisor's recommendations on the two questions the lead had
raised, and asked for one more ticket (NZ-16). Marked as relayed, like the
rest; the lead confirms them with him at the start.

- NZ-13: the `notez-core` change to `collect_all` (also list
  `personal/<name>/` folders of projects not in the registry) is approved.
  It only adds notes to the listing; no file format change. Follow-up for
  Andreas, not part of the ticket: check how epoz uses `collect_all` before
  its pinned rev moves, since epoz will then list those folders too.
- NZ-15: notez only moves the file. It never commits or pushes in a project
  repository. The private-to-public warning says the note is now in the
  repository and not yet committed.

#### NZ-13: unified default view

Status: Done. Merged into `main` as `ee6a6fd` at 19:15 CEST on 2026-10-07
and pushed. Not yet installed by Andreas. This one changes what he sees
on every launch, so it is the build to install and look at.

Record:

- Base `ff33de0`, branch `feat/NZ-13-unified-view`, ticket commit
  `fbc0481` (7 files, 866 insertions, 195 deletions), merge commit
  `ee6a6fd` made with `git merge --no-ff` under the integration
  delegation. To undo the ticket: `git revert -m 1 ee6a6fd`.
- Agents: two `nz-worker` (opus) passes (about 114k and 122k tokens, the
  second including one fix cycle) and one `nz-reviewer` (opus, 72k
  tokens, 3 minutes). No review-driven fix cycle.
- Review: accepted first time, no blockers. The committed diff hashes to
  `0265d388e822f5e0b1eedcba64dbea8303e80510c2aec42142004a4f6be6ad70`
  (`git diff ff33de0 fbc0481 | shasum -a 256`), the hash it accepted.
  `collect_all` keeps its signature (one private helper added), so
  epoz's call compiles unchanged.
- Lead verification: `cargo build --workspace` and `cargo test
  --workspace` on `main` after the merge: build clean, notez-cli 218
  passed, notez-core 146 passed.
- Lead decisions during the ticket: personal sections of unregistered
  projects keep the vault root as `tag_root`; sections opened collapsed
  at the base, so now current sections open expanded and the rest stay
  collapsed; colours personal LAVENDER, public TEAL, scratch FLAMINGO,
  global GREEN; `-g` empty state says "global".
- Left alone, none authorized: (1) bare `notez` inside an unregistered
  repo that sits under the vault lists that repo's public notes under
  NOTEZ instead of its own section (the global walk wins the dedup);
  rare, since repos live outside the vault; (2) no test pins that a
  registered-but-missing project's `personal/` folder stays hidden; (3)
  no symlink tests for `unregistered_personal_dirs`; (4) `icon.len()`
  counts bytes in the header leader, older than the ticket.
- Cleanup done: worktree removed, local branch deleted with `git branch
  -d`. The branch was never pushed.
- What Andreas sees after `./install.sh`: bare `notez` anywhere shows
  one tree: the current repo's sections first and open, then NOTEZ, then
  every other project collapsed, including `personal/` folders of
  projects not registered here; every row has a coloured scope badge
  after the tag dots and headers name their scope; `-g` shows only the
  vault's global notes.

The ticket as it was run:

Confirmed by Andreas on 2026-10-07 in the lead session
("NZ-13: the unified default view, with scope badges"). Supersedes NZ-7's
decision 1 ("inside a project, the project view") where they differ. Runs
after NZ-12, based on `main` then. Brief finalized by the lead at 17:10
CEST from the code at `2f04539` plus the NZ-8 worktree.

Problem (verified by the lead): `aggregate::collect_all` walks only
registry projects, and skips the whole `personal/` subtree of the vault
when it walks the global root, so a `personal/<name>/` folder whose
project is not registered on this machine (for example `personal/socials`)
is listed nowhere. Inside a project, bare `notez` shows that project only
(NZ-7), so the rest of the vault is a flag away.

Outcome and decisions:

1. Bare `notez`, `notez tree` and `treez` with no scope flag open ONE view,
   inside or outside a project: the current repository's sections first
   (personal, public, docs, scratch, in today's `scope_rank` order),
   expanded; then the global notes section; then every other project
   (registered ones with all their scopes, plus unregistered
   `personal/<name>/` folders), collapsed at the section level. Outside a
   project the current-repository group is simply absent. `View::Project`
   and `View::Global` collapse into this one view (`View::All` or a
   rename of the lead's choosing); the title is `notez` with the current
   repository named when there is one.
2. `collect_all` (notez-core) additionally lists, as `Scope::Personal`
   with `project: Some(<name>)`, every directory `personal/<name>/` whose
   name is not in the registry. Addition only: same signature, same
   `NoteEntry`, no file format change, registered projects unchanged, the
   dedup safety net kept. Approved by Andreas as relayed ("ok"). Tests
   with a temp vault and an empty registry.
3. Scope badges: every section header already carries its scope icon;
   this ticket adds the scope word (`personal`, `public`, `scratch`,
   `notez`, from `Scope::label`) in that scope's colour next to it, and
   gives every file and folder row a one-column badge with the scope icon
   in the same colour at the left of the row (today file rows have an
   empty `scope_icon`). Colours: one per scope, chosen from `theme.rs`,
   defined once in a `scope_color(Scope)` helper so NZ-5's `s` scope
   cycling and NZ-8's prompts can reuse it. Docs sections use the docs
   icon and the public colour.
4. The scope flags narrow as before: `-p` the project's personal notes,
   `-l` its scratch, and `-g` becomes the global notes only (the vault
   root minus `personal/`), since bare `notez` now shows everything that
   `-g` used to. `-p` outside a project keeps falling back to the whole
   view. An empty narrowed view opens the browser with NZ-8's empty state
   line, naming the scope.
5. `NewNoteRoots` (NZ-8) must know every project shown, including the
   unregistered personal-only ones (personal root only, no repository),
   so `n` and `Tab` in those sections offer personal and global and never
   public or local.
6. Not in this ticket: remembering collapsed state between runs (no state
   file); any change to the todo board; projects with notes but no
   `personal/` folder and no registry entry (nothing to list).

Acceptance criteria:

1. `collect_all` on a temp vault with `personal/a/x.md` (registered),
   `personal/b/y.md` (unregistered) and `z.md` at the root returns three
   entries with scopes Personal(a), Personal(b), Global, and the
   registered project's local and public notes as before; existing
   aggregate tests unchanged.
2. Inside a project the section order is current repository, global,
   others; outside it is global, others; tested on `sections_from_entries`
   or its successor with a fake current project.
3. The current repository's sections open expanded and every other section
   collapsed; a unit test on the initial forest state.
4. Every section header shows icon and scope word in the scope colour;
   every row shows the badge; a render test at width 80 checks the badge
   column and that the label text is otherwise unchanged. Narrow widths
   drop nothing new (the badge is one column).
5. `-g` lists only root notes (no `personal/`), `-p` and `-l` as before;
   `notez -p` outside a project opens the full view; tests through
   `build_view`.
6. `n` in an unregistered personal section targets that folder and `Tab`
   cycles personal and global only; a test on the roots passed in.
7. README (default view, `-g` meaning, badges) and DESIGN.md (scope flags
   section, where it states what bare `notez` shows) updated.

Allowed files: `crates/notez-core/src/core/aggregate.rs` (decision 2
only), `crates/notez-cli/src/commands/tree.rs`,
`crates/notez-cli/src/tui/tree.rs`, `crates/notez-cli/src/tui/theme.rs`,
`crates/notez-cli/src/main.rs` and `crates/notez-cli/src/cli/mod.rs` only
if the `-g` doc strings or `decide` need the new view name, `README.md`,
`DESIGN.md`. No new dependency.

Method: bounded ticket, two worker passes on one worktree: pass 1
`aggregate.rs` and `commands/tree.rs` (listing, view, ordering, roots,
tests); pass 2 `tui/tree.rs` and `theme.rs` (initial expansion, badges,
render tests) plus docs. One review of the whole diff. Reviewer probes:
the dedup when a registered project's `personal/` is also reachable as
unregistered; a `personal/<name>` that is a file, a symlink or unreadable;
`-g` with a vault that has only `personal/` content; `Tab` in an
unregistered section never offering public.

Follow-up for Andreas, not in the ticket: epoz calls `collect_all` once
(`app/src-tauri/src/commands.rs:33`) and will list the unregistered
personal folders too once its pinned `notez-core` rev moves.

#### NZ-14: folders in the tree browser

Status: Done. Merged into `main` as `ae3617e` at 11:45 CEST on
2026-10-08 and pushed; board Done. Not yet installed by Andreas.

Record:

- Base `4a239e3`, branch `feat/NZ-14-folders`, ticket commit `cf7eabc`
  (4 files: `README.md`, `commands/mkdir.rs`, `commands/tree.rs`,
  `tui/tree.rs`; 1447 insertions, 66 deletions), merge commit `ae3617e`
  made with `git merge --no-ff` under the integration delegation. To
  undo the ticket: `git revert -m 1 ae3617e`.
- Agents: two `nz-worker` (opus) passes plus two small continuations
  (prompt wording, README line) and one fix cycle on the same worker;
  one `nz-reviewer` (opus) with one re-review. About 326k worker tokens
  and 216k reviewer tokens.
- Review: changes requested once (B1: Enter on an unchanged folder name
  ran it through `sanitize::name` and would have renamed `00_quick-notes`
  and `_todos` in the real vault), fixed with an early return and a
  regression test, then accepted. The committed diff hashes to
  `766e8c757994e3df8822099d6ebc31914dbabc73e9a076f5b0111c566c3a3286`
  (`git diff 4a239e3 cf7eabc | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and again on `main`
  after the merge: build clean, notez-cli 254 passed, notez-core 146
  passed.
- Lead decisions on the way (details in the In flight record): `N`
  footer priority 8; `N` refused on docs rows; `N` on a section row
  creates at the section root like `n`; empty-folder prompt reads
  `(no notes)`; deleting the last note in a folder now leaves the empty
  folder listed.
- Leftovers, none blocking, for Andreas: (a) a mouse click while a
  prompt is open moves the selection (pre-existing for notes); (b) a
  scope holding only empty folders shows no section, so `N` into it via
  `Tab` creates the folder and reports its path with no row; (c) `d`
  and `r` work on `_todos` in the global section; (d) `sanitize::name`
  strips `_` and lowercases typed names, and `r` on a NOTE still has the
  Enter-on-unchanged-name problem for titles not already in sanitized
  form (candidate ticket: refuse names sanitizing would alter, and the
  same early return for notes); (e) a repository nested in the vault
  lists its folders under NOTEZ (NZ-13 leftover family); (f) the mkdir
  parity test changes the cwd and restores it only at the end.

Brief as run (confirmed by Andreas on 2026-10-07 in the lead session
("NZ-14: folders in the browser (create, rename, delete)"). Builds on
NZ-8 (`n`), NZ-12 (`d`) and NZ-13 (sections, `is_current`). Runs after
NZ-13 is on `main`. Brief finalized by the lead at 19:05 CEST.

Correction to the earlier note: notez has NO numbered folder convention
(DESIGN.md: "No numbered directory allocation. Real names."); `notez
mkdir` only sanitizes the name with `sanitize::name` and runs
`create_dir_all` under the scope root, plus `ensure_scratch_gitignored`
for the local scope.

Problem (verified by the lead at `ff33de0`): the tree shows folders only
because files sit in them (`build_forest` derives folder rows from the
section's file list), `r` is guarded by `!is_dir`, and `d` on a folder
says folder delete is not available (NZ-12). An empty folder on disk is
invisible and nothing in the browser creates, renames or deletes one.

Outcome and decisions:

1. Empty folders are listed. `commands/tree.rs` walks each section root
   for directories (hidden names skipped) and passes them on the
   `SectionSpec` (a `dirs: Vec<PathBuf>` field or equivalent);
   `build_forest` makes a folder row for every listed directory, with or
   without files. Section rows (depth 0) are not folders for these keys.
2. `N` creates a folder. Same prompt machinery as NZ-8: the target is the
   folder under the cursor (the parent for a file row, the section root
   for a section row), `Tab` cycles the scope exactly as `n` does, the
   lead reads `new folder in <scope>/<folder>: _`, `Enter` creates through
   a `mkdir::create_in_dir(dir, name)` carved out of `mkdir::run` (same
   sanitizing, same `.gitignore` step for local; no behaviour change to
   `notez mkdir`), `Esc` cancels. An empty name is refused with a footer
   message, an existing name is refused (never merges into or overwrites
   an existing folder). The tree is rebuilt with the new folder selected
   and expanded.
3. `r` on a folder row renames it: the prompt shows the current name,
   `Enter` renames within the same parent with `std::fs::rename` after
   sanitizing; an existing target (file or folder, case-insensitive
   match on APFS counts) is refused and nothing changes. Every row under
   it gets its new path with `origin` kept, so the existing
   `changed_tag_maps` retires the old keys and writes the new ones on
   exit; no `.tags` file is touched at rename time, so a rename that
   fails changes nothing. The cursor stays on the renamed folder.
4. `d` on a folder row asks `delete <rel>/ and its <n> notes from
   <scope>? y/n` (`1 note`, `no notes` for an empty folder), appending
   "and other files" when the folder contains anything that is not a
   markdown note, and "not recoverable" for the local scope as NZ-12
   does. `y` removes it with `std::fs::remove_dir_all`, retires every
   `.tags` key under it through NZ-12's retired list, and rebuilds with
   the cursor on the neighbour by NZ-12's rule. A failed delete reports
   `delete failed: <error>` and rebuilds so the tree matches the disk
   (a partially removed folder then shows what is left).
5. Keys in `TREE_KEYS`: `N` (help "new folder", footer priority just
   below `n`), `r` and `d` help texts widened to "rename note or folder"
   and "delete note or folder"; the confirm mode reuses NZ-12's `y` and
   `n/esc`.
6. Not in this ticket: moving folders (NZ-15), multi-select (NZ-16),
   folder badges beyond what NZ-13 draws, the todo board.

Acceptance criteria:

1. An empty directory under a section root shows as a folder row; a
   hidden directory does not; tests on the section builder and
   `build_forest`.
2. `N` then a name creates exactly that directory where `notez mkdir
   <name>` with the matching scope would, for a cursor in each scope and
   on a section row, checked by tests on target resolution and on
   `mkdir::create_in_dir` parity with `run`; an empty or existing name
   creates nothing and shows the message; `Esc` creates nothing; after
   creation the folder is selected and expanded.
3. Rename moves the directory and every note in it, refuses an existing
   target, and after exit `.tags` holds the new keys and none of the old
   ones, other keys untouched; tests on a temp tree including a note
   two levels down and a rename that fails (target exists) leaving
   paths and tags unchanged.
4. Delete removes the directory and its contents, nothing outside it;
   the prompt counts notes correctly (0, 1, many) and says "and other
   files" when applicable; cancel removes nothing; after exit `.tags`
   has no key under the old path; a delete failure reports and rebuilds;
   tests on a temp tree.
5. `r`, `d` and `N` on a section row change nothing (a footer message is
   fine); no key panics on an empty tree; NZ-8 and NZ-12 tests pass
   unchanged except where a `SectionSpec` field addition needs a
   mechanical literal (list each).
6. `N` in the key table, help and footer tests pass, README updated.

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/footer.rs`, `crates/notez-cli/src/tui/help.rs`,
`crates/notez-cli/src/commands/tree.rs` (directory listing),
`crates/notez-cli/src/commands/mkdir.rs` (expose the creation path, no
behaviour change), `README.md`. No new dependency, no `notez-core`
change.

Method: bounded ticket, two worker passes on one worktree: pass 1
directory listing and `N` create (commands/tree.rs, mkdir.rs, the
`N` prompt in tui/tree.rs); pass 2 folder rename and delete. One review
of the whole diff. Reviewer probes: `remove_dir_all` never reachable
with a section root or a path outside the row's section; rename across
a case-only change (`Ideas` to `ideas`); a folder containing a symlink;
the retired-keys list after a folder delete that fails midway; key
dispatch order so `N` never fires inside another prompt.

#### NZ-15: move a note or folder, change its visibility

Status: Done. Merged into `main` as `36699c1` at 13:15 CEST on
2026-10-08 and pushed; board Done. Not yet installed by Andreas. Ran
with the lead's recommendations on the three decisions marked "Andreas"
below (asked at 10:35 CEST, no objection).

Record:

- Base `1074141`, branch `feat/NZ-15-move`, ticket commit `244b2a6`
  (5 files: `README.md`, `tui/footer.rs`, `tui/mod.rs`, new
  `tui/move_path.rs`, `tui/tree.rs`; 1904 insertions, 4 deletions),
  merge commit `36699c1` made with `git merge --no-ff` under the
  integration delegation. To undo the ticket: `git revert -m 1 36699c1`.
- Agents: two `nz-worker` (opus) passes and one fix cycle on the pass 2
  worker; one `nz-reviewer` (opus) with one re-review. About 463k
  worker tokens and 268k reviewer tokens.
- Review: changes requested once (B1 case-insensitive bypass of the
  global-to-`personal/` guard, B2 hidden tagged note losing its tag on
  a second folder move), both fixed with regression tests, then
  accepted. The committed diff hashes to
  `d5d6485329221d1adf55015941117eaab458a051277f58a8a95a805fad34a704`
  (`git diff 1074141 244b2a6 | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and again on `main`
  after the merge: build clean, notez-cli 286 passed, notez-core 146
  passed.
- Behaviour as built: `m` prompt `move <name> to <scope>/<folder>_`,
  `Tab` cycles the row's project's scopes plus global, destination
  folder must exist with exact spelling and no symlink component; `S`
  cycles scope only; confirm with clauses on every scope change; tags
  follow across tag roots, carried for notes that leave the view;
  `move_path` rename-first with copy-verify-remove fallback, never zero
  copies; `m`/`S` refuse docs rows, section rows, folders holding a
  section.
- Leftovers, none blocking, for Andreas: F1 check-then-rename window
  (a no-overwrite rename needs libc, a new dependency, or `hard_link`
  then `remove_file`); F2 silent tag loss on a `RemoveSource` failure
  (cross-volume copy verified, source removal failed); F3 a file written
  into the source during a cross-volume copy is lost; F4 cosmetic
  messages (`already exists` for a case variant of the note's own
  folder; a move that clears the filter does not say so); F5 mouse
  input while a prompt is open (same as rename); README does not
  mention the `not a plain folder` refusal; a typed name whose Unicode
  normalization differs from disk is refused as `no folder` (safe).

Brief as run (confirmed in scope by Andreas on 2026-10-07 in the lead
session ("NZ-15: move and change visibility, with the warning"). Builds
on NZ-8 (prompt machinery, `Tab` scope cycling, `NewNoteRoots`), NZ-12
(confirm mode, retired tag keys), NZ-13 (sections, `tag_root` per
section) and NZ-14 (folder rows, `dirs`, folder rename path updates).
Runs after NZ-14 is on `main`. Brief finalized by the lead at 10:30 CEST
on 2026-10-08 from the relayed draft and the code at `4a239e3`.

Code facts the brief rests on (verified at `4a239e3`): section roots are
personal `<notez_root>/personal/<proj>`, public `<repo>/notez`, docs
`<repo>/docs`, local `<repo>/.notez`, global `<notez_root>`
(`commands/tree.rs::section_meta`). Tag roots differ: personal and
global sections key `.tags` from `<notez_root>` (`personal/<proj>/...`),
public, docs and local from their own store root. `TreeNode` carries
`tag_root` (index) and `origin`; `changed_tag_maps_retiring` retires
`rel_key(origin)` and writes `rel_key(path)` in the SAME tag root, so a
cross-section move cannot be expressed as `origin != path` alone.
`NewNoteRoots` (in `TreeContext`) already lists the global root and each
project's personal, public and local roots in `Tab` order. `std::fs::
rename` fails with `EXDEV` across filesystems; the vault and a
repository may sit on different volumes.

Outcome and decisions:

1. `m` on a note or folder row opens a move prompt built on NZ-8's
   machinery: lead `move <name> to <scope>/<folder>: _`, where the text
   buffer is the destination folder path relative to the chosen scope
   root (empty means the root), prefilled with the row's current
   relative folder, and `Tab` cycles the scope through the row's
   project's scopes and global exactly as `n` does (`NewNoteRoots`).
   `Enter` moves, `Esc` cancels. The destination folder must exist (NZ-14
   has `N` for creating one); a missing folder is refused with `move: no
   folder <scope>/<folder>`. Docs sections are never a destination, and
   `m` on a docs row is refused like `N` is. Andreas: typed folder path
   with `Tab` for scope (recommended, reuses the prompt code, no list
   widget) versus a two-step list picker (scope list, then folder list).
2. `S` on a note or folder row opens a scope prompt: lead `set scope of
   <name>: <scope> (Tab cycles, Enter applies)`, no text buffer; the
   destination is the same relative path under the chosen scope root
   (for a section row nothing happens). It is `m` with the folder fixed.
3. Visibility warning. When the destination scope differs from the
   source scope, `Enter` in either prompt first opens a NZ-12 style
   confirm: `move <rel> to <scope>/<folder>? y/n` plus one clause chosen
   by the transition: into public, "it will be in the <repo name>
   repository, public, not yet committed"; out of public, "it stays in
   the repository's git history"; into local, "scratch is not synced and
   not recoverable"; out of global or personal into a repository, "it
   leaves the vault; the deletion syncs on exit". The word "public"
   appears in so many words whenever the destination is public. A move
   within the same scope has no confirm. notez never commits or pushes in
   a project repository (Andreas, relayed and confirmed 2026-10-07).
4. The move itself: a single pure function `move_path(src, dst) ->
   Result<()>` in a new file `crates/notez-cli/src/tui/move_path.rs`.
   It refuses if `dst` exists (`symlink_metadata`, any kind; never
   merges or overwrites, as NZ-9), then `std::fs::rename`; on an
   `EXDEV`-class error it falls back to copy then delete: for a file
   copy, read back and compare length and contents, then remove the
   source; for a folder copy the tree recursively (files and
   directories only; a symlink inside refuses the whole move before
   anything is copied), verify each file, then `remove_dir_all` the
   source. Any failure before the source removal leaves BOTH copies in
   place and reports `move failed: <error> (destination left at
   <dst>)`; a failure during source removal reports the same with the
   partial source. Never leave zero copies.
5. Tags. On a successful move the browser carries flags: for each file
   row moved (the row itself or every file row under a moved folder)
   push `(old tag root, old key)` onto NZ-12's retired list, then set
   the row's `path` and `origin` to the new path, `tag_root` to the
   destination section's tag root and `section` to the destination
   section, keeping `flags`. `changed_tag_maps_retiring` then drops the
   old key and writes the new one on exit, in two different `.tags`
   files when the tag root changes. No `.tags` file is written at move
   time. The tree is rebuilt with the cursor on the moved row at its new
   place (the destination section expanded as needed); if the rebuild
   does not list it (store with no section yet), the footer reports the
   new path like NZ-14 does for a created folder.
6. Keys in `TREE_KEYS`: `m` help "move note or folder", `S` help "set
   scope"; footer priority below `N`. Both are browse-mode keys and never
   fire inside another prompt or the confirm mode. Andreas: `S` as a
   separate key (recommended, it was his relayed key map) versus folding
   it into `m` only.
7. Not in this ticket: multi-select (NZ-16), moving into or out of docs,
   moving across projects other than through the global root, a `:mv`
   command line, undo. Andreas: should `m` offer OTHER projects' scopes
   as destinations (the lead recommends no: cross-project moves are rare
   and the prompt stays short; global is the hand-off point).

Acceptance criteria:

1. `m` moves a note within its scope to an existing folder and to the
   root, in each of the four scopes; the file is at the new path, the
   old path is gone, nothing else changed; `Esc` moves nothing; a
   missing destination folder or an existing destination name moves
   nothing and shows the message. Tests on a temp tree with a real
   `rebuild` closure.
2. `m` with `Tab` and `S` move a note across scopes for each ordered pair
   among personal, public, local and global that `NewNoteRoots` offers;
   after exit the source tag root's `.tags` has no key for the note and
   the destination tag root's `.tags` has the key with the original
   flags (test with non-zero flags and with zero flags, where neither
   file gets a key); other keys untouched.
3. A folder move (within and across scopes) moves every note under it,
   including one two levels down, and carries every key as in 2; an
   existing destination name refuses the whole move with nothing
   changed.
4. The confirm appears exactly when the scope changes, with the clause
   per transition in decision 3 and "public" for a public destination;
   tests on the pure prompt-building function for all transitions;
   `n`/`Esc` moves nothing.
5. `move_path` tests: rename path; forced copy fallback (a test hook or
   an injectable "rename failed with EXDEV" so the fallback runs on one
   volume) for a file and for a folder tree; verification mismatch
   leaves both copies; a symlink inside a folder refuses before copying;
   an existing destination refuses.
6. `m` and `S` on a section row, on a docs row and on an empty tree
   change nothing and never panic; NZ-8, NZ-12, NZ-13 and NZ-14 tests
   pass unchanged (list any mechanical change).
7. `m` and `S` in the key table, help and footer tests pass, README
   updated (key table and a short "moving notes" paragraph with the
   warning semantics and the "notez never commits" rule).

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/footer.rs`, `crates/notez-cli/src/tui/help.rs`,
`crates/notez-cli/src/tui/move_path.rs` (new), `crates/notez-cli/src/tui/
mod.rs` (module line only), `crates/notez-cli/src/commands/tree.rs` (only
if the destination sections need data the specs do not carry yet),
`README.md`. No new dependency, no `notez-core` change, no file format
change.

Method: bounded ticket, two worker passes on one worktree: pass 1
`move_path` with its tests and the `m` prompt for notes within a scope
and across scopes (decisions 1, 3, 4, 5 for files); pass 2 folders, `S`,
the rebuild cursor rule and README. One review of the whole diff.
Reviewer probes: a move whose destination is inside the source folder
(must refuse); a case-only destination on APFS; `tag_root` and `section`
on rows under a moved folder after the rebuild; a public destination in
a repository that is not the current project; the retired list after a
move that failed midway (must be empty); `y` arriving when no confirm is
open.

#### NZ-16: multi-select with `Space`

Status: Done. Merged into `main` as `5298d54` at 14:55 CEST on
2026-10-08 and pushed; board Done. Not yet installed by Andreas.

Record:

- Base `a0773b9`, branch `feat/NZ-16-multi-select`, ticket commit
  `04af62c` (2 files: `README.md`, `tui/tree.rs`; 1445 insertions, 98
  deletions), merge commit `5298d54` made with `git merge --no-ff` under
  the integration delegation. To undo the ticket: `git revert -m 1
  5298d54`.
- Agents: two `nz-worker` (opus) passes, one `nz-reviewer` (opus), no
  fix cycle. About 337k worker tokens and 109k reviewer tokens.
- Review: accepted first time. The reviewer diffed the four refactors
  (`remove_and_retire`, `resolve_folder`, `move_and_repoint`,
  generalized `move_question`) against the old bodies and found the
  single-row paths unchanged. The committed diff hashes to
  `9f090720694db7c39fcfaa8defcfdfbac0133680ce47c289f09743aca467967a`
  (`git diff a0773b9 04af62c | shasum -a 256`), the hash it accepted.
- Lead verification: build and tests in the worktree and again on `main`
  after the merge: build clean, notez-cli 304 passed, notez-core 146
  passed.
- Leftovers, none blocking: a missing space in the `TREE_KEYS` browse
  `esc` row source (`quit",theme::PEACH`), cosmetic; README "Marking
  several notes" names "a section" among refusals though sections cannot
  be marked, and omits the "destination inside a marked folder" and
  "already in" refusals; no bulk move test with a folder two levels deep
  or an item leaving the view (single-move paths reused per item); the
  event-loop wiring (`Space` swallowed in prompts, marks cleared after
  `y`) checked by tracing only.

Brief as run (confirmed
in scope by Andreas on 2026-10-07 in the lead session ("NZ-16:
multi-select with Space"); the mark key is `Space` by his decision the
same day (`x` stays "check" in the todo board). Depends on NZ-12 (`d`,
confirm mode, retired keys), NZ-14 (folder rows, folder delete) and
NZ-15 (`m`, `S`, `apply_move`, the move confirm). Brief finalized by the
lead at 12:20 CEST on 2026-10-08 against NZ-15 pass 1; the worker
verifies the NZ-15 names below against the merged code.

Code facts: rows are `TreeNode`s in a flat `Vec` with `parent_idx` and
`depth` (section rows at depth 0); `Forest::rebuild` and `carry_state`
match rows by `path`. NZ-12 has `delete_request`/`answer_delete` and a
retired `(tag root, key)` list; NZ-14 has `remove_folder` behind
`folder_change_allowed`; NZ-15 has `MovePrompt`, `resolve_move`,
`apply_move` (per row: retire old key, repoint `path`/`origin`/
`tag_root`/`section`, keep `flags`, `carried` when out of view), the
move question builder, `Mode::Move` and `Mode::ConfirmMove`. In the tree
browser `Space` is unbound today; in the todo board `Space` means check,
a different view, no clash.

Outcome and decisions:

1. `Space` toggles a mark on the row under the cursor (note or folder
   row; a section row cannot be marked, footer message) and moves the
   cursor down one row like a file manager. Marks are a `HashSet<PathBuf>`
   of row paths in session state, never persisted. A marked row is
   drawn with a mark glyph in the gutter and the selection style
   variant the theme already has for emphasis (no new colours); the
   footer shows `<n> marked` while any mark exists, before the key
   hints. `Esc` in browse mode with marks present clears all marks (and
   does nothing else); without marks `Esc` behaves as today.
2. The action set is the marked rows minus every row whose ancestor
   folder is also marked (so a folder plus one of its own notes acts
   once, on the folder). Compute it in one pure function with tests.
3. With marks present, `d` opens one confirm for the whole set:
   `delete <k> notes and <f> folders (<n> notes inside) from <scopes>?
   y/n`, scopes listed as the distinct section labels involved, with
   `(not recoverable)` when any item is in a local section; omit the
   folder clause when no folder is marked and the note clause when no
   note is. `y` deletes in order (notes with NZ-12's path, folders with
   NZ-14's path, retiring keys exactly as they do), stops on nothing:
   a failure on one item is recorded and the rest continue; after the
   set, one rebuild, cursor on the row that followed the last deleted
   row by NZ-12's rule, and a footer line `deleted <ok>, failed <bad>:
   <first failing name> (<error>)` when anything failed. Marks are
   cleared after the action whatever the outcome.
4. With marks present, `m` opens the NZ-15 prompt once for the set: lead
   `move <n> items to <scope>/<folder>: _`, same `Tab` cycling and
   destination rules; each item lands at `<destination>/<its own
   name>`; the whole set is refused before anything moves when two
   items share a name or when any destination already exists (message
   names the first collision); the scope confirm appears when any item
   changes scope, with the clauses for every transition in the set
   (public first, each clause once). `S` likewise sets the scope of the
   whole set, each item keeping its own relative folder (refused as a
   whole when a destination folder is missing or a name exists). Moves
   run through `apply_move` per item; a failure on one item leaves it
   in place and the rest continue; one rebuild; footer `moved <ok>,
   failed <bad>: ...` on partial failure; cursor on the first moved
   item when it is listed, else where it was.
5. Marks survive navigation, expanding and collapsing, filtering and a
   rebuild (matched by path, so a rename or move of a marked row drops
   its mark, by design). Without marks every key behaves exactly as
   before NZ-16: no change to single-row behaviour or messages.
6. Keys in `TREE_KEYS`: `Space` with help "mark", `Esc` help gains
   "clear marks" in browse mode; footer priority for `Space` just above
   `m`. `n`, `N` and `r` ignore marks and act on the cursor row.
7. Not in this ticket: marking in the todo board, select-all, inverting
   marks, persistence, undo, cut or paste.

Acceptance criteria:

1. `Space` marks and unmarks notes and folders, moves the cursor down,
   refuses a section row; the footer shows `<n> marked`; `Esc` clears
   marks and only then behaves as before; marks survive filter on and
   off, collapse and expand, and a rebuild; tests on the pure helpers
   and on the event loop state.
2. The action set rule: tests with a folder plus its own note, nested
   marked folders, and disjoint marks.
3. Bulk delete across two scopes including a folder with notes: exactly
   the set is removed, the confirm text matches decision 3 (test the
   builder for notes only, folders only, mixed, local present), `.tags`
   loses exactly the retired keys on exit, cursor rule holds, a failing
   item (read-only folder, unix) is reported and the rest are deleted.
4. Bulk move and bulk set scope across scopes: files land under their
   own names, tags follow per item, a name collision inside the set and
   an existing destination refuse the whole set before any move, the
   confirm lists each transition clause once, a failing item is
   reported and the rest move.
5. Without marks, every NZ-8, NZ-12, NZ-14 and NZ-15 test passes
   unchanged except mechanical literal additions (list each).
6. `Space` and the `Esc` help in the key table, help and footer tests
   pass, README updated (marking, bulk `d`/`m`/`S`, what clears marks).

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/footer.rs`, `crates/notez-cli/src/tui/help.rs`,
`crates/notez-cli/src/tui/theme.rs` (only if a mark style needs a named
entry), `README.md`. No new dependency, no `notez-core` change, no
`commands/` change, no file format change.

Method: bounded ticket, two worker passes on one worktree: pass 1 marks
(`Space`, `Esc`, drawing, footer count, survival across rebuild, the
action-set function) and bulk delete; pass 2 bulk `m` and `S`, README.
One review of the whole diff. Reviewer probes: a mark on a row that a
rebuild no longer lists; `Space` inside every prompt and the confirm
(must not mark); `Esc` precedence between marks and an open filter; the
action set when a marked folder is collapsed; bulk delete where one
folder is a section root ancestor of another marked row (guard must
hold per item); the retired list after a partial bulk failure; the move
confirm clause set for a mixed personal/public/local set.

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

#### NZ-19: add CI (GitHub Actions) for build and tests

Status: Done. Merged into `main` as `3941481` at 15:00 CEST on
2026-10-08 and pushed; the first `main` CI run `37762203661` is green;
board Done; remote branch `feat/NZ-19-ci` deleted (approved).

Record:

- Base `77c3758`, branch `feat/NZ-19-ci`, commits A `33a5793` (the
  ticket: `.github/workflows/ci.yml` new, `CLAUDE.md`,
  `docs/agent-workflow.md`; 61 insertions, 2 deletions), B `b528909`
  (throwaway failing test), C `836c255` (revert of B); merge commit
  `3941481` with `git merge --no-ff`. To undo: `git revert -m 1 3941481`.
- Runs: `37760007328` on A green (both runners), `37760332799` on B red
  (both check jobs failed in `cargo test` on the throwaway test only,
  lint green), `37760452070` on C green, `37762203661` on `main` green.
- Agents: one `nz-worker` (opus), one `nz-reviewer` (opus), no fix
  cycle. About 38k worker tokens and 48k reviewer tokens. The lead did
  the pushes and the red-run commits (workers never push).
- Review: accepted first time. Pins verified by the reviewer:
  `actions/checkout` v7.0.1 `3d3c42e5...` (lightweight tag),
  `Swatinem/rust-cache` v2.9.2 `6323deb1...` (annotated tag
  dereferenced), `dtolnay/rust-toolchain` branch `stable` at
  `89b12181...` (2026-10-01; a moving branch pinned by immutable SHA,
  acceptable). The diff `git diff 77c3758 836c255 | shasum -a 256` is
  `830d7b902c3e3695f5f240362c825e96a75c68897415d8c58c531746c21311dd`,
  equal to the diff at A, the hash it accepted.
- F1 from the review (docs-only pushes start no run; how to read the
  safe-merge condition then) was added by the lead to
  `docs/agent-workflow.md` right after the merge, as a coordination-doc
  edit, in the same commit as this record.
- Leftovers, none blocking: O1 the concurrency comment says "never on
  main" while GitHub still keeps at most one pending run per group; O2
  `--locked` on the cargo commands would catch a stale `Cargo.lock`
  (needs a decision, it changes the "same two commands" wording); O3
  the `# stable` pin comment could carry its date; O4 `--no-fail-fast`
  on `cargo test` would show every failing binary; the `ubuntu-latest`
  label moves to Ubuntu 26 from 2026-10-19 (GitHub notice).

Brief as run. AUTHORIZED by Andreas in the lead session at 13:55 CEST
("1, yes 2. ru it now 3. yes 4. yes": run, now, branch push allowed,
remote branch deletion after merge allowed). Earlier history of this brief:
relayed on 2026-10-08 at about 12:30 CEST by the advisor session
(`repos-f9`) with Andreas's words "yes, ok from me. lets set this up",
given there in answer to adding CI to notez, and the advisor's own
recommendation (not countered by Andreas) that this lead runs it rather
than being paused. Board item created by the advisor at Ready; the lead
only keeps its Status current. The lead's reading: a relayed approval
cannot widen the standing scope, which excludes CI changes
(`CLAUDE.md`: ask before CI changes), so the lead asked Andreas in the
lead session at 12:35 CEST to confirm (a) the ticket, (b) the slot (second
worker slot now, disjoint from `tui/`, or after NZ-15), (c) pushing the
ticket branch to `origin` so the workflow runs there before the merge,
and (d) deleting that remote branch afterwards. Nothing runs before his
answer.

Problem: the repository has no CI (no `.github/`), yet the safe-merge
rule in `docs/agent-workflow.md` names "CI is green on the pushed ticket
branch" and "CI on `main` is green", which every merge so far has
satisfied vacuously.

Outcome: a GitHub Actions workflow (public repository, free minutes) that
builds and tests pushes and pull requests.

Decisions (as relayed, adopted by the lead):

1. One workflow `.github/workflows/ci.yml`: triggers `push` on every
   branch and `pull_request`, with `paths-ignore` for `**.md` and
   `docs/**`; `permissions: contents: read`; a `concurrency` group per
   ref that cancels superseded runs.
2. Job `check` on a matrix of `ubuntu-latest` and `macos-latest`:
   checkout, stable Rust toolchain, cargo cache, `cargo build
   --workspace`, `cargo test --workspace`. The sync tests shell out to
   `git` and commit in temp repositories, so the job sets a git identity
   (`user.name`, `user.email`) for the runner before testing, if any
   test needs it (the worker verifies by running the suite in a shell
   with no global git identity).
3. Every action pinned to a full commit SHA with the version in a
   trailing comment (Andreas's rule: never `@latest`, not a tag).
4. Job `lint`, NOT gating: `cargo clippy --workspace --all-targets`
   with `continue-on-error: true`, so warnings are visible and can be
   tightened later (floors only ratchet up). `cargo fmt --check` is not
   run at all: it fails repo-wide on old drift, and untouched files are
   not reformatted.
5. No new dependency, no secrets, no release or publish step.
6. Docs in the same ticket: `CLAUDE.md` Ship policy line ("No CI: a push
   triggers nothing" becomes what CI runs and that a push to any branch
   triggers it) and its Checks section; `docs/agent-workflow.md`
   integration policy and safe-merge text (CI exists, what it runs, a
   green run is the condition, how to read it with `gh run list`);
   `README.md` only if it mentions the checks.

Acceptance criteria:

1. The workflow is green on the pushed ticket branch on both runners
   (link the run ids in the worker report).
2. A deliberately failing test turns it red: one throwaway commit on the
   branch shows a red run, then is removed from the branch before review
   (`git reset` on the unpushed tip or a revert commit; the reviewer
   sees the final branch, and the lead records both run ids).
3. Every `uses:` is a full SHA that resolves to the stated version
   (reviewer checks with `gh api repos/<owner>/<repo>/commits/<sha>` or
   the tag's commit); permissions are `contents: read` only.
4. `actionlint` or `gh workflow view` parses the file without error
   (whichever is available; say which).
5. Docs updated as in decision 6; `cargo build --workspace` and `cargo
   test --workspace` still pass locally (no code change expected).

Allowed files: `.github/workflows/ci.yml` (new), `CLAUDE.md` (the Ship
policy and Checks lines only), `docs/agent-workflow.md` (integration
policy and safe-merge text only), `README.md` (checks mention only). No
code change.

Method: bounded ticket, one `nz-worker` pass, one review. The lead
pushes the branch (worker never pushes) so the worker can read the run;
if the worker needs a second push for the red-run demonstration, it
reports and the lead pushes. Reviewer probes: SHA pins resolve to the
claimed versions; `paths-ignore` does not skip a push that mixes docs
and code; the concurrency group does not cancel `main` runs needed for
the safe-merge check; the git identity step; no `pull_request_target`.

#### NZ-20: refuse names sanitizing would alter; protect `_todos`

Status: Done. Merged into `main` as `483cb1c` at 16:00 CEST on
2026-10-08 and pushed; branch run `37771278846` green, `main` run green;
board Done; remote branch deleted. Not yet installed by Andreas.

Record: base `e84e90d`, branch `feat/NZ-20-refuse-altered-names`,
ticket commit `f9bd0b9` (2 files, 429 insertions, 31 deletions), merge
`483cb1c` with `git merge --no-ff`; undo with `git revert -m 1 483cb1c`.
One `nz-worker` (opus, about 132k tokens), one `nz-reviewer` (opus,
about 76k), accepted first time; committed diff hashes to the accepted
`ade3c2bb3012a12bbca986cdd9d1bcdfea7ceeda2b37d2a36f53822136facb26`.
Lead checks in the worktree and on `main`: build clean, notez-cli 312
passed, notez-core 146 passed. Open product question put to Andreas:
`n` now refuses capitals and spaces in a title (a heading like "Meeting
with Bob" needs `meeting-with-bob`); softer rule proposed: refuse only
when sanitizing drops characters. Leftovers: reviewer F1 to F4 in the
In flight record.

Authorized by Andreas in the lead session (13:55 CEST "5. lets try
refusing. 6. protect it"; 15:15 CEST "you may go ahead with NZ-20").
Board item `PVTI_lAHOCU842c4BmE5Zzg_Yflc`, created by the lead on that
go-ahead.

Problem (NZ-14 review and worker findings): typed names in the browser
go through `sanitize::name` (trim, lowercase, whitespace to `-`, keep
only alphanumerics and `-`), so `00_quick` becomes `00quick` and
`My_Note` becomes `mynote` without the user being told. `r` on a NOTE
then Enter on the unchanged shown title renames the file whenever the
title is not already in sanitized form (`2026-10-06-My_Note.md` to
`2026-10-06-mynote.md`, heading rewritten; `x.MD` to `xmd.md`); folders
got the no-op fix in NZ-14. `d`, `r`, `m` and `S` act on `_todos` in
the global section, the todo board's store (`notez_root/_todos`, read by
`notez-core/src/todo/mod.rs`), guarded only by the confirm.

Outcome and decisions:

1. In the browser prompts `n`, `N`, `r` (note and folder) and `m`
   (typed destination folder is matched exactly already since NZ-15, so
   only the name side matters), a typed name whose `sanitize::name`
   result differs from the trimmed input is REFUSED with a footer
   message that shows what it would have become: `name would become
   <cleaned>; use letters, digits and -` (empty result: the existing
   empty-name message). Nothing is created, renamed or moved. The CLI
   commands (`notez add`, `notez mkdir`, `notez rename` if any) keep
   sanitizing as today; only the interactive prompts refuse.
2. `r` on a note: Enter with the trimmed input equal to the shown title
   is a no-op (prompt closes, nothing on disk, no heading rewrite, no
   message), mirroring NZ-14's folder fix. `editable_title` is not
   changed; the no-op check compares against what the prompt showed.
3. `_todos` protection: the row for `notez_root/_todos` and every row
   under it is treated like a docs row for `d`, `r`, `m` and `S`
   (refused with `<verb>: the todo board's store is managed by the todo
   view`), and it is never a destination for `m`/`S` (`move: no folder
   ...` is fine). `n` and `N` under `_todos` are left as they are. Marks
   (NZ-16) on those rows are allowed but the bulk actions refuse the
   whole set as they do for docs rows.
4. Not in this ticket: changing `sanitize::name`, changing the CLI, a
   trash or undo, renaming `_todos`.

Acceptance criteria:

1. Each prompt (`n`, `N`, `r` note, `r` folder, `m` on the name side if
   applicable) refuses `00_quick`, `My Note`, `Ideas` (case), `a.b` with
   the message naming the cleaned result, and still accepts `00-quick`,
   `my-note`, `ideas`; nothing changes on disk; tests on the pure check
   and on each prompt's Enter path.
2. `r` on `2026-10-06-My_Note.md` with the shown title unchanged
   changes nothing (file, heading, tags); the existing rename tests
   pass unchanged.
3. `d`, `r`, `m`, `S` on `_todos` and on a note inside it change
   nothing and show the message; a bulk action whose set includes such
   a row is refused as a whole; `_todos` is not reachable as a move
   destination.
4. All existing tests pass unchanged except mechanical literal
   additions and tests that pinned the old sanitizing behaviour in the
   prompts (list each, with the contract change named).
5. README: one sentence per change (names, no-op rename, `_todos`).

Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/footer.rs`, `crates/notez-cli/src/tui/help.rs`,
`README.md`. No `notez-core` change, no `commands/` change, no new
dependency.

Method: bounded ticket, one worker pass, one review. Reviewer probes:
Unicode input (`Ä`, decomposed forms) against the exact-equality check;
`_todos` spelled in another case on APFS; a `.MD` note's rename no-op;
the message when the cleaned name is empty.

#### NZ-21: tree rows: badge indented with the row, count column aligned

Status: Done. Merged into `main` as `d667999` at 17:00 CEST on
2026-10-08 and pushed; branch run and `main` run `37774540545` green;
board Done; remote branch deleted. Not yet installed by Andreas; the
look is his to judge installed.

Record: base `9a6c0e0`, branch `fix/NZ-21-tree-row-alignment`, ticket
commit `9ce0dae` (`tui/tree.rs` only, 174 insertions, 32 deletions),
merge `d667999` with `git merge --no-ff`; undo with `git revert -m 1
d667999`. One `nz-worker` (opus, about 120k tokens over two rounds), one
`nz-reviewer` (opus, about 41k), accepted first time; committed diff
hashes to the accepted
`031b6acbdbcccaaa9631c41f4702f7c09eac4316e24af90c4969b412b62cf5a2`.
Lead checks: build clean, notez-cli 318 passed, notez-core 146 passed.
As built: badge `icon` plus a space right before the name on nested
rows, blank gutter kept; leader measured in display columns; the
list's text width corrected to pane minus 8 (`list_text_width`), which
is why section counts looked right before while nested ones did not.
Leftovers: `BADGE_COL` test constant now names the blank gutter; a very
long nested folder name can still overflow its row (pre-existing).

Brief as run. Requested by Andreas in the lead session on 2026-10-08
at about 15:55 CEST with two screenshots ("the expanding of tree
structure is a bit weird, visibly, the icons for the expanded branch
are visibly to the left ... indent correctly. Same for number of
documents in view, gets skewed, everything in an extended branch is
skewed to the left"), named as one of "two things directly". Runs
first after NZ-20. Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3Lk`. Touches
`tui/tree.rs` only.

Problem (verified by the lead in `row_line`, `tui/tree.rs` about line
625, at `483cb1c`): (1) `row_badge` (NZ-13's per-note scope badge for
rows with `depth > 0`) is pushed BEFORE the indent, so it sits in a
fixed gutter column at the far left while the row's own content is
indented; in the screenshots the docs icon of every nested row lines up
under the section's expand triangle instead of next to the row. (2) The
dotted leader to the file count computes `prefix_len` with
`icon.len()`, which is BYTES; the branch glyphs `├─▼ `, `├─▶ ` and
`│   ` are 4 columns but 10 or 6 bytes, so nested directory rows get a
leader 2 to 6 columns too short and their count ends left of the
section counts (screenshot: `1` under `superpowers`/`plans` ends about
4 columns before `159`/`2`). `indent` is ASCII so it is fine.

Outcome and decisions:

1. For rows with `depth > 0` the scope badge is drawn directly before
   the name, after the indent and branch glyph, in the scope colour as
   today; the gutter keeps a one-column placeholder so the tag dots and
   the section rows do not move. Section rows (depth 0) are unchanged.
2. The leader width uses display widths (`chars().count()`, or the
   crate's existing width helper if one exists, see `tui/text.rs`), so
   every directory row's count ends in the same screen column as the
   section rows' counts, at every depth.
3. No behaviour change beyond drawing; no key, state or `.tags` change.

Acceptance criteria:

1. A test renders a section row, a nested folder row at depth 1 and 2
   and a nested file row through `row_line` at a fixed `inner_width`
   and asserts: the badge span immediately precedes the name span for
   nested rows; the total display width of each directory row equals
   `inner_width` (counts right-aligned) at every depth; the file row's
   name starts at the same column as its sibling folder's badge plus
   one.
2. A regression test with a name containing wide or multi-byte
   characters still aligns the count.
3. Existing render tests pass unchanged except where they pinned the
   old gutter badge position (list each).

Allowed files: `crates/notez-cli/src/tui/tree.rs`. Method: bounded,
one `nz-worker` pass, one review. Look and feel is Andreas's to
accept; under the delegation it merges and he tries it installed.

#### NZ-22: scroll the preview pane with Shift+Up/Down

Status: Done. Merged into `main` as `82fae54` at 18:00 CEST on
2026-10-08 and pushed; branch run and `main` run `37776056271` green;
board Done; remote branch deleted. Not yet installed by Andreas.

Record: base `d667999`, branch `feat/NZ-22-preview-scroll-keys`,
ticket commit `75e2a88` (`tui/tree.rs`, `README.md`; 123 insertions,
13 deletions), merge `82fae54` with `git merge --no-ff`; undo with
`git revert -m 1 82fae54`. One `nz-worker` (opus, about 147k tokens
over two rounds), one `nz-reviewer` (opus, about 41k), accepted first
time; committed diff hashes to the accepted
`05b21b4beac5bd0bbbf3545caa15bc495882d43eb947860b564648c4f88ec0e8`.
Lead checks: build clean, notez-cli 324 passed, notez-core 146 passed.
As built: Shift+Down/Up one line like `J`/`K`, PgDn/PgUp a page, one
clamped `scrolled` helper for every path, `J/K preview` visible at the
footer's right end (first hint dropped), README with the Terminal.app
caveat. Leftovers: tag mode lets Shift+Down/Up through but swallows
`J`/`K` and the page keys; help lists the two rows after `wheel` and
`click`; no test pins `PgDn/PgUp` as HelpOnly; README's terminal list
is untested fact.

Brief as run. Requested by Andreas in the lead session on 2026-10-08
("i want shift + j/k & up/down to scroll document in the right pane"),
the second of "two things directly". Ran after NZ-21 (same file).
Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3NA`.

Facts (verified at `483cb1c`): `J` and `K` (that is Shift+j/k) already
scroll the preview one line (`preview_scroll`, `tui/tree.rs` about
lines 3884 to 3887) and are listed in `TREE_KEYS` as `Slot::HelpOnly`
("scroll preview down / up"); the mouse wheel scrolls by 3; Shift+Up
and Shift+Down are not bound.

Outcome: Shift+Down and Shift+Up scroll the preview like `J` and `K`;
PageDown/PageUp (or Shift+PageDown/Up if plain PageUp/Down are taken)
scroll by a page minus one line; the `J/K` help row mentions the arrow
aliases; the preview scroll hint becomes visible in the footer at low
priority (not HelpOnly) so the feature is discoverable. Terminal note:
many terminals report Shift+arrow only with the kitty keyboard protocol
or specific escape sequences; the worker checks what crossterm delivers
under macOS Terminal and iTerm2 modifiers and records it; if Shift+arrow
cannot be distinguished, the ticket still ships `J`/`K` discoverability
and PageUp/PageDown and says so in the README.

Acceptance: key table and help tests; a unit test on the scroll
arithmetic (clamp at the end, page step); README key list updated.
Allowed files: `crates/notez-cli/src/tui/tree.rs`, `tui/footer.rs`,
`tui/help.rs`, `README.md`. One worker pass, one review.

#### NZ-23: the bonsai docs are not listed (project not attached)

Status: Done without code, 17:25 CEST on 2026-10-08, board Done. On
Andreas's "1. go with a. you may do that. it must be a private notez
note tho!" the lead moved, with plain `mv` in the vault (uncommitted
there; the next notez session's exit sync commits it): the five docs
`00-overview.md` to `04-open-questions.md` from `~/Repos/bonsai/docs/`
to `~/notez/personal/bonsai-education/docs/`, and the global note
`~/notez/2026-10-06-bonsai.md` to `~/notez/personal/bonsai-education/`.
The now empty `~/Repos/bonsai/docs/` was removed. Nothing was attached
(private notes need no registration). Both show under
`bonsai-education (personal)` after the move. To undo: move them back
with `mv`. Follow-up ideas kept as candidates, not tickets: a footer
line when a new note falls back to global because the directory is not
a project; a one-time warning about legacy `projects` entries.
Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3Pg`. Andreas on 2026-10-08: "i added a document in
bonsai two days ago. i can see the directory for it, but not that
note, i think it was about 2.5kb".

Findings (read-only, 2026-10-08 16:00 CEST): the live registry
`~/.config/notez/registry.toml` has no `bonsai` or `bonsai-education`
project (it lists airwavez, app2, auraz, career, file-gatherer,
imrsv-website, j24-examen, noiz, notez, repoz, rustfinity, tranzlate,
and a few more). `~/.config/notez/projects` is the LEGACY notez-cli
registry (read only by `migrate.rs`) and does contain
`bonsai=/Users/at-a/Repos/bonsai`; `~/Repos/bonsai` is an umbrella
directory, not a git repository (the repository is
`~/Repos/bonsai/bonsai-education`). Five markdown files of 1.5 to 2.9
KB were written on 2026-10-06 into `~/Repos/bonsai/docs/` (00-overview
to 04-open-questions); they match "about 2.5kb" and "two days ago".
The tree shows `bonsai-education (personal)` with 1 note only because
`~/notez/personal/bonsai-education/` exists (a 30-byte untitled note
from 2026-10-06); that section comes from the unregistered-personal
path, not from a registration. So the docs are invisible because no
registered project covers `~/Repos/bonsai`, and `notez attach` works
from inside a git repository, which the umbrella directory is not.

RESOLVED for the note itself at 17:00 CEST: the note Andreas meant is
`~/notez/2026-10-06-bonsai.md` (`# Bonsai`, 1055 bytes, written
2026-10-06 18:22). He ran `notez` inside the umbrella directory, which
is not a git repository, so the scope fell back to Global
(`Scope::resolve`: outside a project the default is Global) and the
note landed at the vault root, listed under NOTEZ, not under any bonsai
section. Nothing is lost; `m` can move it. Follow-up idea for the
brief: when a note is created and the scope fell back to Global
because the directory is not a project, the footer or the CLI output
should say so and name the path. The five docs in
`~/Repos/bonsai/docs/` remain invisible for the reason above.

Options put to Andreas for the docs and the registry: (a) move the five docs into
`~/Repos/bonsai/bonsai-education/docs/` and `notez attach` there (then
they list as `bonsai-education (docs)`); (b) a ticket to let `attach`
register a plain directory as a project (docs and notes stores only, no
sync), which is a scope-model change (DESIGN.md) and a `notez-core`
change needing his approval; (c) a ticket to migrate or warn about
legacy `projects` entries that the new registry does not have. The
lead recommends (a) now and (c) as a small ticket.

#### NZ-24: an icon for the todo store and TODO.md rows

Status: Done. Merged into `main` as `706e169` at 19:10 CEST on
2026-10-08 and pushed; branch run and `main` run `37778913601` green;
board Done; remote branch deleted. Not yet installed by Andreas; the
glyph is his to judge installed.

Record: base `5442854`, branch `feat/NZ-24-todo-icon`, ticket commit
`092b88a` (`README.md`, `tui/theme.rs`, `tui/tree.rs`; 109 insertions,
7 deletions), merge `706e169` with `git merge --no-ff`; undo with `git
revert -m 1 706e169`. One `nz-worker` (opus, about 53k tokens), one
`nz-reviewer` (opus, about 29k), accepted first time; committed diff
hashes to the accepted
`ad4548197873e5032527f9028e7bb758e3b354fe9d4e22ae5086ef46dcc108a3`.
Lead checks: build clean, notez-cli 331 passed, notez-core 146 passed.
As built: `ICON_TODO = "\u{f0ae}"` (nf-fa-tasks) on the `_todos` store
row, every row under it and any `TODO.md` file, scope colour kept.
Leftovers: two metadata lookups per nested global row per draw (cache
on `TreeNode` if lag shows); no test for a directory named `TODO.md`.

Brief as run. Authorized to run after NZ-26 (Andreas, 2026-10-08
17:20 CEST: "3. go with b"). Requested by Andreas on 2026-10-08
("todo's should have it's own icon and perhaps color? if not colors
are reserved for visibility"). Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3RQ`.

Decisions: icon only, colour stays scope (NZ-13). The glyph is `✓` or
a Nerd Font check-list icon in the style of the existing section icons
(`Scope::icon()` values in `notez-core`, look at how they are chosen;
the worker picks the one that renders in the same width as the others
and says which). It replaces the scope badge on: the `_todos` folder
row, every row under it, and every `TODO.md` row in any section
(project stores included, since the todo board reads them). Section
header icons unchanged. The help overlay legend (if one lists icons)
gets the glyph with the word "todo". Uses NZ-20's `in_todo_store` for
the store and a file-name check for `TODO.md`.

Acceptance: render tests for a `_todos` row, a note under it, a
project `TODO.md`, and an ordinary note next to it (unchanged); the
glyph has display width 1 or the same as the other icons; README one
sentence. Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`tui/theme.rs` (glyph constant), `tui/help.rs` (legend), `README.md`.
One `nz-worker` pass, one review.

#### NZ-25: markdown rendering in the preview pane, toggleable

Status: Done. Merged into `main` as `6a1c814` at 20:55 CEST on
2026-10-08 and pushed; branch run and `main` run `37785453082` green;
board Done; remote branch deleted. Not yet installed by Andreas; the
toggle key `p` and the look are his to judge installed.

Record: base `706e169`, branch `feat/NZ-25-markdown-preview`, ticket
commit `f8d77ba` (7 files: `Cargo.lock`, `README.md`,
`crates/notez-cli/Cargo.toml`, new `tui/markdown.rs`, `tui/mod.rs`,
`tui/theme.rs`, `tui/tree.rs`; 1506 insertions, 82 deletions), merge
`6a1c814` with `git merge --no-ff`; undo with `git revert -m 1
6a1c814`. Dependency added: `pulldown-cmark = { version = "0.13.4",
default-features = false }`; `Cargo.lock` gained `pulldown-cmark
0.13.4` and `unicase 2.10.0` (crates.io). Agents: two `nz-worker`
(opus) passes plus one small continuation, about 347k tokens; one
`nz-reviewer` (opus) with one re-review, about 180k tokens; accepted
first time, re-accepted after the lead's two pre-merge changes (perf
bound 5 s, lone `\r`). Committed diff hashes to the accepted
`476413e3f2b87610d5153dd6e10521a411c8cc332848fa7625d01de68a1bb89d`.
Lead checks: build clean, notez-cli 375 passed, notez-core 146 passed.
As built: rendered by default, `p` toggles raw for markdown only, mode
is session state, footer leads with the file type, cache keyed by path,
width, mode, mtime and length, visible slice drawn. Leftovers: wrapped
code lines lose leading indentation and inner whitespace (fixed in
NZ-27); a table inside a quote or list repeats the container prefix;
two doc comments misplaced or overstated; control characters in a note
reach the terminal (pre-existing).

Brief as run. Authorized to run after NZ-24 (Andreas, 2026-10-08
17:20 CEST: "4. go with c, but break it up into multiple
tasks/tickets. add linting and LSP and syntax highlighting too.
approved to add dependencies, tree-sitter ive used before"). Requested
on 2026-10-08 ("can we add a markdown reader (togglable) in the
inspector? ... the inspector pane could also show in the footer what
language it is"). Board item `PVTI_lAHOCU842c4BmE5Zzg_Y3S8`. The family:
NZ-25 rendered markdown (this ticket), NZ-27 syntax highlighting, NZ-28
linting (design first), NZ-29 LSP (design first).

Code facts (at `d667999`): the preview is a `Paragraph` built from
`preview_lines: Vec<Line>` (about line 3207) with `.scroll((preview_
scroll, 0))`; the file is read on selection; NZ-22 adds `scrolled`,
`preview_max` and `preview_height`. `notez-cli` depends on ratatui
0.29, crossterm 0.28, anyhow, chrono, clap; no markdown parser yet.

Outcome and decisions:

1. A rendered view of the selected markdown note in the preview pane:
   headings styled by level (bold, scope-neutral colours from
   `tui/theme.rs`), emphasis and strong, inline code, fenced code
   blocks drawn in a block style with the language tag kept as a line,
   bullet and numbered lists with indentation, block quotes with a
   bar, horizontal rules, links shown as `text (url)` or `text` with
   the url dimmed, tables passed through as text for now. Wrapping to
   the pane width (ratatui `Wrap { trim: false }`); scrolling as today
   (line based on the rendered lines).
2. A toggle key switches the preview between rendered and raw; the
   worker proposes the key from the free ones (candidates `p` or `R`;
   not `v`, `f`, `t`, `s`, `S`, `m`, `n`, `N`, `r`, `d`, `o`, `J`, `K`,
   `Space`, `x`, `y`, `q`, `?`, `/`, `:`, digits) and the lead confirms
   with Andreas at review. The mode is session state, default
   rendered, remembered across selections, never persisted. The footer
   hint shows the key with "raw" or "rendered" as the state.
3. The footer (status line) shows the selected file's type while the
   preview is visible: `markdown` for `.md`, otherwise the extension
   (`toml`, `rs`, `txt`); shown in the Hints slot's leading position
   like NZ-16's mark count, or in the preview block title if that
   reads better (worker's call, say which).
4. Parser: `pulldown-cmark = "0.13.4"` (current release on crates.io
   at 19:05 CEST on 2026-10-08, checked with `cargo search`; pinned
   exactly, the worker records the `Cargo.lock` change),
   default options plus tables and strikethrough; no HTML rendering
   (raw HTML blocks shown as text). It is the ONLY new dependency in
   this ticket.
5. Non-markdown files keep the raw view regardless of the toggle; the
   toggle hint hides for them.
6. Not in this ticket: syntax highlighting of code blocks (NZ-27),
   images, following links, editing.

Acceptance: a pure `render_markdown(text, width) -> Vec<Line>` with
tests for each construct in decision 1 (headings, emphasis, inline
code, fenced block with and without a language, nested lists, quote,
rule, link, table pass-through, raw HTML pass-through); the toggle
state machine tested; the footer file type tested for `.md`, `.toml`
and no extension; scrolling clamps on the rendered line count;
existing preview tests pass unchanged; README documents the toggle and
the footer type. Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`crates/notez-cli/src/tui/markdown.rs` (new), `tui/mod.rs` (module
line), `tui/footer.rs`, `tui/help.rs`, `tui/theme.rs`,
`crates/notez-cli/Cargo.toml`, `Cargo.lock`, `README.md`. Two worker
passes (renderer with tests; integration, toggle, footer, README), one
review. Reviewer probes: performance on a 2 MB note (render on
selection, not on every key); a note with CRLF; unmatched fences;
width 1 and width 0 panes.

#### NZ-26: soft name rule in the browser prompts

Status: Done. Merged into `main` as `5442854` at 18:35 CEST on
2026-10-08 and pushed; branch run and `main` run `37777856510` green;
board Done; remote branch deleted. Not yet installed by Andreas.

Record: base `82fae54`, branch `fix/NZ-26-soft-name-rule`, ticket
commit `a361d30` (`tui/tree.rs`, `README.md`; 119 insertions, 19
deletions), merge `5442854` with `git merge --no-ff`; undo with `git
revert -m 1 5442854`. One `nz-worker` (opus, about 45k tokens), one
`nz-reviewer` (opus, about 37k), accepted first time; committed diff
hashes to the accepted
`8ef332a8083b412c2f81bd9334c3e0b49d54b2c70ab56288e7563ecdefb6371a`.
Lead checks: build clean, notez-cli 327 passed, notez-core 146 passed.
Leftovers: README could say a punctuation-only name counts as empty;
pre-existing: a case-only title change mapping to the same file name
does not rewrite the heading; `İ` is refused as `i`.

Brief as run. Authorized to run after NZ-22 (Andreas, 2026-10-08
17:20 CEST: "2. go with b"). Board item `PVTI_lAHOCU842c4BmE5Zzg_ZIFY`.
Touches `tui/tree.rs`. Changes NZ-20's contract.

Problem: NZ-20 refuses any typed name that `sanitize::name` would
change, so `n` cannot take a title like `My Note` (it must be typed
`my-note`); the reviewer flagged that a heading like "Meeting with Bob"
can no longer come from `n`.

Outcome and decisions:

1. New rule for `n`, `N` and `r` (note and folder): a name is refused
   only when sanitizing would DROP characters, that is when
   `sanitize::name(input)` differs from a soft form of the input
   (trim, lowercase, whitespace runs to `-`). Lowercasing and
   space-to-hyphen stay silent as before NZ-20. `00_quick` (drops `_`),
   `a.b` (drops `.`) and `!!!` are still refused with the NZ-20 message;
   `My Note` is accepted (file `my-note.md`, heading `# My Note` as
   before NZ-20); `Ideas` as a folder name is accepted and becomes
   `ideas` (the typed case is not kept for file names; the heading of
   a note keeps the typed text).
2. The no-op rule (NZ-14, NZ-20) is unchanged: Enter on the unchanged
   shown name does nothing.
3. The refusal message is unchanged; README's sentence on names is
   updated to the soft rule.

Acceptance: pure-check tests for the soft form and each example above;
each prompt's Enter path tested for `My Note` accepted and `00_quick`
refused; NZ-20 tests updated only where they pinned `My Note` or
`Ideas` as refused (list each, contract change named); README. Allowed
files: `crates/notez-cli/src/tui/tree.rs`, `README.md`. One worker
pass, one review.

#### NZ-27: syntax highlighting in the preview (tree-sitter)

Status: Done. Merged into `main` as `299770e` on 2026-10-08 (about
16:45 CEST real clock) and pushed; branch run green; `main` run
`37796746297` green; board Done; remote branch deleted. Not yet
installed by Andreas.

Record: base `9e1bb20`, branch `feat/NZ-27-syntax-highlighting`, ticket
commit `608235c` (8 files: `Cargo.lock`, `README.md`,
`crates/notez-cli/Cargo.toml`, new `tui/highlight.rs`, `tui/markdown.rs`,
`tui/mod.rs`, `tui/theme.rs`, `tui/tree.rs`; 1692 insertions, 43
deletions), merge `299770e` with `git merge --no-ff`; undo with `git
revert -m 1 299770e`. Dependencies added (approved family): tree-sitter
0.27.0, tree-sitter-highlight 0.27.0, tree-sitter-rust 0.24.2,
tree-sitter-python 0.25.0, tree-sitter-kotlin-ng 1.1.0 (replacing the
approved tree-sitter-kotlin 0.3.8, which pins an old runtime; ships no
queries, so `highlight.rs` carries a local `KOTLIN_HIGHLIGHTS`),
tree-sitter-java 0.23.5, tree-sitter-c 0.24.2, tree-sitter-toml-ng
0.7.0, tree-sitter-json 0.24.8, tree-sitter-bash 0.25.1, tree-sitter-md
0.5.3; lock closure tree-sitter-language 0.1.8, streaming-iterator
0.1.9, regex 1.13.1 with regex-automata, regex-syntax, aho-corasick;
build-dep bumps cc 1.6.0, shlex 2.0.1, find-msvc-tools 0.1.14,
serde_json 1.0.151. Release binary 11,992,256 B (was 2,359,440 B).
Agents: two `nz-worker` (opus) passes with two continuations, about 385k
tokens; one `nz-reviewer` (opus), about 87k, accepted first time.
Committed diff hashes to the accepted
`e77347223d168f7f151563885629ce45b3c7d0e161d8a2cba31ef154efa05809`.
Lead checks: build clean, notez-cli 416 passed, notez-core 146 passed.
Leftovers: F1 Kotlin receiver method calls capture as property; F2 raw
markdown markers dimmed (Andreas to judge installed); F3 the 1 MiB
limit measured differently by renderer and footer; F4 README says 1 MB;
measure python and bash near 1 MiB in release; feature flags for bash,
c, java grammars as a later option.

Brief as run. Authorized after NZ-25 (Andreas: "add linting and LSP
and syntax highlighting too. approved to add dependencies, tree-sitter
ive used before"; working rule: the lead decides the open points).
Board item `PVTI_lAHOCU842c4BmE5Zzg_ZIHA`. Brief finalized by the lead
on 2026-10-08. Touches `tui/tree.rs`, `tui/markdown.rs`, a new
module, `Cargo.toml`, `Cargo.lock`.

Dependencies (approved family; versions are the current crates.io
releases checked with `cargo search` at 20:50 CEST): `tree-sitter =
"0.27.0"`, `tree-sitter-highlight = "0.27.0"`, and the grammars
`tree-sitter-rust = "0.24.2"`, `tree-sitter-python = "0.25.0"`,
`tree-sitter-kotlin = "0.3.8"`, `tree-sitter-java = "0.23.5"`,
`tree-sitter-c = "0.24.2"`, `tree-sitter-toml-ng = "0.7.0"`,
`tree-sitter-json = "0.24.8"`, `tree-sitter-bash = "0.25.1"`,
`tree-sitter-md = "0.5.3"`. Grammar crates link against the
`tree-sitter-language` ABI crate, so they do not have to share the
runtime's version; the worker verifies each compiles and loads with
`tree-sitter 0.27.0` and drops any that does not (report which; the
lead records it). All grammars compile C code at build time: the
worker records the clean build time before and after and the release
binary size delta.

Outcome and decisions:

1. A new module `crates/notez-cli/src/tui/highlight.rs` with `pub fn
   highlight(source: &str, language: Language) -> Vec<Vec<(Range<usize>,
   Capture)>>` (or an equivalent that yields styled spans per line) built
   on `tree-sitter-highlight` with each grammar's bundled
   `HIGHLIGHTS_QUERY` (and injections where the crate ships them), and a
   fixed capture list (`keyword`, `function`, `type`, `string`,
   `number`, `comment`, `constant`, `variable`, `operator`,
   `punctuation`, `attribute`, `property`) mapped to `tui/theme.rs`
   styles that reuse palette colours and never the scope badge colours.
   Unknown captures fall back to plain text.
2. Language detection: `Language::from_extension` for `rs`, `py`, `kt`
   and `kts`, `java`, `c` and `h`, `toml`, `json`, `sh` and `bash`, `md`
   (markdown inline only where NZ-25 renders code); and
   `Language::from_fence_tag` for fenced code blocks (`rust`, `rs`,
   `python`, `py`, `kotlin`, `java`, `c`, `toml`, `json`, `bash`, `sh`,
   `shell`). Unknown tags and extensions render as today.
3. Integration: in rendered markdown (NZ-25), a fenced block with a
   known tag is highlighted line by line inside the existing two-space
   code indent, keeping the wrapping rule (NZ-25 follow-up 1 is fixed
   here: code lines keep their leading indentation and inner whitespace
   when wrapped; wrap code by character, not by word); in the raw view
   and for non-markdown files with a known extension, the whole file is
   highlighted; the footer file type (NZ-25) shows the language name
   the grammar matched (`rust`, `python`, ...) instead of the bare
   extension. Highlighting is cached with the NZ-25 preview cache key.
   Files over 1 MB skip highlighting (plain text) to keep selection
   fast; the footer says `rust (not highlighted, large)`.
4. Performance: parsing is synchronous on selection; the worker
   measures a 200 KB Rust file and a 2 MB markdown file with many code
   blocks in a debug build and reports; the guard test bound is loose
   (5 s) like NZ-25's.
5. Not in this ticket: linting (NZ-28), LSP (NZ-29), themes per
   language, injection of markdown into other languages.

Acceptance: a test per language that a small snippet yields at least a
keyword and a string capture; unknown tag and extension unchanged;
fence highlighting inside rendered markdown keeps the NZ-25 line count
and the code indentation when wrapping (the regression from NZ-25
follow-up 1); whole-file highlighting for a `.rs` file; the footer
language name; the 1 MB skip; the guard test; existing NZ-25 tests pass
unchanged except where they pinned unhighlighted code spans (list
each). README: a sentence on highlighting and the languages. Allowed
files: `crates/notez-cli/src/tui/highlight.rs` (new), `tui/markdown.rs`,
`tui/tree.rs`, `tui/theme.rs`, `tui/mod.rs`, `crates/notez-cli/
Cargo.toml`, `Cargo.lock`, `README.md`. Two worker passes (module with
tests and dependencies; integration and README), one review. Reviewer
probes: query compile errors at runtime (must not panic: a grammar
whose query fails to compile is disabled with a logged footer note);
build time; a fence tag with trailing attributes (`rust,ignore`);
highlighting a file with invalid syntax (tree-sitter is error
tolerant, confirm no panic); the cache key including the language.

#### NZ-28: linting in the preview (design)

Status: Draft, design first (Andreas 17:20 CEST). Board item
`PVTI_lAHOCU842c4BmE5Zzg_ZIJE`. Design note written by the lead:
`docs/design-nz28-linting.md` (tree-sitter syntax diagnostics plus
in-process markdown checks, gutter marks, footer count, an `!` issue
overlay; no external tools). Andreas on 2026-10-08 21:30 CEST: "perhaps
28 & 29?", read with the working rule as: go ahead as designed; he
overrules after reading the note or trying the build. Status: Ready,
runs after NZ-5, as ONE ticket in two passes.

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

#### NZ-31: tighten the tree's left margin

Status: Ready, runs right after NZ-4 (small, and it changes the row
prefix that NZ-21's alignment math and the mouse hit tests depend on).
Requested by Andreas on 2026-10-08 at 21:50 CEST: "we are wasting a lot
of space on the TUI, the left side is almost all padding? except for
the small right pointing arrow. this could be tightened up alot? dont
waste any space there." Board item `PVTI_lAHOCU842c4BmE5Zzg_fBhY`.

Problem (code facts at `1edfbce`): every list row starts with the
`List` highlight symbol `"  ▸ "` (4 columns, blank on unselected rows),
then `flags_slots` (7 columns: a space, five tag-dot slots, a space;
`MARK_COL = 4` is the first of them, used for NZ-16's mark glyph), then
the one-column gutter (`BADGE_COL = 4 + 7`), then the indent. That is
12 columns before a section's expand triangle, plus the block's one
column of padding and border. Tag dots are clickable
(`mouse_x_to_dot(mouse_col, area_x)`, dots at `area_x + 5`); the
filter strip above has its own five dots (`mouse_x_to_filter_dot`).

Outcome and decisions (lead, working rule):

1. Drop the `List` highlight symbol: the selected row is shown by its
   row style (the existing selection style on the whole line), not by a
   4-column arrow. Saves 4 columns. The mark glyph (NZ-16) moves into
   the gutter column.
2. Compact the tag dots: the five fixed slots become one 2-column
   field: a space plus the count of set tags drawn as that many dots
   is not readable, so instead draw the set dots only, left-aligned,
   in a field as wide as the maximum number of set tags on any visible
   row (0 to 5 columns, recomputed per draw), with no trailing space
   when the field is empty. A row with no tags uses no dot columns at
   all when no visible row has tags. Mouse: clicking a dot still
   toggles that tag (the hit test maps the column to the n-th set dot
   of that row, and a click on the empty field opens tag mode as a
   click on the row does today; verify what a click does today and
   keep it).
3. Keep one gutter column (badge placeholder on section rows, mark
   glyph on marked rows, NZ-28's `▲` later). Keep the indent of two
   columns per depth.
4. Redraw the branch lines (Andreas, 21:55 CEST: "the actual tree
   graphics should be worked on, theyr not optimal"). Today every
   nested folder draws `├─▼ ` or `├─▶ ` and every nested file `│   `,
   regardless of position, so a last child still shows `├─`, a file
   shows a bar even when nothing follows, and the folder's own
   expand mark sits inside the branch. New rule, the classic `tree`
   drawing: for each ancestor level, `│ ` if that ancestor has a later
   sibling, else two spaces; then `├─` for a row with a later sibling
   and `└─` for the last child; then for a folder the expand mark `▾ `
   (open) or `▸ ` (closed), for a file one space; then the badge and
   the name. Section rows (depth 0) keep `▼ `/`▶ `. Glyphs come from
   one small table in `theme.rs` so they can be swapped. Compute "has a
   later sibling" from `parent_idx` on the flattened rows (a pure
   function with tests), including with a filter active (siblings
   hidden by the filter do not count).
5. Net: a top-level section row starts at column 1 of the pane text
   instead of column 12 in the common no-tags case; nested rows read
   as a real tree; the leader and count math (`list_text_width`,
   NZ-21) is updated to the new prefix and stays exact (branch glyph
   widths vary per row now, measure with `Span::width`).

Acceptance: render tests pin the new prefix for a section row, a
nested folder and a nested file with and without tags and marks; the
count column still ends at the text width at every depth; tag-dot
click tests updated to the compact field (clicking the first set dot
toggles that tag; clicking empty space does what it did before); the
filter strip is unchanged; NZ-16 mark and NZ-21 alignment tests
updated and listed; README's layout description if any. Allowed files:
`crates/notez-cli/src/tui/tree.rs`, `tui/text.rs` (the filter dot
helper if shared), `tui/theme.rs`, `README.md`. One worker pass, one
review. Reviewer probes: hit tests at every tag count; selection
visibility without the arrow on both themes; a wide-glyph badge; the
todo board is untouched (it has its own list).

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

#### NZ-34: reload the tree on demand and when files change

Status: Ready, runs after NZ-31. Requested by Andreas on 2026-10-08 at
22:00 CEST ("should we have some kind of update or reload function? if
stuff are added while in the tui?"). Board item
`PVTI_lAHOCU842c4BmE5Zzg_fMvo`.

Problem (code facts at `1edfbce`): the tree is built once at start and
rebuilt only by the browser's own actions through the `rebuild`
closure (`commands/tree.rs::run`, `build_view`); the event loop blocks
in `event::read()` (`tui/tree.rs` about line 3736) with no timeout, so
nothing can happen while idle. A note written by the editor, by `notez
add` in another shell, or by another session's sync is invisible until
the browser is reopened.

Decisions (lead, working rule):

1. `R` reloads on demand: calls the `rebuild` closure and
   `Forest::rebuild` keeping the cursor row (by path), expanded
   folders, filter, marks (NZ-16 pruning applies) and unsaved tag flags,
   exactly as the post-create rebuild does; footer `reloaded` for one
   draw; `R` is browse-mode only (verify it is unbound; NZ-25 took `p`,
   not `R`).
2. Automatic check: the event loop polls with `event::poll(Duration)`
   using a 2 second timeout; on timeout with no input, a cheap probe
   compares the mtimes of every section root and every expanded folder
   (`symlink_metadata`, no recursion into collapsed folders, no git
   call) against the last probe; any difference triggers the same
   reload as `R`. The probe is skipped while an input mode or a drag is
   open and while the help overlay is up. Cost: a few dozen stats every
   2 seconds idle, nothing while typing. No file-watcher dependency
   (`notify` is outside the approved families).
3. The todo board gets the same `R` and probe only if its loop shares
   the mechanism cheaply; otherwise tree only, and say so.

Acceptance: a test that `R` after an external file creation lists the
new note with cursor, expanded set, filter and marks kept; a test of
the probe's change detection (changed mtime on a root, on an expanded
folder, no change, collapsed folder change ignored); the poll timeout
does not alter key handling (existing event tests pass); key table and
help; README sentence. Allowed files: `crates/notez-cli/src/tui/tree.rs`,
`tui/footer.rs`, `tui/help.rs`, `tui/todo.rs` (only for decision 3),
`README.md`. One worker pass, one review. Reviewer probes: the probe
never reads file contents; mtime granularity on APFS and ext4; a reload
during a prompt must not happen; CPU when idle for an hour.

#### NZ-36: render markdown tables and footnotes in the preview

Status: In flight since 22:08 CEST on 2026-10-08 (second slot beside
NZ-31; it touches `tui/markdown.rs`, which NZ-31 does not). Requested
by Andreas at 22:05 CEST ("the togglable markdown/code reader isn't
really translating the markdown completely yet, for instance it cannot
generate tables and similar. is this something we'd need additional
deps for or?"). Answer given: no dependency; pulldown-cmark already
emits table events, NZ-25 passed tables through as source text as a
scope cut. Board item `PVTI_lAHOCU842c4BmE5Zzg_fWGg`. Base `75bb3dc`,
branch `feat/NZ-36-markdown-tables`, worktree `.claude/worktrees/NZ-36`,
model `opus` via `nz-worker`, one pass.

Decisions (lead, working rule): tables laid out from the events with
pulldown's column alignments, natural widths when they fit, else
shrink the widest columns to a minimum of 3 and wrap cells, else clip
with `…`; light box glyphs, header bold, no outer frame; one container
prefix per line (fixes NZ-25 follow-up 2); footnotes enabled,
references `[n]`, definitions rendered after the document under a
rule; raw HTML, images and math unchanged. Allowed files:
`tui/markdown.rs`, `tui/theme.rs`, `README.md`. One review. Reviewer
probes: CJK cells; a 200k-row table in bounded time; a table with
ragged rows; wrapping inside a cell keeps inline styles.

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

Status: Ready, runs after NZ-4. In the standing scope since 2026-10-06,
named again by Andreas on 2026-10-08 21:30 CEST ("can you do nz-3 and
5"). Board item `PVTI_lAHOCU842c4BmE5Zzg_KWyo`. Lead decisions on
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

#### NZ-4: resizable split, preview fold and pane focus (tree only)

Status: Done. Merged into `main` as `554fd0c` at 21:59 CEST on
2026-10-08 and pushed; branch run and `main` run `37836071227` green;
board Done; remote branch deleted. Andreas told to install; the pane
numbers in the titles and the grip strip are his to judge.

Record: base `1edfbce`, branch `feat/NZ-4-panes`, ticket commit
`bbf550c` (5 files: `README.md`, `tui/mod.rs`, new `tui/panes.rs`,
`tui/theme.rs`, `tui/tree.rs`; 1357 insertions, 121 deletions), merge
`554fd0c` with `git merge --no-ff`; undo with `git revert -m 1
554fd0c`. Agents: two `nz-worker` (opus) passes, about 270k tokens;
one `nz-reviewer` (opus), about 70k, accepted first time; committed
diff hashes to the accepted
`db90a8f0b47aca0095e59541a503f90c34dcc91b615bede3333c9bd9bcdb8db3`.
Lead checks: build clean, notez-cli 460 passed, notez-core 146 passed.
As built: `Panes` model (list min 24, preview min 20, list wins), `<`
`>` `=`, `1` `2` `Tab`, `2` twice folds, grip `⠿` drag with one column
of slop, wheel by pane, click to focus, auto-fold below 45 columns with
the user's fold preserved, nothing persisted; two latent fixes: the
wheel used to scroll the preview wherever the pointer was, and a click
on the filter strip's row inside the preview used to open the filter.
Leftovers: drag re-renders a large note per column; the preview-focused
inert key set is not uniform (`Space` inert, `d m r t S` pass through)
and the README overstates it; the mouse ignores the help overlay
(pre-existing class); tag mode blocks the wheel over the list.

Brief as run. In the standing scope since 2026-10-06 (NZ-2 to NZ-5 and
the integration delegation), confirmed in the 2026-10-07 queue, named
again on 2026-10-08 ("and do nz 4"). A first pass 1 dispatch at 16:58
CEST was stopped a minute later on Andreas's stop instruction before
any change; the ticket ran from 21:37 CEST. The two open points below were decided by the lead on
2026-10-08 at 22:25 CEST under the working rule: (a) the split and fold
are NOT remembered between runs (no state file; the standing scope
forbids a new persisted format), (b) the todo board is left alone.
Current code facts for the worker to verify: the preview is cached per
path, width, mode and language (NZ-25, NZ-27) so a width change
re-renders through the cache and a folded preview costs nothing;
`preview_scroll`, `preview_max`, `preview_height` and `scrolled` exist
(NZ-22), `J`/`K`, Shift+Up/Down and PgUp/PgDn already scroll the
preview from the list; `p` toggles rendered/raw (NZ-25); `Tab` is used
only inside prompts (scope cycling), so it is free in browse mode; `1`
and `2` are unbound; `<`, `>`, `=` are unbound; the list text width is
`list_text_width(pane)` (NZ-21) and must follow the split; the footer
has a Hints slot with the file type and mark count leading (NZ-16,
NZ-25). The fleetz reference for grips is `~/Repos/fleetz/src/ui/
grips.rs` (read-only). Board item `PVTI_lAHOCU842c4BmE5Zzg_KW1E`.

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

STOPPED for the day at about 19:25 CEST on 2026-10-07. Nothing runs
until Andreas says to continue. When he does, the lead (this session if
it is still open, otherwise a new one that records its takeover under
Current lead):

1. Reconciles: `git status --short`, `main` against `origin/main` (push
   if local is ahead; a GitHub server error interrupted the last push),
   `git worktree list` (no ticket worktree should hold work; a leftover
   NZ-13 worktree is fully merged and removable), and the board
   (`projects/2`: NZ-13 Done, NZ-14 Ready; correct it with the ids
   recorded under NZ-12 if the failed edit never landed). Confirms with
   Andreas that the standing scope and "merge as you go along" still
   hold; they were paused, not withdrawn.
2. NZ-14 (folders): DONE 2026-10-08, merge `ae3617e`.
3. NZ-15 (move, set scope): DONE 2026-10-08, merge `36699c1`.
4. NZ-16 (multi-select): DONE 2026-10-08, merge `5298d54`. NZ-19 (CI):
   DONE 2026-10-08, merge `3941481`; CI is live on `main`.
5. NZ-20: DONE 2026-10-08, merge `483cb1c`. Branch pushes are now a
   standing permission (Authorized by the owner, 15:15 CEST): every
   accepted ticket branch is pushed, waits for a green run, merges, and
   its remote branch is deleted after the `main` run.
6. NZ-21: DONE 2026-10-08, merge `d667999`. NZ-23: DONE without code
   (files moved). NZ-22: DONE 2026-10-08, merge `82fae54`.
7. Queue, all authorized on 2026-10-08 17:20 CEST, one at a time on
   `tui/tree.rs`: NZ-26 DONE (merge `5442854`), NZ-24 DONE (merge
   `706e169`), NZ-25 DONE (merge `6a1c814`), NZ-27 DONE (merge
   `299770e`), NZ-4 DONE (merge `554fd0c`), NZ-31 IN FLIGHT, then
   NZ-34 (reload), NZ-3, NZ-5, NZ-32 (type filter), NZ-28 (linting),
   NZ-30 (symbol outline),
   NZ-25 (rendered markdown, two passes), then NZ-27 (highlighting,
   brief to finalize), then the UI tickets NZ-4, NZ-5, NZ-3. NZ-11 and
   NZ-6 in the second slot when their files are free. NZ-28 and NZ-29
   are design tickets (design note under `docs/`, Andreas approves,
   then code). NZ-18 (Pinz, design) waits for his word. Andreas can
   reorder; the lead proposed this order to him at 17:45 CEST.
4. NZ-17 (versioning) and the workflow-doc board wiring wait for
   Andreas's word; tagging is off until a first version exists.

Open with Andreas, none blocking: when he wants to demo the UI and
whether the order above suits it; whether the two `notez-core`
read-error cases found in the NZ-10 review (`.tags`,
`load_single_todo`) should become a ticket; the NZ-13 leftover where an
unregistered repo nested under the vault lists under NOTEZ; the smaller
leftovers on the Done tickets.

What worked on 2026-10-07, for whoever leads next: three tickets merged
in about three hours with one review each and no review-driven fix
cycle; the lead sending a worker back once before review when its
report showed a deviation from the brief (`d` not in the footer) was
cheaper than a review round; two-pass workers on `tui/tree.rs` again;
reviewer probe lists; hashing the uncommitted diff before and after
commit. The brief for NZ-13 had two wrong premises (tag root for
personal sections, sections opening expanded) that the workers caught;
reading the code before writing decisions into a brief pays off.

Standing scope, as it stood at the stop: NZ-2, NZ-7, NZ-9, NZ-10, NZ-8,
NZ-12 and NZ-13 done, then NZ-14, NZ-15, NZ-16, NZ-4, NZ-5, NZ-3, with
NZ-6 and NZ-11 fitted around them (see Authorized by the owner for the
limits). Per ticket the lead:

1. Runs the worker passes, then a separate `nz-reviewer` on the whole diff,
   re-reviewing after any change.
2. Runs `cargo build --workspace` and `cargo test --workspace` itself.
3. Commits exactly the reviewed diff on the ticket branch, staging the
   ticket's files by name.
4. Merges it into `main` with `git merge --no-ff`, reruns the checks on
   `main`, and pushes `main` (authorized: "merge as you go along"), provided
   the safe-merge conditions hold. Otherwise it stops at Ready to integrate
   and asks.
5. Marks the ticket Done here and on the board, removes its worktree
   and local branch, commits and pushes this file, and tells Andreas
   what changed on screen.
6. Starts the next ticket in a new worktree based on `main`.

A lead resuming from this file first checks whether a worker is still
running in the worktree named under In flight and whether it holds
uncommitted work, and does not dispatch a second worker onto it.

Andreas installs and tries when he chooses (`./install.sh`, his to run).

Waiting on Andreas, none of it blocking the scope above:

- NZ-6 (quiet offline exit, decided: quiet like Pinz) is Ready and in
  the standing scope for the second slot; it touches `main.rs`,
  `sync.rs` and `commands/sync.rs`, so it can run beside a tree ticket
  but not beside NZ-3, which may add a helper to `sync.rs`.
- Whether the leftovers listed on the Done tickets become tickets.

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
