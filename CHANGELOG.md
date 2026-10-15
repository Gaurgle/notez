# Changelog

All notable changes to notez are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); see
[docs/versioning.md](docs/versioning.md) for what each bump means here.
Both crates, `notez-core` and `notez-cli`, share one version. Changes to the
`notez-core` public API or to any on-disk format go under a `notez-core`
subsection, because epoz pins the crate by git rev and reads this file to
decide when to move its pin.

## [Unreleased]

### notez-core

Dates on notes and todos, read-only (NZ-48, design in
`docs/design-nz47-dates.md`). The workspace version is 0.2.0. Nothing in
notez writes a date into a file yet. epoz's pinned rev is unaffected until
it moves; after a move, every addition below is additive for serde.

- New module `core::frontmatter`: `split`, `get` and `set` for the
  Pinz-style `---` block at the top of a note. `set` changes one key and
  keeps every other byte; it has no caller yet.
- New module `core::dates` with the `NoteDate` type (`date` plus an
  optional `time`), parsed from `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM` and
  serialized as that string. Any other value is not a date.
- `core::dates::NoteDates` (`created`, `due`, `event: Option<NoteDate>`),
  read on demand: `NoteDates::from_text(text, file_name)` is pure, and
  `NoteDates::read(path)` reads at most the first 4 KiB of a file (a read
  error gives no dates). Values come from the frontmatter; `created` falls
  back to a `Date:` line near the top, then a `YYYY-MM-DD-` or
  `YYYY-MM-DD-HHMMSS-` filename prefix. `NoteEntry` and `collect_all` are
  unchanged.
- `Task` gains `due: Option<NoteDate>`, the one new field on an existing
  type. It is read from an `@YYYY-MM-DD` or `@YYYY-MM-DDTHH:MM` word on the
  line (the last such word, if several) and written back after the text and
  before the tags (`- [ ] text @2026-10-20 #prio`). Code that builds a
  `Task` with a struct literal must add the field.
- New `todo::parse_todo_lines`, returning `TodoLine` values that carry the
  due date. `parse_todos_from_content` keeps its tuple signature and output,
  with the `@date` word left in the text.

## [0.1.0] - 2026-10-15

First versioned release. Everything below shipped before versioning began.

### Storage and sync

- Two-axis scope model: personal or project, public or private, plus local
  scratch notes, stored as plain markdown in the notez vault.
- Global folder tree under `~/notez/`, with notes bound to no project.
- Project registry and per-project metadata; notez repos are registered
  automatically.
- Interactive sessions pull the vault when they open (NZ-1) and commit and
  push it when they end, committing pending changes before pull and push.
- Exit sync stays quiet when the remote is unreachable (NZ-6).
- Creating a note never overwrites an existing file (NZ-9), and a file that
  exists but cannot be read is never replaced (NZ-10).
- Names that sanitizing would alter are refused, and the todo store is
  protected from rename and delete (NZ-20, NZ-26).
- Importance tags stored per note in `.tags`.
- `notez migrate-from-legacy` imports notes and projects from the old
  notez-cli layout.

### CLI

- Bare `notez` opens the project view; scope flags (`-g`, `-p`, `-l`) turn
  the words that follow into a quick note (NZ-7).
- Public notes by default, `-p` for private, and `notez quick`.
- Commands: `add`, `log`, `logz`, `edit`, `nav`, `search`, `rename`,
  `mkdir`, `todo`, `init`, `attach`, `detach`, `sync`, `setup`, `list`,
  `completions`, plus a command picker.
- argv-0 aliases: `todoz`, `zlog`, `logz`, `zlogs`, `znote`, `treez`,
  `editz`, `findz`.
- Full-text search across notes and project docs.
- `install.sh` builds, installs, codesigns and links the aliases, and shows
  cargo's build progress and timing (NZ-35).
- CI builds and tests on ubuntu and macos (NZ-19).

### Tree browser

- Unified default view across scopes, with scope badges (NZ-13, NZ-21).
- Create (NZ-8), rename, delete (NZ-12) and move notes, and change their
  scope (NZ-15).
- Folders: list, create, rename, delete and move (NZ-14).
- Multi-select with `Space` for bulk delete, move and set scope (NZ-16).
- Shared context footer and per-view help overlay (NZ-2).
- Header line with sync state, dirty count and note counts (NZ-3).
- Resizable split, preview fold and pane focus (NZ-4).
- Reload on demand and when files change (NZ-34).
- `Esc` never quits the browser or the todo board (NZ-37).
- Tight left margin and real branch lines (NZ-31).
- Fixed three-column tag field, two tags per cell (NZ-38), and five fixed
  importance dot slots (NZ-40).
- Check-list icon for the todo store and `TODO.md` rows (NZ-24).
- Empty stores are listed.

### todoz

- Todo board across the global store and every project's `TODO.md`, with
  nested tasks, tri-state check boxes, drag and reorder, and quick-add.
- Layout matches the tree browser: cursor row, branch lines and margin
  (NZ-41).

### Preview

- Markdown rendering in the preview pane, toggled with `p` (NZ-25), including
  tables and footnotes (NZ-36).
- Syntax highlighting with tree-sitter for Rust, Python, Kotlin, Java, C,
  TOML, JSON, Bash and Markdown (NZ-27).
- Language suffix on the preview border; `p` toggles highlighting too
  (NZ-39).
- Scrolling with `Shift+Up`/`Shift+Down` and page keys (NZ-22).
- Lint diagnostics for syntax and markdown (NZ-28, first pass): the lint
  module landed but is not yet shown in the UI.

### notez-core

The public API that epoz builds on, as of this release. Later entries record
changes to it and to the on-disk formats.

- Scopes and notes: `Scope`, `Note`, `Project` (`core`), and scope
  resolution in `core::resolve`.
- Config and registry: `Config`, `ProjectRegistry`, `NotezMetadata`
  (`config`), read from the config and registry TOML and from
  `.notez-config.toml`.
- Aggregation: `core::aggregate::collect_all` and `collect_in_scope`,
  returning `NoteEntry` values.
- Todos: `todo::load_board`, `load_scope_board`, `save_todos_for` and the
  `Task` and `CheckState` model, over the `TODO.md` conventions.
- Tags: `tags` (inline flags) and `note_tags` (the `.tags` file).
- Search and filtering: `search::search_notes`, `filter`.
- Migration: `migrate::plan` and `migrate::apply`.
- Sync: `sync::pull_on_open`, `auto_sync`, `commit_pending` and the
  `AutoSync` result.

[Unreleased]: https://github.com/Gaurgle/notez/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Gaurgle/notez/releases/tag/v0.1.0
