# notez

A local-first note-taking tool with a CLI/TUI **and** a native desktop app. Cross-machine portable rewrite of [notez-cli](https://github.com/Gaurgle/notez-cli) (now deprecated).

**Naming:** `notez` is the CLI and core (`crates/`); the desktop app in `app/` is **epoz**. Same engine, two surfaces. (The epoz name previously belonged to a standalone repo-dashboard TUI, which lives on as **fleetz**.)

## What it is

- Two axes, not four silos: **accessibility** (personal vs public) x **binding** (project vs global). Default = public+project: committed with the repo, shared with collaborators (outside a repo it falls back to `~/notez`). `-p` = personal+project: private notes in your own notez repo, synced across your machines, never touching the project repo. `notez add` writes to the scope root; `notez quick` (or `notez add quick`) writes a private quick note to `00_quick-notes/`. `-g` = personal+global: the `~/notez` repo itself, for notes bound to no project. `-l` = scratch: gitignored, this machine only.
- One model that surfaces everything from the CLI or the desktop app: local scratch, personal-per-project, public-with-team, and global cross-project notes.
- A full todoz todo manager: tags, subtasks, drag-to-reorder.
- Cross-machine portable. No OS symlinks. No absolute paths persisted. Per-machine project registry; private notes stay private by living in your own repo.

## Repository layout

A Cargo workspace plus a Tauri/Svelte frontend:

```
crates/notez-core/   # scopes, aggregation, todoz model, tags (GUI-agnostic)
crates/notez-cli/    # the `notez` binary (+ todoz/zlog symlink dispatch)
app/                 # epoz, the desktop app: Tauri (Rust) + SvelteKit frontend
```

## Desktop app (epoz)

A native desktop app (Tauri + SvelteKit + TypeScript, CodeMirror 6 editor) that reads and writes the same files as the CLI, byte-for-byte, so notes round-trip through `notez`, `nvim`, and the GUI without spurious diffs.

**Notes**
- Sidebar of scopes (Personal / Public / notez (global) / Scratch / Docs), the global notez folder tree, and registered projects; filter by one scope, folder, **or** project, or view everything.
- Note list with importance-tag dots and a scope/project pill.
- Markdown editor with optional **vim mode** (toggle with the footer pill or `Ctrl+;`, with a NORMAL/INSERT/VISUAL badge) and a separate, resizable, togglable **live preview** pane.
- Right-side inspector (scope, project, path, tags) and a status-bar footer.
- Sort by latest touch / oldest / name, across any scope or project.
- Importance tags (`1`-`5`), vim + arrow-key navigation, `/` and `Cmd/Ctrl+F` to search.

**Todoz**
- Interactive tree todo board: tri-state checkboxes, 5 importance tags, subtasks, sections, drag-to-reorder.
- Full keyboard control mirroring the todoz TUI, `#tag` / `#1` filtering.

All panes are resizable; preview and inspector are independently togglable (`p` / `i`).

### Run / build the app

```bash
cd app
npm install
npm run tauri dev      # dev build with hot reload
npm run tauri build    # production bundle
```

## CLI

The CLI surface mirrors notez-cli's. Working today:

```
notez add        notez quick       notez log
notez mkdir
notez attach     notez detach      notez list
notez sync       notez setup       notez completions
notez init       notez --help      notez migrate-from-legacy
notez search     todoz             todoz -g
notez tree       notez -g tree     notez edit
notez nav        notez logz        notez logs
```

**todoz** is the full interactive board TUI (tags, subtasks, drag-to-reorder,
`#tag` filtering, code-TODO scanning, mouse support); `todoz "item"` quick-adds
without opening it. Saves rewrite only the files you actually edited, and the
footer warns if a `TODO.md` contains non-todo text a save would drop.

**notez search** (alias `findz`) is case-insensitive full-text search across
every note source: global root, every registered project's scopes, and each
project's `docs/*.md` (surfaced as `docs`; disable with `project_docs = false`
under `[paths]` in config.toml).

**notez tree** is the interactive tree browser (sections per scope and
project including `docs`, tag strip + `#tag` filtering, preview pane,
open-in-editor). Tag changes write only `.tags` roots that actually changed.

**notez add** writes a public note to the repo's `notez/` (`-p` for a
private one under `~/notez/personal/<project>/`). **notez quick** (or
`notez add quick ...`) writes a private quick note to `00_quick-notes/`.

`notez add --in <dir>` targets a subdirectory (global root by default,
the current scope's root with `--in-local`); bare `--in` opens an fzf
picker. Scratch writes (`-l`) auto-gitignore `.notez/` in the repo.

**notez edit [term]** (alias `editz`) opens an existing note. Candidates come
from the scope model, so it sees exactly the notes the rest of the tool
considers in scope; a term matching one note skips the picker.

**notez nav** picks a directory in the vault and opens it, with `personal/`
expanded one level so every project is one hop away.

**notez logz** / `logs` / `zlogs` opens the daily-logs directory for the
current scope, creating it on first use.

Still stubbed (exits 1 with a clear message): `demo`, a screenshot helper
from the legacy CLI, not ported by decision.

`notez migrate-from-legacy` (with `--dry-run`) is the one-time port of a
notez-cli layout: numbered `NN_project` mirror dirs move to `personal/<project>/`,
repo-private symlink targets are materialized as real files, public-store links
are dropped (those files already live in the repo), and the projects are
attached to the per-machine registry.

```bash
./install.sh            # release build, ~/.local/bin/notez, codesign, alias symlinks
notez --help
```

Run `install.sh` on every machine after pulling. The vault layout assumes both
machines run the same binary; see `docs/handover-2026-09-02-small-machine.md`.

See [DESIGN.md](DESIGN.md) for the architecture, scope model, and test-scenario matrix. Core logic is covered by unit tests (`cargo test`).

## Roadmap

Planned, not yet built (details in [DESIGN.md](DESIGN.md) → *Open questions and future work*):

- **Dates & calendar for todos** - tag a todo with a **deadline** or **event** date and surface it on a calendar. The todoz preview pane already hosts a placeholder calendar; the open decision is the `@date` encoding inside `TODO.md` so it round-trips through the `notez` / `todoz` CLI byte-for-byte (mirroring how `#tags` already work).
- **Scope migration** - move a note or todo between scopes (e.g. `personal → public`) without losing history.

## License

MIT.
