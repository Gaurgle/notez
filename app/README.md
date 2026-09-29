# epoz (desktop app)

**epoz** is the desktop workspace over the notez data model: Notes, Todos, Tickets (GitHub Issues), Spaze, and a live repo Dashboard. Tauri (Rust) backend reusing `notez-core`, SvelteKit + TypeScript frontend, CodeMirror 6 editor. It reads and writes the same files as the `notez` CLI, so notes and todos round-trip without spurious diffs.

Naming: **notez** is the CLI/core (the `notez` binary); **epoz** is this desktop app. Scope language is two-axis: accessibility (personal vs public) x binding (project vs global), plus a machine-only scratch tier. The name previously belonged to a standalone ratatui repo-dashboard TUI, which now lives on as **fleetz**.

## Develop

```bash
npm install
npm run tauri dev      # dev build, hot reload
npm run check          # svelte-check (types + a11y)
npm run tauri build    # production bundle
```

The Rust backend lives in `src-tauri/` (crate `notez-app`); the frontend in `src/`.

## Layout

```
src/lib/ipc.ts                  typed wrappers around Tauri commands
src/lib/types.ts                DTO mirrors of the Rust wire format
src/lib/components/             DashboardView, NotesView, TodozView, TicketzView,
                                SpazeView, NoteEditor, MarkdownPreview, Sidebar,
                                Inspector, Calendar, Resizer, NoteList, todo/…
src/routes/+page.svelte         shell: Dashboard / Notes / Todos / Tickets / Spaze
src-tauri/src/commands.rs        #[tauri::command] handlers
src-tauri/src/github.rs          gh-CLI-backed GitHub data layer
src-tauri/src/dto.rs             serde wire DTOs (paths as strings)
```

See the root [README](../README.md) for features and the scope model.
