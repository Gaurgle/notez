# notez design

A from-scratch rewrite of [notez-cli](https://github.com/Gaurgle/notez-cli) with cross-machine portability as a first-class concern. UX surface is preserved 1:1; only the storage layer is reworked.

## Goals

1. Same CLI and TUI surface as notez-cli. Same keybindings, same flags, same z-binary aliases (`todoz`, `zlog`, `znote`, `treez`, `logz`, `editz`, `findz`).
2. No OS symlinks. The current `~/notez/NN_<project>/` symlink scheme breaks when machines have different usernames or repo layouts.
3. Cross-machine sync of `~/notez/` works via plain `git pull/push`. No filesystem state assumes a specific home prefix.
4. Per-machine project registry is per-machine and never synced. Each machine learns where its own projects live.
5. No 100-project limit. No numbered directory allocation. Real names.
6. macOS and Linux first. Windows kept in mind but not blocking.

## Storage model

Four scopes. The TUI aggregates them into one view at runtime.

**Local** (`<project>/.notez/`, auto-gitignored on first write):
- Per-machine scratch. Truly private, never syncs anywhere.
- Falls back to `<cwd>/.notez/` even outside a git repo.
- For ephemeral notes that don't need to follow you.

**Personal** (`<notez_root>/personal/<project>/`, default scope):
- Your notes about this specific project, synced via your own notez remote.
- Invisible to teammates because they live in your `~/notez/` repo, not the
  project's repo.
- When you're inside a git project, this is the default for `notez add`.
- Outside a git project, falls back to global (no project subdir).

**Public** (`<project>/notez/`, committed):
- Public notes shared with the team via the project's git remote.
- Travels with the project automatically.

**Global** (`<notez_root>/`, has its own git remote):
- Cross-project notes, daily logs, todoz categories, scratch pad.
- Synced between machines via `notez sync` (commits pending changes, then `git pull --rebase && git push`).
- Contains a `.notez-config.toml` metadata file (synced) with project display
  names, descriptions, tags.

### Why personal exists

In notez-cli, private project notes lived in `<project>/.notez/` and were
mirrored into `~/notez/<project>/` via OS symlinks. The symlinks stored
absolute paths, which broke when usernames or repo layouts differed between
machines.

Personal scope solves the same problem from the other direction: instead of
trying to mirror project-local files into your synced home, the notes live
in your synced home from the start. The project repo never knows about them.
No symlinks, no encryption, no extra remotes to set up per project.

### Scope flags

Two axes: accessibility (personal vs public) x binding (project vs global),
plus the machine-only scratch tier. See "Two-axis scope language" below.

| Flag | Meaning | Where |
|---|---|---|
| _(default)_ | public + project (personal + global outside a project) | `<cwd>/notez/` |
| `-p` `--private` | personal + project | `<notez_root>/personal/<project>/` |
| `-g` `--global` | personal + global | `<notez_root>/` |
| `-l` `--local` | scratch (this machine) | `<cwd>/.notez/` |

`notez add` writes to the root of the resolved scope. Quick notes
(`notez quick`, or `notez add quick ...`) go to `00_quick-notes/` and are
private: the default public scope becomes personal; `-g` and `-l` apply as
usual.

## Config files

**Per-machine, not synced**: `$XDG_CONFIG_HOME/notez/config.toml`

```toml
[paths]
notez_root = "~/notez"             # global notes root, stored as tilde-relative
quick_notes_dir = "00_quick-notes"
daily_logs_dir = "01_daily-logs"

[editor]
command = "nvim"
new_note_args = ["+4", "-c", "startinsert"]

[tools]
fzf = true
rg = true
yazi = true
```

All paths are stored tilde-relative and expanded at runtime via `dirs::home_dir()`. No absolute paths persist to disk in the config.

**Per-machine, not synced**: `$XDG_CONFIG_HOME/notez/registry.toml`

```toml
[projects.app2]
local_path = "~/repos/sigma/App2"

[projects.notez-cli]
local_path = "~/Repos/notez-cli"
```

This is the per-machine equivalent of the old `ProjectMapping`. It maps a project name (key) to its location on this machine. The path is stored tilde-relative; absolute resolution happens at every load.

**Synced, lives in ~/notez/**: `~/notez/.notez-config.toml`

```toml
[projects.app2]
display_name = "App2 (Android BLE testing)"
tags = ["sigma", "lia"]
order = 2

[projects.notez-cli]
display_name = "notez"
tags = ["personal"]
order = 4
```

This synced file holds project metadata that should be the same across machines: display names, tags, sort order. The per-machine `registry.toml` decides where the project actually lives on this specific machine. Lookups merge metadata with per-machine paths.

## Project discovery

`notez attach <name> [path]` registers a project on the current machine:
- If `name` is omitted, derives from current dir's git toplevel
- If `path` is omitted, uses current dir
- Stores tilde-relative path in `registry.toml`
- If `~/notez/.notez-config.toml` does not already have an entry for `<name>`, prompts for display name and tags, then writes it

`notez detach <name>` removes the registry entry; does not touch the project's notes.

`notez attach --scan` walks `~/repos/`, `~/Repos/`, and other configurable roots, finding directories with `.notez/` or `notez/` subdirs, and prompts the user to attach each one.

## TUI aggregation

When the user opens the global tree (`notez -g tree`) or global todoz (`todoz -g`), the TUI:

1. Loads `registry.toml` to get the local paths for each project
2. For each registered project, scans `<path>/.notez/` and `<path>/notez/`
3. Loads `~/notez/.notez-config.toml` for project metadata (display name, ordering)
4. Loads `~/notez/` itself for global notes (quick notes, daily logs, `_todos/<category>/TODO.md`)
5. Builds the tree as before, but with no filesystem symlinks involved

A project whose `local_path` does not exist on this machine renders as dimmed with a "(not attached)" marker, so the user knows the metadata exists but the source is unreachable. Notes for that project are not shown.

### CLI and desktop read the same board

The global todo board (GLOBAL → `_todos/<category>` → each project's personal/public/local) is built by a single function, `notez_core::todo::load_board`. The TUI (`todoz -g`) and the desktop app's `load_todo_board` command both call it, so `_todos/<category>/TODO.md` categories appear identically in the CLI and the GUI. Legacy `_todos/` boards carried over from notez-cli need no migration: `migrate.rs` treats `_todos` as a special dir and leaves it untouched, and `load_board` always shows it.

## Sync

`notez sync` is a thin wrapper:

1. Validates that `~/notez/` is a git repo with a remote
2. Commits any pending changes (`git add -A`, message `notes: sync <date time>`), since `pull --rebase` refuses a dirty tree
3. Runs `git pull --rebase`
4. Runs `git push`
5. On conflict, surfaces the git output and tells the user to resolve manually

The first-time setup walks the user through `git init && git remote add origin ...` if `~/notez/` does not have a remote yet.

## Tag system

Same as notez-cli:
- Five colored flags: important, prio, longterm, idea, blocked
- Persisted inline in TODO.md as `#important #prio #longterm #idea #blocked` suffixes
- Tree browser persists in `.tags` files (one per notes root), format `<relpath>:<flagbyte>` per line
- Filter syntax `#tagname` with prefix matching, `#13` for tags 1+3, AND across tokens, OR within

## Migration from notez-cli

**Implemented** (`notez migrate-from-legacy`, with `--dry-run`; logic in
`notez-core/src/migrate.rs`, also reachable from epoz's MigrationDialog). It
reads the legacy `~/.config/notez/projects` (`name=path`) and, for every
`~/notez/NN_<name>` dir whose stripped name matches a legacy project:

1. Moves the dir to `<notez_root>/personal/<name>/` and attaches the project
   to the per-machine `registry.toml`. Existing destinations are merged
   entry-by-entry; collisions are never overwritten, only reported.
2. Materializes the legacy symlink mirrors: links into a repo's `.notez/`
   (private store) are replaced by the real file, moved here - personal scope
   owns it now. Links into a repo's `notez/` (public store) are just dropped;
   those files already sit exactly where notez's public scope wants them.
   Dangling links are pruned.

Global dirs (quick-notes, daily-logs, `_todos`) and unmatched dirs are left
untouched. `config.toml` and `.notez-config.toml` are not generated - write
those once by hand or via `notez setup`.

Executed for real on 2026-07-05 (this machine): 11 project dirs migrated, the
duplicate/empty numbered dirs removed, `~/notes` (the pre-notez knowledge
base) consolidated into `~/notez/reference/`, and the legacy binaries in
`~/.local/bin` replaced by notez builds. `~/notez/` now contains zero
symlinks and no numbered project dirs.

## Open questions and future work

### Calendar / deadlines for notes and todos (future)

Idea: surface a calendar dimension across notez and todoz. Each note/todo
could carry a **created date** (already implicit in filenames/mtime) and,
more importantly, an optional **deadline and/or event date**. A calendar
view would then link documents to dates - see what's due, what was created
when, and what events are coming up - across scopes and projects.

**Surface / home for it:** the todoz **preview pane** (the selected-todo
detail card added in the Melt UI refresh) is the natural place to set and
show a todo's date. The right pane could hold a date field + a mini-calendar
to assign a deadline/event to the selected todo, and a fuller calendar view
could aggregate dated todos/notes across sections.

Open questions when this is picked up:
- Where does the date live? Frontmatter in the `.md` / inline metadata on a
  todo line (e.g. a `@2026-07-01` token) vs. a sidecar index. **This is the
  crux** - TODO.md round-trips byte-for-byte through `notez`/`todoz`/nvim, so
  the encoding must survive the CLI untouched (likely an inline `@date` token
  parsed by notez-core, mirroring how `#tags` already work).
- Deadline vs. event vs. created-date as distinct fields.
- Which UI library - Melt UI's `melt` package has **no** Calendar builder
  yet (only the legacy `@melt-ui/svelte` does), so this is a hand-roll or a
  legacy-pkg dependency when the time comes.
- Interaction with todoz tags (`#blocked`, `#longterm`) and sorting.

**TODO / next steps** (a `Calendar.svelte` placeholder now renders in the
todoz preview pane, so this is moving from idea to in-progress):

- [ ] Pick the date encoding: an inline `@YYYY-MM-DD` token on the todo line,
  parsed by notez-core next to `#tags`, so `TODO.md` still round-trips through
  the CLI untouched. This is the gating decision.
- [ ] Parse / serialize `@date` tokens in notez-core's todo model; expose
  `due` / `event` fields on the task DTO.
- [x] `Calendar.svelte` is now reusable/context-driven (`marked` days, parent
  `selected` set, `onPick`, `onClear`, `label`) so each view drives it.
- [x] Toggleable calendar pane in **todoz** and **notes** (footer indicator +
  `c` key; resizable). Dashboard keeps its own inline month widget.
- [x] Notes: real days-with-notes are lit (from mtimes); **multi-day selection
  filters the list**, with a clear button.
- [ ] Tickets: calendar pane once tickets carry due dates / milestones.
- [x] **Draggable start/end date range** in the calendar - drag across days to
  select a span (live preview while dragging, commits the whole range on
  release; plain click still toggles one day). Wired into notes' day filter via
  an `onRange` callback. Single source: `Calendar.svelte`.
- [ ] The real date model: `@date` for todos, frontmatter/mtime for notes, so
  selections round-trip through the CLI. Calendar is navigation-only until then.
- [ ] A fuller calendar view aggregating dated todos across sections and scopes.
- [ ] Date-based sorting and filtering; interplay with `#blocked` / `#longterm`.

### Real GitHub org data (done, via the gh CLI)

A backend module (`app/src-tauri/src/github.rs`) shells out to the
already-authenticated **`gh` CLI** (no HTTP client, no token handling, no
device-flow build-out) and exposes `github_user/repos/commits/issues/
contributors/create_issue` over IPC (`ipc.ts`). The default org is `airwavez`.
`gh` is resolved from `PATH`, falling back to `/opt/homebrew/bin/gh` for the
Finder-launch case. This is the pragmatic stand-in until/if a real device-flow
auth lands; it already gives every view real identity + org data.

- [x] Dashboard weather is **live** via Open-Meteo (no API key), location from
  browser geolocation → IP lookup → Stockholm default, with animated
  sun/cloud/rain/snow/fog/storm scenes, humidity, and wind. Falls back to mock
  if offline.
- [x] **Dashboard is live**: real repos, recent-commit feed, **git-activity
  heatmap + streak built from real commit dates**, contributors with real
  avatars, an open-issues list, and note/todo/issue/commit stat tiles.
- [x] **Ticketz reads real issues**: airwavez issues mapped to lanes (closed =
  Done; review / in-progress labels drive the middle lanes; else Backlog), real
  assignees/labels/points/markdown bodies; New creates a real issue via `gh`.
- [x] **Spaze**: rooms are the org repos, member counts from real contributors,
  composer posts as the real GitHub identity (login + avatar).
- [x] **Notes/Todoz**: airwavez repos attached to the per-machine registry
  (`~/.config/notez/registry.toml`) so their notes aggregate in the desktop app.
- [ ] **Avatars on commits/issues**: commit authors already carry a real avatar
  URL; issue authors/assignees don't yet (the issues DTO leaves `avatar_url`
  null) - fetch them so ticketz/inspector show real faces.
- [x] **All repos across owners, not one org.** `github_all_repos` lists every
  reachable repo (owned / collaborator / org member) via `/user/repos`; a shared
  `repoStore` holds the global active-repo selection, grouped by owner with
  collapsible categories, a filter, and a 6-month archive threshold (dormant
  repos hidden behind a toggle). `Ctrl+1-6` jumps between views.
- [x] **Auto-generated backlog.** Seeded ~68 story-pointed GitHub issues across
  11 active repos (airwavez x3, notez, spaze, repoz, epoz, noiz-kmp, glanze,
  clockz, bonsai-education) from each repo's README/design-docs/git-history -
  backlog + in-progress + closed "done" waypoints, points via `sp:N` labels.
- [ ] **bonsai-education/content + /socials backlog (blocked).** The `Gaurgle`
  account has only **read** access on these two org repos, so labels/issues
  couldn't be written. Grounded backlogs are prepared; needs Triage/Write to
  create. (`bonsai-education/bonsai-education` got issues but no `sp:` labels for
  the same reason - points show as default there.)

### Desktop dashboard + sync - open feedback (next)

- [x] **CLI → desktop sync.** Todos polls the board from disk every 3s while the
  tab is open (no focus guard, so it works side-by-side with a terminal). The
  reload used to fold the tree back up - fixed with `preserve_collapsed` in the
  backend (carry expand/collapse across the disk reload, keyed by header
  identity). Skipped during edits/dialogs. _Follow-up: proper fs-watch (notify →
  emit event) to replace polling; and the TUI doesn't auto-refresh on desktop
  edits - that's a notez-cli change._
- [x] **Ticketz: drag cards between columns** (Backlog / In progress / Review /
  Done) - pointer-based kanban drag-and-drop (WKWebView won't fire HTML5 DnD),
  with reorder-within-lane and a floating ghost.
- [ ] **Ticketz: GitHub write-back for moves/edits.** Drag-to-organize and the
  edit pane are local/session-only right now. Persist them to GitHub: moving to
  Done closes the issue (reopen on the way out); moving between open lanes sets a
  `status:<lane>` label (auto-create it); title/body/assignee/points edits map to
  `gh issue edit`. Re-fetch after each write so the board reflects the source of
  truth. Needs a `github_move_issue` / `github_edit_issue` backend pair.
- [x] **Widget resize handles** - thinned to 5px rounded corner dots + 2px edge
  pills, hidden until hover with a delayed fade-in.
- [x] **Recent-commits widget → repoz CLI style**, now on **real commits**:
  per-repo `name ~/path ···· N open` header, one full-width row per commit
  (`hash  message ···· relative-time  avatar`), monospace, dotted leaders. Churn
  (+adds/−dels) was dropped with the mock data - the commits API doesn't return
  per-commit stats without N extra calls; a follow-up could fetch churn for just
  the visible rows or read it from the local clone.
- [ ] **repoz: alternative layout for multiple selected repos.** The current
  full-width single-column listing reads great for **one** repo, but when
  several projects are selected the stacked per-repo blocks get long and the
  `behind`/path headers compete for attention. Need an alternative way to show
  repoz across multiple repos (e.g. collapsible per-repo sections, a compact
  summary row per repo with expand-on-demand, or side-by-side columns again for
  the multi-repo case). Single-repo view stays as-is.

Captured here so it isn't lost.

### Refactor: a shared view shell (tech debt, do later)

The three working views - `NotesView`, `TodozView`, `TicketzView` - each
**hand-roll the same chrome**, and it has drifted. This isn't urgent, but it
should be consolidated before adding more views.

What's duplicated across the views (with subtle, accidental differences):

- **Sidebar**: brand (`MachineAvatar` + name), `group` / `group-label` / `item`
  / `count`, the project list, and ~60 lines of identical CSS - copy-pasted into
  each view (and into the Dashboard sidebar). Notes uses the standalone
  `Sidebar.svelte`; todoz/ticketz/dashboard re-implement it inline.
- **Footer / pane toggles**: each view rebuilds the statusbar + `pane-toggle`
  buttons. Ticketz had even diverged with a **local `.pane-toggle`** (purple
  background) that broke parity with notes - only caught by eye. The shared
  `.pane-toggle` lives in `app.css`; views should not redefine it.
- **Resizable panes**: every view repeats `Resizer` + a `*Width` `$state` +
  the `{#if showX}` pane wiring, with per-view min/max drift.
- **Pane toggles + keybinds**: the `p` / `c` / `i` / `e` handlers and the
  show-state booleans are re-written per view (todoz/notes via melt `Toggle`,
  ticketz via a hand-rolled keydown effect).
- **Inspector / Calendar**: sometimes the shared `Inspector.svelte` /
  `Calendar.svelte` components, sometimes inline markup (ticketz inspector is
  inline). Same for the calendar pane wrapper.

Proposed shape:

- A `<ViewShell>` that owns the sidebar slot, the main slot, the right-pane
  stack, the footer with toggle indicators, and the pane show-state + keybinds +
  persisted widths. Views provide content via slots/props.
- One `Sidebar` (brand + groups + items) reused everywhere (incl. Dashboard).
- Move the sidebar / footer / pane-toggle / item CSS into `app.css` (or the
  shared components) as the single source of truth; delete the per-view copies.
- Standardize on `Inspector.svelte` + `Calendar.svelte` in every view.
- A tiny `usePanes()` rune for `{showPreview, showInspector, showCalendar,
  showEdit}` + keybinds + width persistence.

Goal: a new view is "sidebar content + main content + which panes," not a
re-implementation of the chrome.

### Scope migration (move notes/todos between scopes)

Sometimes a note that started as personal should become public, or a local
scratch should be lifted to personal so it follows you between machines.
Two operations, two complexity profiles:

**Note files**: a single `.md` file moves between scope directories.
Straightforward `notez mv <pattern> --to <scope>` command. Side effects to
think through:

- `personal -> public`: the file now lives in the project repo, will be
  committed next time the project is committed. Visible to teammates from
  that commit onward.
- `public -> personal`: the file leaves the project repo, but its history
  is still there in past commits. Removing from history is a destructive
  rewrite; offer `--rm-from-history` flag but only with strong warnings.
- `personal -> global`: removes the project association, lands in the
  user's cross-project notes.
- `* -> local`: file becomes per-machine again, stops syncing entirely.

**Todo entries**: each scope has its own `TODO.md`, with many entries per
file. Moving a single entry between scopes is fundamentally different from
moving a file - it is an edit operation, not a rename. Best done from
inside the todoz TUI:

- Press `m` on a selected todo: prompt "Move to: local / personal / public
  / global / cancel"
- Implementation: remove the line(s) from source TODO.md (subtree if it has
  subtasks), append to target TODO.md, preserve tags and indent depth

Both deferred until the TUI lands. The note-level `notez mv` could come
sooner if needed; the todo-level move only makes sense once todoz exists.

### TUI interactions for moving

The TUI should support both keyboard and mouse for the same operation, so
users can pick whichever fits their flow:

**Keyboard** (`m` keybind): opens a status-bar prompt
```
Move to: [l]ocal · [p]ersonal · [u]blic · [g]lobal · [esc] cancel
```
Single keystroke commits; cancel returns to navigation. Works in both
the tree browser and todoz, on the currently selected row.

**Mouse drag** (todoz, global view only): drag-and-drop already exists for
reorder. Extending it across section boundaries triggers a scope move
instead of a reorder when:

- The drop target is in a different section than the source
- That section corresponds to a different scope (local/personal/public/global)
  or a different project entirely

Visual cue: target section header highlights while dragging across it.
Cancel by dropping back into the source section.

For the tree browser, drag-and-drop across deeply-nested directory boundaries
is fragile. The tree TUI sticks to the keyboard `m` keybind only.

The two paths converge on the same internal move() function so behavior is
identical regardless of input modality.

### Authorship + activity, via GitHub identity (future)

Once the desktop app authenticates with GitHub, every note and todo can carry
**who** touched it and **when** - surfaced on the row as a small author avatar
and a relative time ("2d ago"). A mock version of this already renders in the
todoz rows (deterministic placeholder data) to evaluate the layout.

- Identity comes from GitHub auth; the avatar/initial and name follow the
  GitHub user. Long term this generalizes to other forges (GitLab, etc.).
- The "when" reuses the date model from the calendar work above (inline
  `@date`-style tokens that survive the CLI round-trip), plus git history as a
  secondary source of truth for created/last-touched.
- This is what makes a **richer task lifecycle** meaningful: beyond the current
  tri-state checkbox (`[ ]` / `[/]` / `[x]`), a todo could be **done**,
  **deferred**, or **scrapped/won't-do**, with the row showing the state + who
  set it + when. Encoding must still round-trip byte-for-byte through `todoz`
  (likely an inline status token next to `#tags` / `@date`).

### A repo-bound ticket / kanban layer (big-picture, future)

The natural next layer is a **Trello-style ticket board** bound to the
project/repo - tickets with status columns, assignees (GitHub users), and links
to the notes and todos that already live with the repo. This ties the pieces
into one workspace:

- **Git authorizes everything.** GitHub identity = who can see/edit; the repo =
  the boundary. Notes, todoz, tickets (and chat, below) all scope to the repo.
- **Everything syncs the same way: through git.** Tickets persist as files in
  the repo (e.g. markdown + frontmatter, or a structured dir), so they sync,
  diff, and merge with the same byte-for-byte CLI-compatible discipline as
  notes/todoz. No separate backend; GitHub is the sync substrate.
- **`spaze` (the CLI chat tool) folds in** as the conversation layer - chat
  threads attachable to a ticket / repo, same identity, same git-backed sync.

Design principle for all of the above: **it must always work and always sync**,
which means every new artifact is a plain file in the repo with an encoding the
CLI tools can read and write losslessly. Build the data model first (round-trip
tests), UI second. Captured here as direction, not committed scope.

**Status:** both desktop views are now backed by **real org data via `gh`**
(see "Real GitHub org data" above). **Ticketz** (`TicketzView.svelte`) reads real
issues; **Spaze** (`SpazeView.svelte`, modelled on the `spaze` TUI: rooms
sidebar, `<name>` timeline, inline `#note`/`#todo` capture) draws real rooms,
members, and identity. The git-backed file-per-ticket data model is still the
long-term target; GitHub Issues is the real backing in the meantime. The rail is
Notes / Todos / Tickets / Spaze.

**Decision (2026-07-01): keep GitHub Issues as the ticket storage.** Tickets
stay backed by real GitHub Issues (read/written via `gh`) rather than building
the file-per-ticket model now. Rationale: Issues already sync through GitHub,
carry identity and assignees for free, and the board already reads them live, so
this keeps the "always works, always syncs" principle without new infrastructure.
The git-backed file-per-ticket model remains the long-term target but is
deferred; revisit if Issues become limiting (offline-first editing, custom
fields, or lossless round-trip with the CLI tools).

### Two-axis scope language (decided 2026-07-06)

Scopes are presented as two axes, not four silos:

- **Accessibility**: personal (private to the user, syncs via their own
  notez repo) vs public (committed in the project repo, shared with
  collaborators).
- **Binding**: project (attached to a repo) vs global (the `~/notez` repo
  itself, for notes bound to no project).

Valid combos and their storage (unchanged): personal+project =
`<notez_root>/personal/<project>/`; public+project = `<repo>/notez/`;
personal+global = `<notez_root>/`. public+global does not exist (the notez
repo is private). A fourth tier is kept by explicit decision: **scratch**
(`<repo>/.notez/`, gitignored, this machine only, never syncs), renamed
from the confusing "local" in every UI; the CLI `-l` flag and the on-disk
layout and wire values (`local`) are unchanged. This is display language
only; the `Scope` enum and serde stay as they were. Attaching a project now
also scaffolds `<path>/notez/` so the public store exists from day one.

### Search + project docs + safe saves (landed 2026-07-06)

- **Full-text search** (`notez-core/src/search.rs`): rides on
  `core::aggregate::collect_all`, so the search universe is exactly what the
  UIs show; case-insensitive substring scan, one hit per file (first line +
  match count). Consumed by `notez search`/`findz` and epoz's Notes search.
  No index: the whole corpus is a few MB, scanning is milliseconds.
- **Project docs as a source**: each registered project's `docs/*.md` is
  aggregated as `SourceKind::Doc` (Public scope, labeled `docs`), so repo
  documentation like `airwavez/docs/hardware.md` is findable from the same
  place as notes. Config: `[paths] project_docs` (default true).
- **Dirty-source saves**: `todo::save_todos_for(items, sources)` writes only
  the given files; the todoz TUI tracks dirty sources per mutation, so a
  no-edit quit writes nothing. Root cause: the serializer is canonical
  (checkbox-only) and used to rewrite every file on quit, silently dropping
  prose from `TODO.md`s. A footer warning names sections whose file contains
  non-todo text before a save would drop it. The desktop app should migrate
  to the same call (its wholesale `save_all_todos` has the identical hazard).

### Naming and companion tools (decided 2026-07-05, refined 2026-07-06)

**epoz is the product; it has two shells.** The Tauri app in `app/` is
**Epoz Desktop**. **epoz (TUI)** is a planned umbrella terminal app with the
same rail (Home / Notes / Todos / Tickets / Spaze), composing the module
TUIs rather than reimplementing them. Every module also ships standalone:

| Module | Standalone TUI | Status |
|---|---|---|
| Todos | `todoz` | shipped 2026-07-05 |
| Notes | `notez tree` | next: last unported legacy surface |
| Dashboard | `fleetz` | exists; realtime + worktrees roadmap in its README |
| Spaze | `spaze` | exists (was TUI-first) |
| Tickets | `ticketz` | to build: ratatui kanban over `gh`, reusing the desktop's lane mapping |

Build order toward the umbrella: 1) `notez tree` port, 2) `ticketz` TUI,
3) extract the fleetz shared core (also unblocks the repoz TUI), 4) the
epoz umbrella shell that mounts the module TUIs as library crates. Until 4
lands, a tmux/zellij layout preset over the standalone TUIs is the
zero-code stand-in. notez stays the CLI/core (notez-core is the data
layer under all of it).

This also resolves the earlier "repoz big brother" naming confusion: the
standalone ratatui multi-repo dashboard TUI that used to hold the epoz name
was renamed **fleetz** and lives on as a repoz sibling (and becomes the
umbrella's Dashboard module).

The sibling CLI tools share the same git-backed, GitHub-identity discipline:

- **`fleetz`** (formerly the epoz TUI) - live multi-repo dashboard: CI, PRs,
  ahead/behind, worktrees, stashes. Roadmap: true realtime refresh and
  first-class worktree operations (see its README).
- **`repoz`** - the snapshot repo-status command; a repoz-owned TUI is wanted,
  sharing a data layer with fleetz rather than duplicating it.

Their capabilities (repo fleet status, CI/PR state, worktrees) are also
roadmap items for epoz's Dashboard view, surfaced through the same desktop
shell rather than reimplemented ad hoc.

## Test scenarios

A representative matrix to validate behavior end-to-end. These are not unit
tests; they exercise multi-machine, multi-user, multi-project realities the
storage layer has to handle. Each scenario lists the setup and the
properties that must hold.

### Cast

- **Alice**: works on two machines, **laptop** (username `alice`, home
  `/Users/alice`) and **desktop** (username `alice-desk`, home
  `/Users/alice-desk`). Has her own `~/notez/` repo on GitHub used as a
  private notez remote.
- **Bob**: works on a single machine **bobpc** (username `bob`). Has his
  own local `~/notez/` repo with no remote.
- **Carol** and **Dave**: each on one machine. Collaborate on a repo
  Alice and Bob are not part of.

### Repos

- `shared-repo-1` and `shared-repo-2`: GitHub repos shared between Alice
  and Bob (Carol and Dave have no access).
- `cd-repo`: GitHub repo shared between Carol and Dave (Alice and Bob have
  no access).

### Scenario A. Alice writes a personal note about shared-repo-1 on laptop

Setup: Alice has `shared-repo-1` cloned at `/Users/alice/repos/shared-repo-1`
on laptop, and at `/Users/alice-desk/Repos/shared-repo-1` (capital R) on
desktop. She has run `notez attach` inside the project on each machine.

Action: on laptop, `cd ~/repos/shared-repo-1 && notez -p add "API redesign idea"`

Verify:
- File lands at `/Users/alice/notez/personal/shared-repo-1/<date>-api-redesign-idea.md`.
- `~/notez/personal/shared-repo-1/` is tracked in Alice's notez repo.
- After `notez sync` on laptop and again on desktop, the file appears at
  `/Users/alice-desk/notez/personal/shared-repo-1/...` (note the different
  home path).
- The file is not visible inside `shared-repo-1`'s git tree on either
  machine, so Bob cannot see it via the project remote.

### Scenario B. Bob pushes a public note in shared-repo-1

Action: Bob runs `notez add "deploy steps"` inside `shared-repo-1`,
commits the new `notez/<date>-deploy-steps.md`, pushes.

Verify:
- Alice pulls the project on laptop and sees the file in
  `~/repos/shared-repo-1/notez/...`.
- `notez tree` on Alice's laptop shows the note under the Public scope for
  `shared-repo-1`, with the team-globe icon.
- Bob's same note is visible on Alice's desktop after she pulls there too.
- The file is not touched by `notez sync`; it travels via the project's
  git remote, not Alice's notez remote.

### Scenario C. Alice writes a local scratch note

Action: Alice on laptop runs `notez -l add "try this branch out"`.

Verify:
- File lands at `/Users/alice/repos/shared-repo-1/.notez/...`.
- The `.notez/` directory is gitignored (notez ensures this on first write).
- After `notez sync`: still only on laptop. Not synced to desktop, not
  visible to Bob.
- After `git pull` on desktop: not present (gitignored).

### Scenario D. Bob and Alice both have personal notes for the same project

Setup: both Bob and Alice have attached `shared-repo-1`. Each has their
own `~/notez/personal/shared-repo-1/` directory in their own notez remote.

Verify:
- Alice's `notez tree` shows her personal notes only.
- Bob's `notez tree` shows Bob's personal notes only.
- Neither sees the other's personal notes anywhere.
- Both can see the same public notes (Scenario B).

### Scenario E. Alice's two machines have shared-repo-1 at different paths

Setup: on laptop, registry has `shared-repo-1 = "~/repos/shared-repo-1"`.
On desktop, registry has `shared-repo-1 = "~/Repos/shared-repo-1"`
(capital R).

Verify:
- Registry differs between machines (it's per-machine and not synced).
- Personal notes resolve correctly on each machine via tilde expansion
  against the local home, producing different absolute paths but the
  same project name.
- No symlink ever points at a hardcoded absolute path.

### Scenario F. Carol and Dave's project is invisible to Alice and Bob

Setup: Carol and Dave each have `cd-repo` attached on their machines.
Their `~/notez/` is sync'd between them. Alice has not cloned `cd-repo`.

Verify:
- Alice's `notez list` does not show `cd-repo`.
- Alice's `notez tree` does not show notes from `cd-repo`.
- Carol's `notez tree` shows her personal + public + local notes for `cd-repo`.
- If Alice clones `cd-repo` and runs `notez attach`, it appears in her
  registry. The public notes from Carol show up immediately. Carol's
  personal notes do NOT appear (those live in Carol's notez remote, not
  the project remote).

### Scenario G. Carol shares cd-repo with Alice later

Setup: Carol invites Alice to `cd-repo`. Alice clones it and runs
`notez attach` to register it.

Verify:
- Alice sees all public notes Carol committed.
- Alice does not see any of Carol's personal notes (those are in Carol's
  own `~/notez/`, which Alice has no access to).
- Alice can write her own personal notes about `cd-repo`. These land in
  `~/notez/personal/cd-repo/` on Alice's machine and sync via Alice's
  notez remote.
- Carol cannot see Alice's personal notes about `cd-repo`.

### Scenario H. Alice deletes a personal note on laptop, syncs

Action: Alice deletes `~/notez/personal/shared-repo-1/<file>.md` on laptop
and runs `notez sync` (which commits and pushes the deletion).

Verify:
- After `notez sync` on desktop, the file is gone there too.
- Alice's project repo is not affected (the note was never in it).
- Bob is unaffected (it was never in his clone or notez remote).

### Scenario I. Project moved on one machine

Action: Alice moves `~/repos/shared-repo-1/` to `~/Code/work/shared-repo-1/`
on laptop. The registry still points at the old path.

Verify:
- `notez tree` warns about the missing project but does not crash.
- `notez attach --path ~/Code/work/shared-repo-1 shared-repo-1` updates
  the registry to the new path.
- After that, everything resolves correctly again.
- Other machines are unaffected (their registry is independent).

### Scenario J. Conflict during notez sync

Action: Alice writes a personal note on laptop and on desktop without
syncing in between, then runs `notez sync` on both.

Verify:
- First machine's push succeeds.
- Second machine's `git pull --rebase` surfaces a conflict.
- `notez sync` does not silently lose data; it tells the user to resolve
  the conflict manually in the notez repo and rerun.
- After manual resolution, both notes coexist.

### Scenario K. Public note moves to personal (future `notez mv`)

Action: Alice realizes a public note in `shared-repo-1` should not have
been shared. She runs `notez mv "leaked-thoughts" --to personal`.

Verify:
- File leaves `~/repos/shared-repo-1/notez/`.
- File arrives at `~/notez/personal/shared-repo-1/00_quick-notes/`.
- The history in `shared-repo-1`'s git is unchanged (Alice has to
  separately rewrite that history if she wants the leak removed from
  past commits; notez does not do this automatically).
- After `notez sync` + a commit in `shared-repo-1`, the file is gone from
  the public scope and synced via Alice's personal remote.

These scenarios are the acceptance criteria for the storage layer. Each
should eventually have an integration test (probably under `tests/`)
that spins up tempdir fixtures simulating two machines and two users.

## Status

This document describes the target design. Initial implementation focuses on:

1. Cargo.toml and module skeleton
2. Config and registry types with TOML round-trip tests
3. Core abstractions: Scope, Project, NoteSource
4. Commands: `add`, `log`, `mkdir`, `attach`, `detach`, `list` end-to-end
5. Stubs for `tree`, `todo`, `edit`, `search`, `nav`, `sync` (return not-implemented)
6. Panic-safe TUI entry/leave helpers (carried from notez-cli)

Progress since: the storage layer, migration, desktop app (Epoz Desktop),
todoz TUI (2026-07-05, dirty-source saves), full-text search, project docs
sources, and the tree-browser TUI (2026-07-06, dirty-only `.tags` writes)
are done. Every legacy notez-cli surface is now ported; `edit` and `nav`
remain stubs. Next per the umbrella plan: ticketz TUI, fleetz-core, the
epoz umbrella shell.
