# NZ-47: dates as a first-class part of a note (design)

Status: draft for Andreas's decision, written by the Relay lead on
2026-10-15. No code runs from this note until he approves it; every
`notez-core` change below needs his explicit yes (epoz pins the crate
by rev, `5f24836` today).

## Why

Andreas, 2026-10-15: dates are "a big and important addition,
concerning both pinz notes and what could be adjacent to a calendar
TUI, slack bots and more. should be an integral part of a notez note
.md document". `DESIGN.md` already calls the date encoding "the crux"
and "the gating decision" for anything calendar-shaped: `TODO.md`
round-trips byte-for-byte through `notez`, `todoz` and nvim, so the
encoding must survive the CLI untouched.

## What exists today

- Notes: `notez add` writes `# Title`, a blank line and `Date:
  YYYY-MM-DD` (`core/note.rs::rendered`). Quick notes get a
  `YYYY-MM-DD-` filename prefix. Nothing parses either; the UIs sort
  and light calendars by mtime.
- Todos: `- [ ] text #tag #tag` lines. `tags::parse_flags` strips the
  tag suffixes into a `u8`, `serialize_flags` writes them back. The
  serializer is canonical for checkbox lines and leaves other lines to
  the dirty-source rule (`todo::save_todos_for`).
- Pinz: a pin file is a frontmatter block (`---`, `x:`, `y:`, `z:`,
  `color:`, `---`), then `# Title` and the body. `pinz-core`'s
  `split_frontmatter` is forgiving: no block means a plain note.
  `pinz-core/src/model.rs` says a pin is "a notez2 note plus the
  spatial metadata", "exactly what would serialize into a notez2
  file's frontmatter, so a note can round-trip between pinz and a
  notez2 workspace". Pinz has been waiting for notez to grow a
  frontmatter.
- epoz: `Calendar.svelte` sits in the todoz view, navigation only, no
  date model. It reads `Task` through `notez-core`'s serde types.

## What a date on a note means

Three fields, each optional, all ISO `YYYY-MM-DD` with an optional
`THH:MM` (local time):

- `created`: when the note was made. Already implicit; made explicit
  so it survives copies and syncs that reset mtime.
- `due`: a deadline. The one todos need.
- `event`: when the thing the note is about happens (a meeting, a
  booking, a release).

Product rule that keeps this small: a todo is due, a note is on a
date. A todo line carries a `due` only; a note may carry any of the
three. Done-dates, recurrence and reminders are out of scope for the
first version and are listed under Later.

## Encoding (recommendation)

Notes: a frontmatter block at the top of the file, the Pinz format.

```markdown
---
created: 2026-10-15
due: 2026-10-20
event: 2026-11-02T14:00
---
# Book the venue

Date: 2026-10-15
```

Rules: `key: value` lines between two `---` lines starting at byte 0;
unknown keys are preserved verbatim and in order (so a Pinz pin's
`x`, `y`, `z`, `color` survive a notez edit); a file without a block
is a plain note; a block is only written when a date is set, and the
human `Date:` line in the template stays as it is for now.

Todos: an inline token on the task line, like tags.

```markdown
- [ ] send the contract @2026-10-20 #prio
```

`@YYYY-MM-DD` or `@YYYY-MM-DDTHH:MM` anywhere after the checkbox is
the due date; the parser strips it the way it strips `#tags`, the
serializer writes it back after the text and before the tags. At most
one per line; a second is left in the text. Non-checkbox lines are
untouched, as today.

Why not the alternatives:

- Inline tokens in notes too (`@due 2026-10-20` in the first line):
  mixes metadata with prose and breaks the moment a note quotes a
  date. Frontmatter is the convention every markdown tool knows.
- Frontmatter in `TODO.md` lines: a line cannot carry a block; the
  file-level block would date the whole list, not a task.
- A sidecar index like `.tags`: dates are content, not UI state. They
  must travel with the file, show in nvim and survive a rename. (The
  `.tags` importance flags could move into frontmatter one day for the
  same reason; not now, epoz and the tree browser both read `.tags`.)

## `notez-core` changes (minor bump, 0.2.0 under the NZ-17 policy)

1. `core::frontmatter`: `split(text) -> (Vec<(String, String)>, &str
   body)` and `set(text, key, value) -> String` that inserts or
   replaces one key and leaves everything else byte-identical,
   including unknown keys and the body. Same forgiving rules as
   `pinz-core::split_frontmatter`, so Pinz can later depend on this
   module instead of its own.
2. `core::dates`: parse and format the two accepted shapes; a
   `NoteDate { date: NaiveDate, time: Option<NaiveTime> }` type.
   `chrono` is already a dependency; no new crate.
3. `NoteEntry` (aggregate) gains `created`, `due`, `event:
   Option<NoteDate>`, filled from the frontmatter when present
   (`created` falls back to the `Date:` header, then the filename
   prefix, then nothing; mtime stays what it is). Additive, serde
   default, so epoz's DTOs keep deserializing.
4. `Task` gains `due: Option<NoteDate>`. `parse_todos_from_content`
   returns a `(String, CheckState, u8, u8)` tuple today and is `pub`;
   adding a field means a signature change. Proposal: a new
   `parse_todo_lines` returning a small struct, the tuple function kept
   as a thin wrapper until epoz has moved. Check epoz's call sites
   before the ticket is cut.
5. `serialize_tasks_for_file` writes `@date` back. A round-trip test
   proves a `TODO.md` with dates, tags, prose and nested tasks is
   byte-identical after load and save.
