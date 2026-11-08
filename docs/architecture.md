# notez architecture

The code map: what the structure is (measured) and the shape the owner
accepted. `nz-architect` keeps the as-is sections true; the target shape and
its rules are Andreas's. How the doc is used: `docs/agent-workflow.md`,
"Architecture". Scope model and storage design: `DESIGN.md`.

As-is sections measured 2026-11-08 at `main` = `642ee41`. Counts are from
`rg` over import paths, `wc -l`, and `#[test]` markers; "non-test" lines are
the lines before a file's `mod tests`.

## Overview

One Cargo workspace, two crates, one version (`0.2.0`, `docs/versioning.md`):

- `crates/notez-core`: the engine. Scopes, project discovery, config and
  registry, note aggregation, frontmatter and dates, todos, tags, search,
  migration, vault sync. Plain data in and out, no terminal I/O (measured:
  zero `println!`/`eprintln!`/`stdin` outside tests). Pinned by git rev from
  the desktop app epoz, so its public API and the on-disk formats are a
  contract.
- `crates/notez-cli`: the `notez` binary and its aliases (`todoz`, `treez`,
  `logz`, ...). Three layers inside: `cli` (clap types), `commands` (one
  module per subcommand), `tui` (ratatui browser and todo board).

Size: 44,398 lines of Rust in total (tests included). notez-core 6,279 of
them; notez-cli 38,119, of which `tui/` is about 31,000.

## Module map

### notez-core (`crates/notez-core/src`)

| Module | Owns | Public types |
|---|---|---|
| `config/` (`mod`, `paths`, `metadata`, `registry`) | `config.toml`, per-machine `registry.toml`, synced `.notez-config.toml`; XDG paths | `Config`, `PathsConfig`, `EditorConfig`, `ToolsConfig`, `NotezMetadata`, `ProjectMetadata`, `ProjectRegistry`, `ProjectEntry` |
| `core/scope` | the four scopes | `Scope` |
| `core/project` | git toplevel discovery, `notez/` and `.notez/` stores, scratch gitignore | `Project` |
| `core/resolve` | scope to directory: `root`, `quick_notes`, `daily_logs`; the todos store names `TODOS_DIR` (`_todos`) and `TODO_FILE` (`TODO.md`) | |
| `core/aggregate` | walking every store into one list | `SourceKind`, `NoteEntry` |
| `core/frontmatter`, `core/dates` | the `---` block, `NoteDate` parsing | `NoteDate`, `NoteDates` |
| `core/note` | | `Note` |
| `todo/` (`mod`, `model`) | `TODO.md` parse and serialize, board load and save, tree edits (toggle, drag, add, remove, collapse) | `Task`, `TodoLine`, `CheckState` |
| `tags`, `note_tags` | flag definitions and `.tags` files | `FlagDef` |
| `filter` | `@type` and flag filter language | `Filter` |
| `search` | full-text search | `SearchHit` |
| `sync` | `commit_pending`, `auto_sync`, `pull_on_open` over `git` | `AutoSync` |
| `migrate` | notez-cli layout to notez | `PlanItem` |
| `util/` (`sanitize`, `tilde`, `tools`) | file names, `~` display, external tool lookup | |

Largest file: `todo/mod.rs`, 947 lines (629 non-test). 204 unit tests.

### notez-cli (`crates/notez-cli/src`)

| Layer | Files | Role |
|---|---|---|
| `main.rs` (1,054; 588 non-test) | | argv rewrite for aliases, `decide` which action, config load, auto-register discovered projects, open-pull and after-sync around interactive sessions, help text |
| `cli/mod.rs` (334) | `Cli`, `Commands` | clap derive types only |
| `commands/` | 21 modules, `add` 1,343 lines (492 non-test), `tree` 1,073 (446), `agenda` 623 (326), `todo` 332, the rest under 300 | one `run` per subcommand; `tree` and `todo` build the data and start a TUI session |
| `tui/` shared | `mod` (enter/leave, editor, Vim command mode), `header` (`SyncState`, `VaultRepo`), `footer`, `panes`, `prompt`, `line_edit`, `notice`, `help`, `theme`, `text`, `clipboard`, `yank`, `sync` (`VaultSync` background runner), `tags`, `todo_rows`, `move_path` | widgets and session plumbing shared by both sessions |
| `tui/markdown`, `tui/highlight`, `tui/lint` | 1,931, 895, 787 lines | preview rendering: pulldown-cmark, tree-sitter grammars, diagnostics (lint computed, not yet shown) |
| `tui/tree/` | 15 files, 16,000 lines total; `render` 2,760 (799 non-test), `input` 2,070 (1,692), `move_prompt` 2,016 (691), `preview` 1,741 (608), `bulk` 1,618 (614), `model` 1,593 (684), `delete` 1,556 (424), `test_support` 697 | the browser: `Forest` of `TreeNode`s built from `SectionSpec`s, one event loop in `input.rs` |
| `tui/todo.rs` | 5,151 lines, 3,059 non-test, one file | the todo board: `run_board`, one `event_loop` (lines 1586 to 3011, 1,425 lines) |

