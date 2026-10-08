# NZ-29: LSP in the preview (design note)

Status: draft for Andreas's approval, written by the Relay lead on
2026-10-08. No code until approved. Ticket record and board item:
`docs/agent-handoff.md`, NZ-29.

## The question

What does a read-only preview inside a notes browser gain from a
language server, and is it worth running one?

## What LSP would give

- Hover documentation for the symbol under a cursor the preview does
  not have (the preview scrolls, it has no text cursor).
- Diagnostics: the same class of information NZ-28 gets from
  tree-sitter error nodes, plus semantic errors (type errors, unused
  imports). Semantic errors need the project built or indexed by the
  server, which can take seconds to minutes per project.
- Document symbols: an outline of functions, types and headings.
- Go to definition, references, rename: editing features; the preview
  cannot act on them.

## What it costs

- One server process per language, found on `PATH` or configured
  (`rust-analyzer`, `pyright`, `kotlin-language-server`, `jdtls`,
  `clangd`). Each is a large program with its own startup, memory and
  indexing time. Starting one when a file is selected and stopping it
  when the selection moves is the wrong lifetime; keeping them alive
  for a session turns notez into a process manager.
- A JSON-RPC client (initialize, didOpen, publishDiagnostics,
  shutdown), a new dependency for the protocol types, async I/O or a
  thread per server, and error handling for every server that is
  missing, crashes or answers slowly.
- Notes are mostly markdown. Markdown language servers exist
  (`marksman`) but what they add over NZ-25 and NZ-28 is link
  completion and rename, both editing features.

## Recommendation

Do not embed LSP in the preview. The browser already has `o` to open
the file in the editor, where LSP lives with a cursor and the ability
to act on results. Instead, take the one LSP feature that is useful
read-only and get it from tree-sitter, which NZ-27 already runs:

- **Symbol outline** (proposal, ticket NZ-29a): a toggleable outline
  pane or overlay listing the file's headings (markdown) or top-level
  functions, types and constants (code), from the grammar's `tags` or
  a small per-language query; `Enter` on an entry scrolls the preview
  to it. No process, no dependency, works offline and instantly.

Revisit LSP if notez ever gets an editing mode with a cursor; then the
question is real and the answer is probably still "open the editor".

## Decision needed from Andreas

1. Close NZ-29 as "not now" and open NZ-29a (symbol outline), or
2. keep LSP on the list for a later editing mode, or
3. insist on LSP in the preview now: then the lead writes the full
   design (client, lifetime, servers, failure modes) as the next step,
   with the costs above in view.
