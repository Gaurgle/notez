# NZ-28: linting in the preview (design note)

Status: draft for Andreas's approval, written by the Relay lead on
2026-10-08. No code until approved. Ticket record and board item:
`docs/agent-handoff.md`, NZ-28.

## What the preview is

The tree browser's right pane shows one selected file, read only
(rendered markdown since NZ-25, syntax highlighted since NZ-27). Linting
here means: tell the reader that something in the file is off, without
leaving the browser and without editing. Fixing happens in the editor.

## Questions and answers

**Which linters?** Two in-process sources, no external tools in the
first version:

1. Syntax diagnostics from tree-sitter (NZ-27 already parses the file).
   Every `ERROR` and `MISSING` node in the tree is a diagnostic on its
   line: "syntax error", "missing `)`". Works for all nine languages
   with zero extra cost and zero configuration.
2. Markdown structure checks, written in Rust against the pulldown
   events NZ-25 already produces: heading level jumps (`#` to `###`),
   duplicate headings in one note, an unterminated fence, a relative
   link whose target file does not exist, a reference link with no
   definition, trailing whitespace, a `TODO.md` task line that is not
   `- [ ]` or `- [x]`, a note with no `#` heading. Each check is one
   function, on or off in a small table.

External linters (`ruff`, `ktlint`, `clippy`, `markdownlint`) are not
run. They are slow, need the tool installed, and their output belongs
in the editor or CI. If a concrete need appears, it becomes its own
ticket with the tool named.

**When do they run?** On selection, together with highlighting, since
both sources are already computed there. Cached with the NZ-25 preview
cache key. Files over 1 MB skip linting like they skip highlighting.

**What does the pane show?**

- A gutter mark `▲` in a warning colour at the start of each line with
  a diagnostic, in both the rendered and the raw view (rendered lines
  map back to source lines through the renderer's line map, which
  NZ-25 does not keep yet: this is the one renderer change).
- The footer's file type segment gains a count: `markdown  2 issues`.
  Zero issues shows nothing extra.
- A key opens an issue list overlay (proposal: `!`), listing
  `line: message` sorted by line; `j`/`k` move, `Enter` scrolls the
  preview to that line, `Esc` closes. The overlay reuses the help
  overlay's drawing.

**How does a missing tool degrade?** There is none to miss. A grammar
whose query failed to load (NZ-27) simply yields no syntax diagnostics.

## Not in scope

Auto-fix, running external tools, lint configuration files, linting
the whole vault, CI integration.

## Tickets if approved

- NZ-28a: diagnostics model, tree-sitter syntax diagnostics, the
  markdown checks, tests (one worker pass).
- NZ-28b: gutter marks, footer count, the `!` overlay, the renderer's
  source line map, README (one worker pass).

No new dependency. Allowed files: a new `tui/lint.rs`, `tui/markdown.rs`
(line map), `tui/highlight.rs` (error nodes), `tui/tree.rs`,
`tui/footer.rs`, `tui/help.rs`, `tui/theme.rs`, `README.md`.

## Decision needed from Andreas

Approve as written, or strike the overlay (gutter marks and the footer
count alone are enough for a first version), or name an external tool
you want from the start.