838 unit tests in `src`, 5 integration tests in
`tests/storage_scenarios.rs` (DESIGN.md scenarios A, C, E, I, J; B, D, F,
G, H, K have no test).

## Dependencies

Crate edge: `notez-cli -> notez-core` only. notez-core depends on no
workspace crate and no UI crate (serde, toml, dirs, chrono, anyhow, which,
walkdir).

Inside notez-core (`use crate::` edges, 19 counted): `todo -> config, core,
tags`; `filter -> tags`; `search -> core, config`; `migrate -> config, util`;
`config/registry -> core, util`; `core/resolve -> config`; `core/aggregate ->
config`; `core/project -> util`. No cycles beyond `config <-> core`
(`registry` uses `core::Project`, `resolve` and `aggregate` use `Config`).

notez-cli into notez-core (87 distinct file-to-module edges): `config` 22,
`core` 18, `util` 14, `tags` 11, `todo` 6, `sync` 6, `filter` 6,
`note_tags` 2, `search` 1, `migrate` 1.

Inside notez-cli:

- `main -> cli, commands, tui::header::SyncState, commands::tree::View`.
- `commands -> tui`: `commands/tree.rs:18-19` (`SyncState`, `run_tree`,
  `TreeContext`, `SectionSpec`, `NewNoteRoots`), `commands/todo.rs:18-19`
  (`SyncState`, `run_board`, `BoardContext`), `commands/cp.rs:13`
  (`clipboard`).
- `tui -> commands`: `tui/tree/mod.rs:40` (`commands::{add, mkdir,
  rename}`), used by the browser's new-note, mkdir and rename prompts.
  Together with the line above this is a `commands <-> tui` cycle.
- `tui -> cli`: none.

`git` is spawned from 14 files. In core: `sync.rs`, `core/project.rs`,
`core/resolve.rs`. In the CLI: `commands/tree.rs` (4 sites),
`commands/sync.rs` (3), `tui/sync.rs` (3), `tui/header.rs` (3), `main.rs`,
`commands/add.rs`, `commands/mkdir.rs`, `commands/logz.rs`,
`tui/tree/folder.rs` (1 each), plus `build.rs` and the test helper.

No layer test exists; the edges above are hand-counted with `rg`.

## Runtime and data flow

1. `main` rewrites argv for the alias the binary was invoked as
   (`rewrite_for_symlink`), parses `Cli`, loads `Config`, and runs
   `decide` to pick an action: a subcommand, the quick-note shorthand, or
   the browser for bare `notez`.
2. A subcommand's `commands::<name>::run(...)` does the work and prints.
   `add` creates the note file, then `main::report_created` opens it in the
   editor and syncs if asked.
3. The browser (`commands/tree.rs`) resolves the repo, registry and
   metadata, calls `notez_core::core::aggregate::collect_all`, groups the
   entries into `SectionSpec`s (one per store, ordered current repo, then
   global, then other projects, per DESIGN.md "TUI aggregation"), and hands
   them to `tui::tree::run_tree` with a `rebuild` closure. The session
   returns changed `.tags` maps per tag root and the caller persists them.
4. The todo board (`commands/todo.rs`) calls `notez_core::todo::load_board`
   or `load_scope_board`, then `tui::todo::run_board`. The session returns
   `BoardOutcome { items, dirty }`; only dirty source files are rewritten
   through `todo::save_todos_for`.
5. Around both sessions `main` runs the opening pull
   (`notez_core::sync::pull_on_open`), classifies it into a
   `tui::header::SyncState` for the header, and runs `auto_sync` after.
   Inside a session `g` runs `tui::sync::VaultSync` in a thread; the board
   also polls source files (`probe_*`) and reloads on change.
6. The preview pane (both sessions) reads the selected file and renders it
   through `tui::markdown` or `tui::highlight`, cached by `PreviewKey`.

epoz calls notez-core directly (at least `todo::load_board`, per
DESIGN.md) and shares every on-disk format.

