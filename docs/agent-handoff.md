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

None. The last lead, `8a79e020` (Claude, `claude-fable-5-1`), took the
baton on 2026-10-15 11:36 CEST and released it at the stop that day;
its record is in `docs/agent-log.md` under Earlier leads. No ticket has
run since: `main` gained only owner commits migrating the Relay docs
(0.4.0 to 0.6.0, 2026-10-16 and 2026-10-17). On 2026-10-17 the board
moved to issues (`dd917d5`). Check `git log` for later commits on
takeover.

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
- No other ticket execution is authorized. Andreas names which tickets run.

## In flight

Nothing is in flight.

## Tickets

Ticket briefs and records live here; the board at `projects/2` carries
the same Status per ticket. Status values: Backlog (brief written, not yet
approved by Andreas), Ready, In progress, Review, Ready to integrate, Done.
On 2026-10-17 Andreas had the board moved to the Relay 0.6.0 shape: every
draft became an issue in `Gaurgle/notez` (NZ-1 is #30, NZ-52 is #78; Done
tickets closed), Draft became Backlog, and NZ-30 (#59, P2) and NZ-43 (#71,
P1) went into milestone `0.3.0`. Records below that say Draft or In flight
use the old names.

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

### NZ-42 (#70): todoz navigation like the tree browser (2026-10-09, Draft)

NARROWED 2026-10-15 (Andreas "42: ok?" to the lead's proposal): `/`
fuzzy search in todoz reusing NZ-5's matcher and highlight, plus the
reload-on-change behaviour from NZ-34. No `0`/`s` jumps, no pane-focus
keys. Brief to be written after NZ-5 lands and the NZ-43 brainstorm
has happened.

## Constraints

- `notez-core` file formats stay compatible with epoz, which pins the
  crate by git rev. UI work belongs in `crates/notez-cli/src/tui/`.
- No new dependency without Andreas's approval, beyond the family above.
- Look and interaction need Andreas to try the branch build first.

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

## Start the lead

From the repo root: `claude --agent nz-coordinator`. A Codex lead starts from
`AGENTS.md`.
