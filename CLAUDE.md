# notez

Local-first notes and todos: `crates/notez-core` (engine), `crates/notez-cli`
(the `notez` binary and its aliases). The desktop app, epoz, lives in
`Gaurgle/epoz` and depends on `notez-core` by pinned git rev, so file-format
changes here must stay compatible with it.
Design and scope model: `DESIGN.md`.
Agent work runs on Relay: `docs/agent-workflow.md` (roles, checks, safe-merge),
`docs/agent-handoff.md` (the baton), `.claude/agents/nz-*.md`.

<!-- house-rules:start v1 -->
## House rules

These mirror the global config at `~/claude-config`, which is machine-local and
therefore invisible to cloud and mobile sessions. They apply here regardless of
where the session runs.

- Avoid em dashes (U+2014) and en dashes (U+2013) in new prose. Preserve exact
  quotations, literal data, code, and unrelated existing text.
- **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`,
  `cleanup:`), first line under 72 characters. Body is for the why, not the what.
- **No `Co-Authored-By` lines.** Ever.
- By default, present the full `git commit -m "..."` command for the human to
  run. An explicit commit/push request authorizes that action. **"Ship it"**
  authorizes committing and following this repo's branch/PR and **Ship policy**
  instructions. If that flow is missing or unclear, ask before committing or
  pushing. Never include unrelated changes or secrets, or ship failing checks.
- Merging, tagging, releasing, deploying, force-pushing, rewriting published
  history, deleting remote refs, and infrastructure changes need a separate
  explicit request. Check for deployment side effects before pushing.
- Never weaken tests just to hide a failure. Update tests for authorized
  behavior changes while preserving meaningful coverage. In spec-driven/ATDD
  mode, acceptance tests stay frozen; raise a suspected test defect first.
- Ask before dependency, schema/API, CI, or file-deletion changes unless
  already explicitly authorized. Do not request the same permission twice.
<!-- house-rules:end -->

## Ship policy

Direct commits to `main`, no PR step. No CI: a push triggers nothing.
Relay leads present commit commands and stop at Ready to integrate unless the
owner records a delegation in `docs/agent-handoff.md`.

## Checks

```bash
cargo build --workspace && cargo test --workspace
```

`cargo fmt --check` fails repo-wide on pre-existing drift. Do not reformat
untouched files as part of an unrelated change.

## Gotchas

- This repo is public. The private notes vault is `Gaurgle/notez-vault`,
  checked out at `~/notez`. Never let a vault remote point here.
- `master` was renamed to `main` on 2026-10-05. A checkout still on
  `master` needs: `git branch -m master main && git fetch origin &&
  git branch -u origin/main main && git remote set-head origin -a`.
- `docs/` is historical record (specs, plans, handovers) and keeps the old
  `notez2` name on purpose.
- After pulling, run `./install.sh`. Its `codesign` step is required on Apple
  silicon.
