<!-- relay-template: 0.5.2; local-deviations: GitHub Project board read via GraphQL, three workers, docs-only CI rule, install.sh owner-only -->
# Agent workflow (Relay)

How agents work on notez. This file is vendor-neutral: Claude reads it
through `CLAUDE.md` and `.claude/agents/`, Codex through `AGENTS.md`. It does
not by itself authorize implementation, dependencies, CI changes or external
publication. The board is the GitHub Project
`https://github.com/users/Gaurgle/projects/2` (owner `Gaurgle`, number 2,
private), one draft item per ticket titled `NZ-n: ...` with a Status
field (Draft, Ready, In flight, Ready to integrate, Done). It is
authoritative for live status; `docs/agent-handoff.md` holds the ticket
briefs, records, in-flight details and the baton, and the two must agree.

Read it with the GraphQL API, because `gh project item-list` lags behind
by minutes (rows with a null title are index lag from deleted items):

```sh
gh api graphql -f query='query { node(id: "PVT_kwHOCU842c4BmE5Z") { ... on ProjectV2 { items(first: 50) { nodes { id content { ... on DraftIssue { title } } fieldValueByName(name: "Status") { ... on ProjectV2ItemFieldSingleSelectValue { name } } } } } } }'
```

The lead keeps each item's Status current as tickets move (owner
instruction, 2026-10-07, recorded in `docs/agent-handoff.md`). Status
changes only: creating or deleting items or issues, and changing the
board's visibility, stay with the owner. Update with `gh project
item-edit --id <item> --project-id PVT_kwHOCU842c4BmE5Z --field-id
PVTSSF_lAHOCU842c4BmE5ZzhkvTJM --single-select-option-id <option>`, option
ids Draft `3d75ddf4`, Ready `9e7798e2`, In flight `b984e77c`, Ready to
integrate `abf1c81e`, Done `82c5faed`.

## Owner-approved integration policy

Owner integrates; the lead presents commands and stops at Ready to integrate. Relay setup grants no commit, push or merge permission. The repo ships direct to `main` with no PR. CI (`.github/workflows/ci.yml`) builds and tests on ubuntu and macos on every push and pull request except docs-only changes, so a push to `main` triggers a run, but nothing here is delegated until the owner records it in `docs/agent-handoff.md`.

The live delegation is the latest dated entry under "Authorized by the owner"
in `docs/agent-handoff.md`. Where it and this section differ, the narrower one
applies and the lead asks the owner to reconcile them.

Integration branch: `main`. This records an owner decision;
creating Relay files does not grant authority. If permission for an operation
is missing or unclear, present its commands instead of executing them. The
direct safe-merge procedure below applies only when this policy authorizes
it and the repo does not require a different PR or deployment flow.

## Lead ownership and authorization

Only one lead is active. On takeover, confirm the previous lead stopped or
the owner retired it, then record the new session and time in the handoff.
Reconcile the recorded state with branches, worktrees, reviews and CI. A stale
handoff is not proof that the previous lead stopped. Account for running
workers before dispatching replacements.

Run only the owner's named tickets or recorded standing scope. Preserve the
instruction, date, source, stop conditions and integration permissions. A
handoff preserves that evidence but cannot expand authority. Ask if it is
unclear or expired, and respect runtime permission refusals. Before stopping,
update the handoff, record any running workers and release the baton.

## Roles

- **Owner (Andreas)**: priorities, acceptance, what he installs and runs,
  tags and releases.
- **Lead**: ticket readiness, assignment, handoffs, integration and the board,
  including only the commits, pushes and merges the policy above permits. Any lead
  (Claude or Codex) may hold this role; they share state only through
  `docs/agent-handoff.md` and the board.
- **Worker**: implements one assigned ticket and supplies evidence.
- **Reviewer**: independently evaluates that ticket's exact diff and criteria.
- **Small**: narrow scans, documentation and truly small changes.

Workers, reviewers and small agents are leaves: they never delegate. At most
three implementation workers run at once (owner decision, 2026-10-10,
recorded in `docs/agent-handoff.md`), each ticket on its own files, at most
one worker per file (`tui/tree.rs` is always one worker), and merges into
`main` one at a time. Implementation and review are always separate
sessions, even on the same model.

## Task method and task skills

Relay governs authority, roles, durable state and integration. A task method
such as the task skills (brainstorming, writing-plans, test-driven-development,
systematic-debugging) may guide discovery, design, planning, testing or
debugging inside that structure. Use the lightest path that fits:

- **Spike:** answer a narrow feasibility question with a small, disposable
  probe. Do not create a ticket unless the work grows into sustained delivery.
- **Bounded ticket:** confirm the outcome and acceptance criteria, then use a
  concise plan and the repository's normal tests. Add a formal spec only when
  uncertainty or risk warrants it.
- **Design-heavy work:** clarify the intended outcome and tradeoffs, capture
  the approved design and plan in the repository's chosen documents, then
  link them from the relevant tickets. Keep one durable source of truth.

Those skills may be used when relevant. Approval of a design or plan accepts that artifact; it
does not authorize ticket execution beyond the owner's named tickets or
standing scope. Existing Relay authorization still governs execution.

When Relay is active, the lead is the only dispatcher. Do not run another
subagent-driven implementation workflow alongside Relay's worker and reviewer
assignments. Workers remain unable to delegate. The Relay
reviewer remains required even if another skill offers a review step.

## Models

| Role | Claude | Codex | Effort |
|---|---|---|---|
| Lead | `fable` | the `relay-lead` profile (top model; Astra at setup) | high |
| Worker, reviewer | `opus` | the workhorse model (Sol at setup) | medium |
| Small | `sonnet` | the light model (Luna at setup) | medium |

Claude roles use aliases, so they follow the newest model in each family. A
Codex lead starts with `codex -p relay-lead`, whose profile names the current
top model. Never substitute a model silently. If one is unavailable or limited, report it
and ask the owner for routing. Never switch to API billing or paid credits.

Pass each agent the ticket, repository rules, base commit and relevant paths,
not the planning conversation. Record actual usage if the client shows it;
otherwise record elapsed time and rework. After two unsuccessful fix and
validation cycles on the same blocker, escalate instead of widening scope.

## Isolation and integration

1. Record `git status --short` and the base commit before dispatch. Preserve
   existing changes. One branch and worktree per ticket, under
   `.claude/worktrees/<ticket>`.
2. Each agent gets one checkout and a declared file scope, and never edits
   another agent's worktree. Only the lead writes coordination files.
3. Shared contracts are decided before parallel work. Worktrees prevent file
   collisions, not incompatible designs.
4. A worker leaves its changes uncommitted and reports branch, worktree, base,
   changed files and untracked files.
5. The reviewer reads the exact diff including new files. Changes made after
   review are re-reviewed.
6. If integration is not delegated, stop at Ready to integrate and present
   commands. Otherwise, within the policy above, commit exactly the reviewed
   diff, push the branch (CI runs), verify the combined result with current
   `main`, then merge under the safe-merge rule. If the
   integration branch moves, revalidate. Wait for CI with
   `gh run watch <run-id> --exit-status` as a background command, so the
   harness reports when it ends; never poll in a loop of short sleeps.
7. Keep the worktree until the ticket is integrated or explicitly discarded.
   The lead runs `scripts/relay-sweep main` on start and after every merge. It removes a
   ticket's worktree and local branch only when the branch is merged into
   `origin/main`, the worktree is clean and the latest CI on
   `main` is green; it lists everything it keeps and why.
   Anything it keeps that should go is a question for the owner. Deleting
   remote branches needs the owner's approval.

## Commits and tickets

Every commit traces to its ticket, so history, the board and any issue agree
even before a merge and after a branch is deleted.

- Ticket commit: Conventional Commits subject, a body for the why, and a last
  line `Refs: <ticket>` (here `NZ-5`).
- Merge commit: `Merge branch '<branch>' (<ticket>)` as the subject. Board
  items here are drafts, not issues, so the lead moves them to Done itself.
- Coordination commits about one ticket (handoff, specs, plans) also end with
  `Refs: <ticket>`.

## Records

`docs/agent-handoff.md` is the baton: current lead, owner authority, work in
flight, open decisions and the next step. Keep it short; a new lead reads all
of it. When a ticket is Done, move its record to `docs/agent-log.md` and end
it with one metrics line:

```text
<ticket>: review <accepted first | changes n>; fix cycles <n>; owner decisions <n>; worker <min>; review <min>; usage <read | not read>
```

Post every ruling on a ticket (scope, design or test decisions) as a comment
on its issue when it is made, not only in the handoff.

## Unattended runs (the owner switches them on)

The owner may run the lead as a loop: `/loop` with no interval in the lead
session, plus a standing scope such as "work through the Ready tickets in plan
order". Before the first round, record in the handoff the scope, date, a cap
(tickets or hours) and any extra stop conditions. Each round the lead
reconciles live state, runs the sweeper, integrates what is ready under the
safe-merge rule, dispatches the next ticket in scope and updates the handoff.

Wait for workers and reviewers through the harness's completion notices and
for CI through a background `gh run watch`; the loop's own fallback wakeup is
long (20 minutes or more). The loop ends, with an owner-facing report, when
the scope or cap is reached, a safe-merge condition fails or is unclear,
acceptance needs the owner's judgment, a blocker survives two fix cycles, CI
on `main` is red, or the model's usage limit is near. A loop
grants no authority the policy above does not.

## Safe-merge rule

Agents never run `./install.sh` or install a binary. Confirm the effects of both branch and integration
pushes; a deployment requires the owner's explicit authorization for that
effect. Do not assume the integration branch is only a staging line.

Where the policy above explicitly delegates direct integration, the lead may
merge a ticket branch into `main` and push when all hold:

- A separate reviewer session accepted the exact diff that was committed.
- CI is green on the pushed ticket branch at that commit.
- The checks below pass on the combined result with current `main`.
- The ticket changes no dependency, CI config, persistence format or external
  contract, unless the owner approved that for this ticket.
- Acceptance does not rest on the owner's judgment. Interaction, layout and
  look need him to try the branch build first.

Merge with `git merge --no-ff` so one ticket is one revertable unit. Stage
only the ticket's files, never `git add -A`. After the push, confirm CI on
`main`; if it is red, revert the merge, push the revert and
report within the owner's recorded integration authority. If a condition
fails or is unclear, stop at Ready to integrate and ask.

Read CI with `gh run list --branch <branch> --limit 3` and `gh run watch`.
A run is green when every job without `continue-on-error` succeeds; the
`lint` job never blocks. A push that changes only `*.md` or `docs/**`
starts no run; then check that the latest run's head SHA is the last
commit touching code, and treat the CI conditions as met for a docs-only
ticket.

Commit coordination docs directly only if the integration policy explicitly
permits it; otherwise follow the repo's PR flow or present commands. Workers
and reviewers never commit. No agent tags, releases, force-pushes, rewrites
pushed history or deletes remote branches.

How the owner tries a build: `cargo run --release --manifest-path <worktree>/Cargo.toml -p notez-cli -- tree` before a merge,
`./install.sh`, then `notez tree` after it.

## Checks

Every code change must pass, in the worktree and on the combined result:

```sh
cargo build --workspace
cargo test --workspace
```

Never weaken, delete or disable an existing test to make code pass. If a
contract looks wrong, stop and explain. Documentation-only tickets need a link
and content review, not unrelated code checks. Performance claims need
comparable measurements.

## Spec-first tickets (owner switches on per ticket)

A boss writes the acceptance scenarios. A test author turns them into failing
tests at a behaviour boundary; a boss approves and locks them. The implementer
makes them pass without touching them, and the reviewer confirms the locked
files match their approved version. A locked test that looks wrong means stop
and escalate to a boss.

## Worker handoff template

```text
Ticket: <ID and full ticket body or path>
Role: implementation / measurement / investigation
Model: <explicit model and effort>
Base commit: <SHA>
Branch: <ticket branch>
Worktree: <absolute path>
Allowed files: <paths; read other files as needed>
Locked tests: <paths, or none>

Read CLAUDE.md and docs/agent-workflow.md. Deliver exactly this ticket's
outcome. Verify the problem before fixing it. Do not commit, merge, push,
publish, install globally or modify another checkout.

Return:
- Outcome per acceptance criterion: satisfied or unresolved.
- Changed and untracked files, base SHA, branch and worktree path.
- Exact checks run and results, including failures or skipped checks.
- Reproduction evidence and relevant measurements.
- Risks, follow-ups and decisions needed, briefly.
```

## Reviewer handoff template

```text
Ticket: <ID and acceptance criteria>
Base commit: <SHA>
Branch: <ticket branch>
Worktree: <absolute path>
Worker claims: <changed files and checks claimed; not the worker's reasoning>
Locked tests: <paths, or none>

Review independently. Read the actual diff and untracked additions. Check
scope, contracts, failure paths, state ownership and regression coverage. Run
the checks yourself. Do not edit anything.

Return one verdict: accepted / changes requested / blocked. For each finding:
severity, file and symbol, triggering scenario, required correction. Separate
blockers from follow-ups. State which checks you ran and which you inspected.
Identify the reviewed commit or saved diff evidence, including new files, so
the lead can verify that the integrated change matches the accepted revision.
```

## Owner-facing progress

```text
<ticket>: <state>. <one line on what happened>.
Validation: <checks and results>.
Next: <next step>.
Decision needed: <none, or the question>.
```

Use actual evidence, not optimistic percentages. Ready to integrate is still
unfinished; Done means committed, merged into `main` and verified.