## State and who changes it

| State | Lives | Written by |
|---|---|---|
| `config.toml`, `registry.toml` | XDG config dir | `notez_core::config` (`Config::save`, `ProjectRegistry::save`); `main::register_discovered` and `commands/{attach,detach,setup,init}` call them |
| `.notez-config.toml` | vault root, synced | `notez_core::config::metadata` |
| Note files | the four scope roots | created by `commands/add.rs` (`create_new_note_file`, 4 write sites), `commands/log.rs`, `commands/logz.rs`; renamed, moved and deleted by `commands/rename.rs`, `commands/mkdir.rs`, `tui/move_path.rs` (4), `tui/tree/delete.rs` (3), `tui/tree/rename_prompt.rs`; edited only by the external editor |
| `TODO.md` files | per store and `_todos/<category>/` | `notez_core::todo::{save_all_todos, save_todos_for}`; the board calls them on exit and before a sync; `commands/todo.rs` creates a category dir |
| `.tags` | per tag root | `notez_core::note_tags` (1 write site). The browser edits the maps in memory, retires keys on rename, move and delete (`bulk`, `delete`, `move_prompt`, `rename_prompt`), and returns them to `commands/tree.rs` to persist |
| The vault git repo | `~/notez` | `notez_core::sync` for commit, pull, push; `tui/header.rs` and `tui/sync.rs` probe it directly |
| Frontmatter dates | inside notes | nobody yet: `frontmatter::set` has no caller (CHANGELOG, Unreleased) |
| Session state (cursor, modes, panes, search, drag, prompts) | locals of `tui/tree/input.rs::event_loop` and `tui/todo.rs::event_loop`, plus `Forest` for the tree | the event loops only; nothing escapes a session except the outcomes above |

## Where the domain rules live

In notez-core, as intended: scope to path (`core/resolve`), store discovery
and the scratch gitignore (`core/project`), what counts as a note
(`core/aggregate`), the `---` block and date grammar (`core/frontmatter`,
`core/dates`), `TODO.md` syntax and tree semantics (`todo`), flags and the
filter language (`tags`, `filter`), sync order and refusal (`sync`),
migration (`migrate`), file-name sanitising (`util/sanitize`).

In the CLI, where epoz cannot reach them:

- Note creation: title to file name with `-N` suffixes, the quick-note
  folder and scope flip, dated notes (`commands/add.rs:186-304`,
  `parse_due`/`parse_on` at 87-118).
- The agenda: which dates of a note or todo become entries, what a daily
  log is (`commands/agenda.rs:77-189`, `agenda`, `is_daily_log`,
  `todo_entries`, `note_entries`).
- Section ordering and labels of the aggregated tree
  (`commands/tree.rs:135-404`, `scope_rank`, `section_meta`,
  `sections_from_entries`), and the `tag_root` rule (personal and global
  share the vault's `.tags`, other stores keep their own).
- The browser's view scopes and document types (`tui/tree/scope.rs`,
  `tui/tree/doc_type.rs`), and the move rules (`tui/move_path.rs`).
- Store names as literals: `core/resolve` names `quick_notes`,
  `daily_logs` and the todos store (`TODOS_DIR`, `TODO_FILE`, NZ-80), and
  core's `todo` and `migrate` and the CLI's `commands/{todo,add}.rs`,
  `tui/todo.rs`, `tui/lint.rs` and `tui/tree/{render,doc_type,delete}.rs`
  read `TODOS_DIR` and `TODO_FILE` from it. `00_quick-notes`, `daily*`
  and the alias names were not recounted.

## Target shape

