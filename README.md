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
A bare `notez` opens it too. With no scope flag it shows everything in one
view: the current repository's sections first (personal, public, docs and
scratch), expanded, then the vault's global notes, then every other
project, collapsed; outside a repository the first group is absent. A flag
narrows the view: `-g` the vault's global notes only (not the `personal/`
folders), `-p` only the project's personal notes, `-l` only its scratch
notes. It opens even when the view has no notes yet, with an empty-state
line pointing at `n` that names the scope in a narrowed view.

A row starts with a one-column gutter (the mark of a marked row), then the
row's tag dots: only the tags it has, left-aligned in their colours, in a
field as wide as the most tags on any visible row, and no field at all
when no visible row has a tag. Clicking a dot removes that tag. The
selected row is shown by its highlight across the whole line. Nested rows
are drawn as a classic tree (`├─` for a row with a later sibling, `└─` for
the last, `│` for an ancestor with more to come), with `▾`/`▸` on open and
closed folders and `▼`/`▶` on sections.

Every nested row carries a scope badge right before its name: the
section's scope icon (the docs icon for a `docs` section) in that scope's
colour, so rows from different sections tell apart at a glance. Section
headers show the scope icon in the same colour before their label; the
icon and colour name the scope, so no scope word is repeated. Todo rows
(the `_todos` store, every row in it, and every `TODO.md` file) carry a
check-list icon instead of the scope icon, still in the scope's colour.

**New notes in the tree.** `n` creates a note in the folder under the
cursor, in that row's scope. The footer prompt names the target before
anything is created, for example `new note in personal/ideas: _`; on a
project's `docs` section the target is the project's personal root.
`Tab` cycles the scope (personal, public, local scratch, global, as far as
they apply), each time at that scope's root, and returns to the original
folder after a full cycle. Public notes are committed with the project
repository, so the prompt says so: `public (committed with the project)`.
`Enter` creates the note the way `notez add` does (same file name and
content, never overwriting an existing file; an empty title becomes
`untitled`), opens it in the editor, and selects it in the refreshed tree;
`Esc` cancels. In `n`, `N` and `r` the file name is lowercased and spaces
become hyphens without a word (`My Note` makes `my-note.md` with the heading
`# My Note`), but a name sanitizing would drop characters from (`_`, `.`,
punctuation) is refused with `name would become <cleaned>; use letters,
digits and -`, and the prompt stays open with what you typed; the
`notez add`, `notez mkdir` and `notez rename` commands still sanitize.

**Folders in the tree.** Every folder under a section's root is listed,
including empty ones; hidden folders (names starting with `.`) are not.
`N` creates a folder with the same prompt as `n`: the target is the folder
under the cursor, the footer names it (`new folder in personal/ideas: _`)
and `Tab` cycles the scope the same way. `Enter` creates the folder the way
`notez mkdir` does (the same `.gitignore` step for local scratch), then selects and expands it in the refreshed tree; `Esc`
cancels. An empty name, or the name of anything already in the target
folder, is refused in the footer and creates nothing. `N` does nothing in a
`docs` section.

`r` on a folder renames it in place: the prompt shows the current name,
`Enter` renames it, and every note inside
moves with it; their `.tags` entries move to the new paths on exit. `r` on
a note shows its title (the file name without the date and `.md`) and
renames it the way `notez rename` does; on a note or a folder, `Enter` on
the name as shown changes nothing, even one like `My_Note` that sanitizing
would alter. A name
already taken in that folder (file or folder, in any case) is refused and
nothing changes; changing only the case of a name works where the file
system allows it. `d` on a folder asks first, counting what goes, for
example `delete ideas/ and its 2 notes from personal? y/n` (`and its 1 note`,
or `(no notes)` for an empty folder), adding `and other files` when the folder holds anything but
notes and `(not recoverable)` in local scratch. `y` removes the folder with
everything in it, drops the `.tags` entries of the notes that went, and
refreshes the tree with the cursor on the next row. If the delete fails
partway, the footer says `delete failed: ...` and the tree shows what is
left. `r` and `d` on a section row, and on a folder in a `docs` section,
change nothing.

