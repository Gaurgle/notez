Historical (written 2026-09-02); current design in DESIGN.md.

# Handover: bring the small machine in line with the big one

Date: 2026-09-02. Written on the big machine (`/Users/andreasroos`) for the
small one (`/Users/at-a`). Goal: identical structure on both, so `notez` and
`todoz` can be used on either without one machine scrambling the other.

Read this top to bottom. The order matters in two places, both marked.

## What happened, in one paragraph

The small machine ran the old `notez-cli` binary (April build) against the
vault after the big machine had migrated it to the notez2 scope layout in
July. On every `-g` command the old binary recreated legacy mirror dirs
(`18_career`, `19_rustfinity`, `20_file-gatherer`), each holding a symlink to
an absolute `/Users/at-a/...` path. Those were dangling on the big machine.
The checkpoint commit from 2026-09-01 carried them into the vault. They are
now removed (vault commit `8151067`). Nothing else was lost: the merge the
small machine made kept the migration intact, and the three new todo boards
under `_todos/` came through fine.

## State of the big machine after today

| Item | State |
| --- | --- |
| `~/repos/notez2` | at origin/master, plus `install.sh` and these docs |
| `~/.local/bin/notez` | built from that commit, ad-hoc signed, 8 alias symlinks |
| `~/notez` | clean, at origin/main `8151067`, no numbered dirs except `00_`/`01_` |
| registry | `career` and `wireless-test-platform` repointed to their git toplevels |
| subdir check | `-l` and `-p` from three levels deep land at the repo root (verified) |

## Steps on the small machine

### 1. Replace the binary. Do this before anything else.

The old binary rewrites the vault on `-g` commands. Running it once more
after pulling recreates the mirror dirs and the loop starts again.

```bash
notez --help | head -3          # "Standalone commands" block = old notez-cli
cd ~/Repos/notez2
git pull --ff-only
./install.sh                    # cargo build, cp, codesign, alias symlinks
notez --help | head -3          # must now say "notez2  a local-first note-taking tool"
```

`install.sh` re-signs the binary after copying. That is not optional on
Apple silicon: an unsigned overwrite gets SIGKILLed on launch.

If `~/.config/notez/config.toml` does not exist yet, run `notez setup` once.
It writes defaults (`notez_root = "~/notez"`). The old `~/.config/notez/config`
and `~/.config/notez/projects` files can stay; notez2 only reads `projects`
during `migrate-from-legacy`.

### 2. Sync the vault

```bash
cd ~/notez
git status                      # commit anything local first
git pull --rebase
ls -d [0-9][0-9]_*              # expect only 00_quick-notes and 01_daily-logs
```

If `18_career`, `19_rustfinity` or `20_file-gatherer` are still present after
the pull, the old binary ran in between. Delete them with `git rm -r` and
commit; they are symlinks, no content lives in them.

### 3. Rescue the three files the mirrors pointed at

These live in repo stores on the small machine and are untouched. Under the
new rules, local and public stores sit at the git toplevel, not in
subdirectories, so move them there:

| Old location (subdir store) | Move to |
| --- | --- |
| `~/Repos/career/applications/marshall/notez/TODO.md` | `~/notez/personal/career/TODO.md` (synced; `personal/career/` exists, has no TODO.md yet) |
| `~/Repos/Rust/rustfinity/.notez/00_quick-notez/` | `~/Repos/Rust/rustfinity/.notez/00_quick-notes/` (note the `s`; then remove the empty `00_quick-notez`) |
| `~/Repos/IMRSV/file-gatherer/spike/2026-08-31-dry-run/.notez/TODO.md` | `~/Repos/IMRSV/file-gatherer/.notez/TODO.md` (merge by hand if one exists) |

The marshall board goes to personal scope on purpose: it is the one you want
on both machines. The other two are scratch and stay machine-local.

After moving, delete the now-empty subdir stores
(`applications/marshall/notez`, `spike/.../.notez`) so nothing keeps reading
from them.

### 4. Rebuild the per-machine registry

The registry is per machine by design and is never synced. Every entry must
point at a git toplevel, because that is where notez2 now roots `.notez/` and
`notez/`.

```bash
cat ~/.config/notez/registry.toml           # see what is there
notez attach career --path ~/Repos/career   # NOT applications/marshall
notez attach rustfinity --path ~/Repos/Rust/rustfinity
notez attach file-gatherer --path ~/Repos/IMRSV/file-gatherer
notez list
```

Repeat `notez attach <name> --path <toplevel>` for every project on that
machine. Keep the names identical to the big machine's registry where the
project exists on both, because the name selects `~/notez/personal/<name>/`.
Names in use on the big machine: airwavez, app2, auraz, career, j24-examen,
noiz, notez-cli, notez2, repos, repoz, tokenz, tranzlate, wavez,
wireless-test-hub, wireless-test-platform, zalary.

`attach` scaffolds an empty `notez/` in the project. Git ignores empty dirs,
so this is invisible until a public note is written.

### 5. Verify from a subdirectory

```bash
cd ~/Repos/career/applications/marshall
notez -l log "probe"      # must print .../Repos/career/.notez/01_daily-logs/...
notez -p add probe "x"    # must print .../Repos/career/notez/00_quick-notes/...
```

