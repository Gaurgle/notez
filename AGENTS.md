# AGENTS.md

Instructions for Codex and other agents that read `AGENTS.md`. Claude reads
`CLAUDE.md`; both point at the same working agreement, so either can lead.

1. Read `CLAUDE.md` first. Its house rules and invariants bind every agent,
   not only Claude.
2. This repo runs Relay. The workflow, roles, safe-merge rule, checks and
   handoff templates are in `docs/agent-workflow.md`.
3. If you are taking over as lead, read `docs/agent-handoff.md` and the board
   (`https://github.com/users/Gaurgle/projects/2`; read command in
   `docs/agent-workflow.md`) before acting, and update both before you stop.
4. Claude-side agent definitions live in `.claude/agents/` and are for
   reference only on the Codex side; dispatch your own workers with the same
   handoff templates.