**Deleting notes in the tree.** `d` on a note asks first in the footer,
naming the file and its scope, for example
`delete ideas/2026-10-07-x.md from personal? y/n`; a local scratch note's
prompt adds `(not recoverable)`, since scratch is in no repository. `y`
deletes the file and refreshes the tree, keeping open folders, unsaved tag
changes and the filter, with the cursor on the next note in the folder (or
the one before it). `n`, `Esc` or any other key cancels. The note's `.tags`
entry goes on exit, and the exit sync commits the deletion like any other
change. There is no trash and no undo. `d` on a folder is covered under
**Folders in the tree** above.

**Moving notes in the tree.** `m` on a note or folder moves it: the footer
asks `move <name> to <scope>/<folder>_`, prefilled with the folder it is in
now, where the typed path is relative to the scope's root (empty means the
root) and `Tab` cycles the scope (the row's project's personal, public and
local scratch, then global). `S` sets the scope only: `set scope of <name>:
<scope> (Tab cycles, Enter applies)` keeps the same folder under the scope
`Tab` picks, and `Enter` on the scope it is already in does nothing. The
destination folder must already exist (`N` creates one; `move: no folder
...` otherwise), a name that is taken there is refused, a folder cannot go
inside itself, and nothing is ever overwritten. Every note inside a moved
folder goes with it, and the `.tags` entries follow to the new paths on
exit, into the destination's `.tags` when the scope changes. A move to
another scope asks first, `y` to go ahead, naming the folder's notes the way
a delete does and adding a warning per change: into public `(it will be in
the <repo> repository, public, not yet committed)`, out of public `(it stays
in the repository's git history)`, into local scratch `(scratch is not
synced and not recoverable)`, and out of the vault (personal or global) into
a repository `(it leaves the vault; the deletion syncs on exit)`. notez never
commits or pushes in a project repository: a note moved into public is
yours to commit. A move between volumes copies, checks the copy, then
deletes the original; if that fails partway, both copies stay and the footer
names where. `m` and `S` on a section row or in a `docs` section change
nothing; `docs` is never a destination. The todo board's store, `_todos`
in the global notes, belongs to the todo view: `d`, `r`, `m` and `S` on it
or on anything in it are refused (`delete: the todo board's store is managed
by the todo view`), a marked set holding such a row is refused whole, and it
is never a move destination; `n` and `N` work there as anywhere.

**Marking several notes.** `Space` marks the note or folder under the
cursor (again to unmark) and steps down one row; section rows cannot be
marked. Marked rows show a bar in the
gutter and are drawn bold, and the footer leads with `<n> marked`. With
marks present, `d`, `m` and `S` act on the marked set with one prompt and
one confirm: `d` asks `delete 2 notes and 1 folder (3 notes inside) from
personal, public? y/n` (with `and other files` and `(not recoverable)` as
for a single delete), `m` asks `move 3 items to <scope>/<folder>_` and puts
every item in that folder under its own name, and `S` (`set scope of 3
items: ...`) keeps each item's own folder under the new scope. A move that
changes any item's scope asks first, naming each warning once. A marked
folder together with notes inside it acts once, on the folder. A set that
cannot work as a whole is refused before anything happens, naming the first
problem: a section, a `docs` row or a folder holding another section, items
from different projects for `m` and `S`, two items with the same name, or a
name already taken at the destination. Once a confirmed set runs, an item
that fails stays where it was and the rest go on; the footer reports
`deleted 2, failed 1: ideas/ (...)` or `moved 2, failed 1: ...`. The marks
clear after the action runs, and `n`, `Esc` or any other key at the confirm
keeps them. `n`, `N` and `r` ignore marks and act on the cursor row. Marks
live only for the session and are never saved; a marked row that is renamed,
moved or deleted loses its mark.

