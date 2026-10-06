# notez

A local-first note-taking tool with a CLI/TUI. Cross-machine portable rewrite of [notez-cli](https://github.com/Gaurgle/notez-cli) (now deprecated).

**Naming:** `notez` is the CLI and core (`crates/`). The desktop app, **epoz**, lives in [its own repo](https://github.com/Gaurgle/epoz) and builds on `notez-core`.

## What it is

- Two axes, not four silos: **accessibility** (personal vs public) x **binding** (project vs global). Default = public+project: committed with the repo, shared with collaborators (outside a repo it falls back to `~/notez`). `-p` = personal+project: private notes in your own notez repo, synced across your machines, never touching the project repo. `notez add` writes to the scope root; `notez quick` (or `notez add quick`) writes a private quick note to `00_quick-notes/`. `-g` = personal+global: the `~/notez` repo itself, for notes bound to no project. `-l` = scratch: gitignored, this machine only.
- One model that surfaces everything from the CLI or epoz: local scratch, personal-per-project, public-with-team, and global cross-project notes.
- A full todoz todo manager: tags, subtasks, drag-to-reorder.
- Cross-machine portable. No OS symlinks. No absolute paths persisted. Per-machine project registry; private notes stay private by living in your own repo.

## Repository layout

A Cargo workspace:

```
crates/notez-core/   # scopes, aggregation, todoz model, tags (GUI-agnostic)
crates/notez-cli/    # the `notez` binary (+ todoz/zlog symlink dispatch)
```

## Desktop app

The desktop app, **epoz**, moved to its own repo:
[Gaurgle/epoz](https://github.com/Gaurgle/epoz). It uses `notez-core` as a
library, so notes and todos round-trip between `notez`, `nvim` and epoz.

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
notez            notez -g <title>  notez -p <title>
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
A bare `notez` opens it too. With no scope flag it shows the whole current
project (personal, public, docs and scratch sections), or the global view
outside a project; `-g` opens the global view, `-p` only the project's
personal notes, `-l` only its scratch notes.

**Keys.** In both the board and the tree, the footer shows the keys for the
current mode (browsing, filter, tags, focus, text entry such as rename or a
new todo, `:` command) and lights the ones whose mode is on, such as `f`
while a section is focused. In tag, text-entry and `:` command modes the
tag legend, prompt or command comes first and the keys follow in the space
left. On the `:` command line, `Esc` (or Backspace past the `:`) only closes
it; `:q`, `:wq`, `:qa` or `:q!` then Enter quits. `?` opens a help overlay
listing every key of that view; `?` or `Esc` closes it, and `j`/`k` scroll
it on a short terminal.

**notez add** writes a public note to the repo's `notez/` (`-p` for a
private one under `~/notez/personal/<project>/`). **notez quick** (or
`notez add quick ...`) writes a private quick note to `00_quick-notes/`.
A scope flag followed by words is the short form: `notez -g call the bank`
is `notez -g quick call the bank`, and likewise for `-p` and `-l`. A first
word that names a subcommand runs it (`notez -g tree`); use `quick` to title
a note with such a word. Words without a scope flag are an error, so a
mistyped subcommand never becomes a note. Words starting with `-` go after
`--` (`notez -g -- -x marks`). A new note never overwrites an existing
file: if today's `YYYY-MM-DD-<title>.md` is taken, it gets `-2`, `-3` and so
on before the extension.

`notez add --in <dir>` targets a subdirectory (global root by default,
the current scope's root with `--in-local`); bare `--in` opens an fzf
picker. Scratch writes (`-l`) auto-gitignore `.notez/` in the repo.

**Auto sync.** When an interactive session ends (`notez tree`, `todo`, `edit`,
`logz`, or `add` that opened the editor), notez commits the vault, runs
`git pull --rebase` and pushes, like `notez sync` but silent unless something
happened. Offline, no upstream or a conflict never fails the command: a failed
pull aborts its rebase, leaves your notes as local commits and prints a one-line
warning. Pass `--no-sync` to skip it. Only `~/notez` syncs; a project's public
`notez/` folder is never touched.

**Pull on open.** `notez tree` (and bare `notez`), `notez todo` with no item
and `notez edit` also pull before they open, so you see the merged vault:
pending changes are committed, then `git pull --rebase` runs, without a push.
Offline or no upstream opens the local notes silently. A conflict aborts the
rebase, opens the local notes with a warning in the footer (repeated on stderr
when you quit, or after the editor for `edit`), and skips the exit sync so
nothing is pushed over it; resolve it with `notez sync`. `add`, `quick`, `log`,
`logz` and `todo "item"` never pull, and `--no-sync` skips the pull too.

**notez edit [term]** (alias `editz`) opens an existing note. Candidates come
from the scope model, so it sees exactly the notes the rest of the tool
considers in scope; a term matching one note skips the picker.

**notez rename [term] [title]** retitles a note found the same way as `edit`.
The `YYYY-MM-DD-` prefix is kept, the title is slugified into the filename, and
a leading `# heading` is rewritten to match. Omit the title to be prompted. It
refuses to overwrite an existing note.

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