Drafted by the architect from `DESIGN.md`, `docs/versioning.md` and the
`notez-core` crate doc. Accepted by the owner 2026-11-08 ("I trust you
regarding the new shape"), with his reason for rule 7: a similar project
ended up with `app.rs` and `main.rs` files past 10,000 lines, which is
neither clean code nor encapsulation. Changes to this section are his.

1. **Core is the engine.** Every rule about files on disk (where a scope
   lives, what a note or todo is, how a name, a date or a tag is written,
   how the vault syncs) lives in `notez-core` and returns plain data. No
   terminal, no ratatui, no prompts, no `println!`.
2. **One direction.** `main -> commands -> tui -> notez-core`, and
   `tui -> notez-core`. `tui` never imports `commands`; `commands` import
   `tui` only to start a session and read its outcome. `cli` is types only.
3. **Sessions own their state and return outcomes.** A TUI session holds
   its cursor, modes and buffers privately and returns what changed
   (`BoardOutcome`, the `.tags` maps). Files are written by core functions
   or by the command that started the session, never from a keystroke
   handler.
4. **git goes through `notez_core::sync`.** Vault state (dirty count,
   upstream, head) is read through one core probe; no CLI or TUI module
   spawns `git` itself except for project discovery already in core.
5. **Store names in one place.** `_todos`, `TODO.md`, `00_quick-notes`, the
   daily-log folder and the default folders are named once in core
   (`core/resolve` or config), never as string literals in the CLI.
6. **Contract discipline.** A change to notez-core's public API or an
   on-disk format is a minor bump with a `notez-core` changelog subsection
   (`docs/versioning.md`); epoz compatibility is checked before merge.
7. **Size.** A file stays readable in one sitting: 1,000 non-test lines
   per file and 300 lines per function, applied to new and touched code.
   `tui/todo.rs` is the one standing exception until P2 (the board split)
   lands; the other files over the limit shrink as tickets touch them.

## Owner items

Open: none.

Decided 2026-11-08. Andreas accepted the target shape and delegated the
rest of this survey's items to the architect's recommendation; each stays
reversible by his word.

1. Target shape: accepted as written above.
2. Size guideline: 1,000 non-test lines per file, 300 per function (rule
   7). At the time: `tui/todo.rs` 3,059 non-test lines with a 1,425-line
   `event_loop`; `tui/tree/input.rs` 1,692 with a 1,559-line `event_loop`;
   `tui/markdown.rs` 1,192.
3. Core API moves: the agenda (`commands/agenda.rs`) moves into notez-core
   when ticketed, as a minor bump with a `notez-core` changelog entry;
   note creation (`commands/add.rs`) waits for a ticket that needs it.
4. Layer test: wanted. A unit test in notez-cli that scans `src` for
   `use crate::commands` under `tui/` and for `Command::new("git")`
   outside the allowed files, so rules 2 and 4 run under `cargo test`.

Refactor proposals from the 2026-11-08 survey, for the lead to ticket, in
the recommended order. Each names one seam, the rule it restores and what
it must not change.

- **P1 Todos store name in core.** Add the todos store to `core/resolve`
  next to `quick_notes` and `daily_logs`; replace the `_todos` and
  `TODO.md` literals in `commands/todo.rs`, `tui/todo.rs`,
  `tui/tree/{render,doc_type,rename_prompt}.rs`, `main.rs`,
  `commands/nav.rs`. Rule 5. No path written may change. Before or with
  NZ-76.
- **P3 `SyncState` and `VaultRepo` out of `tui/header.rs`** into
  `notez_core::sync`. Removes the `commands -> tui::header` edge (rule 2).
  Header text unchanged.
- **P4 Note creation into core** (`create_new_note_file`, `suffixed_name`,
  `quick_scope`, `create_dated_in_dir` from `commands/add.rs`) so
  `tui/tree/new_note.rs` imports core, not `commands`. Removes the
  `tui -> commands` edge; with P3 ends the cycle (rule 2). File names
  produced unchanged. Minor bump (item 3).
- **P2 Split `tui/todo.rs`** into `tui/todo/{mod,model,input,render,
  preview}.rs` on the tree's pattern, with a `Board` state struct in place
  of the loop's mode locals. Rule 7. No keybinding, drawn row or saved byte
  changes; the unit tests are the net.
- **P5 Agenda into `notez_core::agenda`** (`agenda`, `todo_entries`,
  `note_entries`, `is_daily_log`), `run` and JSON output stay in the CLI.
  Rule 1. Minor bump (item 3).
- **P6 One vault probe in core** replacing the direct `git` calls in
  `tui/header.rs`, `tui/sync.rs`, `commands/sync.rs`, `commands/tree.rs`.
  Rule 4. The sites were counted, not read: the ticket lists what each one
  asks git before merging them.

## Keeping it true

- A ticket that adds a module, moves a responsibility, adds an event or a
  kind of state, or changes a dependency edge updates the as-is sections
  in the same ticket; the reviewer checks it against the diff.
- The architect re-measures on a survey (milestone retro, after a batch of
  refactor tickets, or two open `Structure` notes) and records the date
  and commit at the top.
- Re-measure with: `rg -o "use crate::([a-z_]+)" crates/notez-core/src`,
  `rg -o "notez_core::([a-z_]+)" crates/notez-cli/src`,
  `rg -n "crate::commands" crates/notez-cli/src/tui`,
  `rg -n 'Command::new\("git"\)' crates`, and `wc -l` over
  `crates/**/*.rs`.