6. `notez_core::filter` is not touched in the first version; date
   filtering lives in the CLI's `@` token family from NZ-32.

## First consumers, one ticket each, after approval

1. todoz: the due date shows right-aligned in the row, dimmed when
   far, lit when within seven days, red when overdue; `D` sets or
   clears it with a one-line prompt accepting `2026-10-20`, `+3d` and
   `clear`. Sorting is unchanged; a `@overdue`-style filter can follow.
2. Inspector footer: the selected note's dates (`due 2026-10-20`, `on
   2026-11-02 14:00`, `created` when there is room) in the preview
   footer next to the language badge from NZ-39. Nothing in the tree
   rows (Andreas, 2026-10-15: the row stays clean; per-note detail
   belongs in the inspector). `@due`, `@overdue`, `@today`, `@week`
   tokens for the filter are a later option, not part of the first
   version; `agenda` answers "what is coming up".
3. `notez agenda [--days N] [--json]`: dated notes and due todos across
   all scopes, grouped by day, overdue first. This is the seed of a
   calendar TUI and the call a Slack bot or a routine would make.
4. `notez add --due <date>` and `--on <date>` write the frontmatter at
   creation.
5. epoz: binds `Calendar.svelte` to the real fields through the DTOs.
   No notez work; it moves its pin when it wants the fields.

## Pinz

With `core::frontmatter` in `notez-core`, a pin file is already a
valid notez note with extra keys. NZ-18 ("Pinz notes as a special
note type") then reduces to: notez reads `~/pinz-board` read-only
through the same parser, shows pins as a note type (the `pinz` token
NZ-32 reserved), and dates on pins come for free. Pinz keeps its own
repo, sync and lock; notez never commits into `~/pinz-board`.

Checked 2026-10-15: `pinz-core/src/file_store.rs::render_pin` writes
only `x`, `y`, `z` and `color` and drops every other key when Pinz
saves a pin (a drag saves). So a `due:` written into a pin file would
be lost on the next drag. Before dates on pins mean anything, Pinz
needs a small change: `parse_pin` keeps the unknown pairs and
`render_pin` writes them back in order. That is a Pinz ticket. Until
it lands, notez reads pins and never writes into `~/pinz-board`,
which is the NZ-18 rule anyway. Old notez notes need nothing: a file
without a block is a plain note, `created` falls back to the `Date:`
header, the filename prefix, then nothing, and no file is rewritten
until a date is set on it.

Andreas, 2026-10-15, on the first round of questions: frontmatter
format agreed ("good format"), `agenda` agreed, the malformed-value
rule agreed ("kept verbatim until linting"), the `@date` token rule
agreed. Still open: questions 2, 4, 6, 7, 8 below.

## Rules decided by the lead (working rule, 2026-10-15)

- `agenda` lists `due` and `event` only; a note with just `created`
  does not appear. Checked tasks with a past due are hidden. Daily
  logs (`01_daily-logs/`) are never listed.
- A malformed value (`due: next friday`) is kept verbatim and treated
  as absent; never an error. A lint rule can follow when linting
  unparks.
- Times are local wall-clock, no zone stored. Dates-only in the first
  version sidesteps it; a synced note keeps its written time.
- The `@date` token on a todo line must be whitespace-delimited and
  match the exact shape, so `name@example.com` or `@2026` in prose is
  never read as a date.
- The `created` fallback from the filename also accepts Pinz's
  `YYYY-MM-DD-HHMMSS-` prefix.
- Setting a date on an existing note: nvim only in the first version
  (the inspector footer shows the result). A `D` key in the tree
  browser or a `notez date` command is a later ticket, if missed.
  Andreas decides (question 7).

## Later, deliberately not in the first version

Recurrence (`every monday`), reminders and notifications, done-dates
on checked tasks, natural-language dates beyond `+Nd`, time zones (all
times are local), a calendar TUI view, date-based sorting of the tree.

## Decisions for Andreas

1. Frontmatter for notes and `@date` tokens for todo lines: agreed?
2. Field names `created`, `due`, `event`. (`on` instead of `event`?)
3. Dates only in the first version, with the time part parsed and
   kept but not shown anywhere yet: agreed?
4. New notes keep the `Date:` line and get frontmatter only when a
   date is set (recommended), or every new note starts with a
   `created:` block from now on?
5. Order of consumers: todoz due plus `notez agenda` first
   (recommended), tree marker and filters second.
6. Permission for the `notez-core` changes in the list above as one
   ticket (NZ-47a), bumping the workspace to 0.2.0 with a `notez-core`
   changelog entry.
7. Dating an existing note: nvim only at first (recommended), a `D`
   key in the tree browser, or a `notez date <note> --due <date>`
   command?
8. NZ-18 (Pinz as a note type): fold it into this rollout as NZ-47d
   (read `~/pinz-board` through the same parser, read-only, Pinz owns
   its repo), or keep it separate for its own brainstorm?

## Rollout

NZ-47a `notez-core` frontmatter, dates, fields and round-trip tests,
read-only (no write path yet); NZ-47b `notez agenda` and the inspector
footer; NZ-47c the write path: todoz due column with the `D` prompt
and `notez add --due/--on`, after the parser has run on the real vault
for a while. One worker and one review each; 47b and 47c can run in
parallel on disjoint files once 47a is on `main`. Lead's own caveat:
reading is safe, writing a note file notez did not create is new
ground (`TODO.md`'s prose-loss bug is the precedent), so `set()` must
be byte-exact outside the one key and only run on explicit actions,
never on load or sync.