Then delete the probe files. If either path contains `applications/marshall`,
the old binary is still on PATH somewhere: `which -a notez`.

### 6. Push and confirm both machines agree

```bash
notez sync                 # git pull --rebase then push on ~/notez
```

On the big machine afterwards: `git -C ~/notez pull --rebase` and
`todoz -g` should show the marshall board.

## Daily rule from now on

- Start a session on either machine with `notez sync` (or `git -C ~/notez pull --rebase`).
- Never run the old `notez-cli` binary again. Archive the repo once Phase 1 lands.
- Registry edits happen on each machine separately. That is expected.

## What is still open, in plan order

See `docs/superpowers/plans/2026-09-01-notez-consolidation.md` for the
full plan. Status as of today:

| Phase | Status |
| --- | --- |
| 1. naming (vault -> `notez-vault`, notez2 -> `notez`) | **not started**, gated on step 1 below |
| 2. close the stubs (`logz`, `nav`, `edit`) | **done** 2026-09-10, all of 2.1 to 2.5 |
| 3. install script | done today (`install.sh`) |
| 4. migrate the vault | done on the big machine in July; the small machine only pulls |
| 5. docs and config | **done** 2026-09-10 (see addendum); branch rename to `main` open |

Phase 1 sequence, unchanged from the spec. The one irreversible-if-wrong
step is creating a new `Gaurgle/notez` while any machine still pushes its
vault there, so:

1. `gh repo rename notez-vault -R Gaurgle/notez`
2. On **both** machines: `git -C ~/notez remote set-url origin git@github.com:Gaurgle/notez-vault.git`, then `git -C ~/notez fetch` to prove it.
3. Only then: `gh repo rename notez -R Gaurgle/notez2`, `mv ~/Repos/notez2 ~/Repos/notez`, `git remote set-url origin git@github.com:Gaurgle/notez.git`, re-attach the `notez2` registry entry as `notez`.
4. Rename "notez2" to "notez" inside the repo (31 occurrences, three by hand; see spec Phase 1 step 5).
5. Archive `Gaurgle/notez-cli` with a pointer README. Delete `~/Repos/epoz`.

## Where this is heading

notez2 is the engine (`crates/notez-core`) plus the CLI (`crates/notez-cli`).
The desktop app in `app/` (epoz) already depends on the core by path. The
plan is one core, three fronts: the `notez` CLI, the epoz desktop app, and
later an epoz TUI. All three read the same vault and the same scope model,
which is why the layout has to be identical on every machine first.

---

## Addendum, 2026-09-10 (big machine)

Written because the original above is notez-only, and the small machine also
needs a dotfiles and claude-config catch-up that has to happen in a specific
order. This addendum supersedes the status table above where they disagree.

### What changed since 2026-09-02

- **Phase 2 is done.** `logz`, `nav` and `edit` are implemented and no longer
  exit 1. Only `demo` stays stubbed, by decision. 208 workspace tests pass.
  The small machine picks these up when it runs `./install.sh`.
- **claude-config was the real source of the cross-machine drift.** The
  "global CLAUDE.md updated today" line in the table above described an edit
  that was written on 2026-09-02 and never committed. The small machine
  therefore never received it, and on 2026-09-10 it moved the old April text
  into `skills/notez/SKILL.md`. That skill is now corrected and pushed.
- **fleetz could not see repos directly under `$HOME`.** A dead `.git`
  directory in `$HOME` (no HEAD, config, objects or index) made its home
  sweep stop immediately. Moved aside on the big machine. If the small
  machine shows repos as "not cloned" that clearly exist, check for `~/.git`.

### Read this before trusting the notez skill

Until step 2 below completes, this machine's `skills/notez/SKILL.md` is the
**pre-correction** version: it points at the deprecated `~/Repos/notez-cli`,
describes numbered vault directories, and says `todoz` resolves against the
cwd. All three are wrong. Pull claude-config first, then trust it.

### Full order for the small machine

Steps 3 and 4 are the original handover above, unchanged. Do not reorder them.

1. `dotup` (dotfiles, and the fleetz config that now scans `~/claude-config`)
2. `git -C ~/claude-config status --short`, commit anything local, then
   `git -C ~/claude-config pull --rebase`
3. Replace the notez binary: `cd ~/Repos/notez2 && git pull --ff-only && ./install.sh`
   **This must happen before any notez command touches the vault.** The old
   binary recreates the legacy mirror dirs on every `-g`.
4. Vault, registry, and the three rescued files: sections 2 to 6 above.
5. `pinz sync` if you want the board's `sync` world, which carries a short
   version of this list.

Expect step 2 to hit uncommitted local changes to `CLAUDE.md` or
`settings.json`. That is what happened on the big machine. Commit them, do
not discard them, and reconcile against what upstream already rewrote.

### Still open after all of the above

Phase 1, the rename, remains gated on **both** machines repointing their
`~/notez` remote to `notez-vault` before `Gaurgle/notez2` takes the `notez`
name. Two decisions are also unanswered: whether `notez-core` is published to
crates.io, and whether real distribution (`cargo install`, prebuilt binaries,
a Homebrew formula) is in scope.