**Preview.** The tree's preview pane shows a markdown note (`.md`, any
case) rendered by default: headings styled by level, emphasis, strong and
strikethrough, inline code, code blocks with their language tag, bullet,
numbered and task lists, block quotes with a bar, horizontal rules, and links
as their text with the url dimmed after it. Tables show as aligned columns
with a bold header: a table wider than the pane shrinks its widest columns
and wraps their cells, and one that cannot fit even then is cut off with
`…`. A footnote reference shows as `[1]`, `[2]` in order of first use, and
the footnotes follow the note under a short rule. Raw HTML shows as its
source text; an image shows as `[image: <alt text>]`. The text wraps to the pane
width; code block lines wrap by character and keep their indentation. `p`
switches between rendered and raw; the footer hint names the view
`p` switches to (`p raw` or `p rendered`) and shows only while a markdown
note is selected. The choice holds for every note until the browser closes
and is never saved. Other files always show raw.

**Syntax highlighting.** The preview highlights code with tree-sitter
grammars built into the binary: Rust, Python, Kotlin, Java, C, TOML, JSON,
Bash and Markdown. In a rendered note, a fenced code block is highlighted
when its tag is one of `rust`/`rs`, `python`/`py`, `kotlin`/`kt`, `java`, `c`,
`toml`, `json`, `bash`/`sh`/`shell` or `markdown`/`md` (any case; anything after
a comma or space, as in `rust,ignore`, is ignored); other tags show plain. A
raw note and any file with a matching extension (`.rs`, `.py`, `.kt`, `.kts`,
`.java`, `.c`, `.h`, `.toml`, `.json`, `.sh`, `.bash`, `.md`) is highlighted
whole. Files over 1 MB are shown plain to keep selection fast.

While a file is selected, the footer leads with its type: the language name
when the extension has a grammar (`rust`, `kotlin`, `markdown`), otherwise
the lowercase extension (`txt`, `yaml`), or `file` when it has none; folder
and section rows show none. A file over the limit reads `rust (not
highlighted, large)`, and a grammar that failed to load reads `rust
(highlighter unavailable)`.

**Keys.** In both the board and the tree, the footer shows the keys for the
current mode (browsing, filter, tags, focus, text entry such as rename, a
new note or a new todo, a delete confirmation, `:` command) and lights the ones whose mode is on, such as `f`
while a section is focused. In tag, text-entry and `:` command modes the
tag legend, prompt or command comes first and the keys follow in the space
left. On the `:` command line, `Esc` (or Backspace past the `:`) only closes
it; `:q`, `:wq`, `:qa` or `:q!` then Enter quits. While browsing the tree,
`Esc` clears the marks if there are any (and does nothing else), otherwise
clears the filter, otherwise quits. In the tree, `J`/`K` (or
Shift+Down/Up) scroll the preview pane a line and PgDn/PgUp a page; plain
`j`/`k` and Down/Up move the cursor. The mouse wheel scrolls the pane under
the pointer: three lines of the preview, or the list's cursor a row.
Shift+Down/Up need a terminal that sends them as distinct keys: Ghostty,
kitty, iTerm2
and tmux (with its default `xterm-keys`) do; macOS Terminal.app has no
default mapping for them, so use `J`/`K` there or add the mappings
`\033[1;2B` and `\033[1;2A` in its keyboard settings. `?` opens a help overlay
listing every key of that view; `?` or `Esc` closes it, and `j`/`k` scroll
it on a short terminal.

**Panes.** The tree's list and preview are two panes with a split you can
move: `<` and `>` narrow and widen the list by 5 points, `=` resets it to
50/50, and dragging the `⠿` grip on the border between them resizes it
directly (the grip lights while you drag). Neither pane gets narrower than
it can use. `1` focuses the list and `2` the preview, `Tab` switches between
them, and clicking a pane focuses it; the focused pane has the lit border
and its number lit in its title. With the preview focused, `j`/`k` (or
Down/Up) scroll it a line and PgDn/PgUp a page, the keys that act on the
list's cursor row do nothing, and the footer shows the preview's keys. `2`
on the focused preview folds it and the list takes the full width; `2`
again unfolds and focuses it. Opening a prompt, the filter, tag mode or the
`:` line moves focus back to the list. On a terminal too narrow for both
panes the preview folds on its own and comes back when there is room again;
meanwhile `2` and `Tab` do nothing, and a preview you folded yourself stays
folded. The split, focus and fold are not saved between runs, and a folded
preview reads nothing from disk.

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
