//! notez binary entry point.
//!
//! Handles three things before delegating to per-command modules:
//!
//! 1. Installs a panic hook that always calls [`tui::leave`] so a crashed
//!    TUI never leaves the terminal in raw mode + alt screen + mouse capture.
//! 2. Performs argv-0 dispatch. If the binary was invoked as `todoz`,
//!    `zlog`, `znote`, `treez`, `logz`, `editz` or `findz`, rewrites argv to
//!    select the corresponding subcommand.
//! 3. Parses the CLI and dispatches.

mod cli;
mod commands;
mod tui;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};
use notez_core::config::Config;
use notez_core::core::{Project, Scope};

fn main() -> ExitCode {
    install_panic_hook();

    let argv: Vec<String> = std::env::args().collect();
    let argv = rewrite_for_symlink(argv);

    let parsed = match Cli::try_parse_from(&argv) {
        Ok(cli) => cli,
        Err(err) => {
            err.print().ok();
            return ExitCode::from(2);
        }
    };

    if parsed.help {
        print_help();
        return ExitCode::SUCCESS;
    }

    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e:#}");
            return ExitCode::FAILURE;
        }
    };

    if parsed.nav {
        return finish(commands::nav::run(&config));
    }

    let in_project = Project::try_detect().is_some();
    let scope = Scope::from_flags(parsed.global, parsed.private, parsed.local, in_project);

    // No subcommand: open the tree browser. notez-cli made a bare `notez`,
    // `notez -g` or `notez -p` open a browser on the resolved scope, and
    // `tree` is that command's successor here. Without this, the habit of
    // typing `notez -g` hits a dead end.
    let sync = !parsed.no_sync;
    let Some(cmd) = parsed.command else {
        let stop = pulled_before(&config, sync);
        let warning = footer_warning(stop.as_deref());
        let result = commands::tree::run(scope, &config, warning.as_deref());
        return finish(synced_after(result, &config, sync, stop.as_deref()));
    };

    let result: anyhow::Result<()> = match cmd {
        Commands::Add { mut title, r#in, in_local }
        | Commands::Znote { mut title, r#in, in_local } => {
            let is_quick = commands::add::take_quick_keyword(&mut title);
            commands::add::run(title, r#in, in_local, is_quick, scope, &config)
                .map(|created| report_created(&created, &config, sync))
        }
        Commands::Quick { title } => commands::add::run(title, None, false, true, scope, &config)
            .map(|created| report_created(&created, &config, sync)),
        Commands::Log { message } | Commands::Zlog { message } => {
            commands::log::run(message, scope, &config).map(|p| {
                println!("Appended to: {}", p.display());
            })
        }
        Commands::Logz | Commands::Logs => {
            synced_after(commands::logz::run(scope, &config), &config, sync, None)
        }
        Commands::Mkdir { name } => commands::mkdir::run(name, scope, &config).map(|p| {
            println!("Created: {}", p.display());
        }),
        Commands::Search { term } | Commands::Findz { term } => {
            commands::search::run(term, &config)
        }
        Commands::Tree | Commands::Treez => {
            let stop = pulled_before(&config, sync);
            let warning = footer_warning(stop.as_deref());
            let result = commands::tree::run(scope, &config, warning.as_deref());
            synced_after(result, &config, sync, stop.as_deref())
        }
        Commands::Setup => commands::setup::run(),
        Commands::Demo { view: _ } => Err(anyhow::anyhow!(
            "demo is not implemented; it was a screenshot helper in the legacy CLI"
        )),
        Commands::Completions { shell } => commands::completions::run(&shell),
        Commands::Init { shell } => commands::init::run(&shell),
        Commands::Todo { item } | Commands::Todoz { item } => {
            let interactive = item.is_none();
            let stop = pulled_before(&config, sync && interactive);
            let warning = footer_warning(stop.as_deref());
            let result = commands::todo::run(item, scope, &config, warning.as_deref());
            synced_after(result, &config, sync && interactive, stop.as_deref())
        }
        Commands::Edit { term } | Commands::Editz { term } => {
            // No footer to warn in: the editor owns the screen, so a stopped
            // pull is only reported on stderr once it exits.
            let stop = pulled_before(&config, sync);
            let result = commands::edit::run(term, scope, &config);
            synced_after(result, &config, sync, stop.as_deref())
        }
        Commands::Rename { term, title } => {
            commands::rename::run(term, title, scope, &config).map(|path| {
                println!("Renamed to {}", path.display());
            })
        }
        Commands::Nav => commands::nav::run(&config),

        Commands::Attach { name, path } => commands::attach::run(name, path).map(|r| {
            let suffix = if r.already_existed { " (updated)" } else { "" };
            println!(
                "Attached {} at {}{}",
                r.name,
                notez_core::util::tilde::contract(&r.local_path),
                suffix,
            );
            if r.created_public_store {
                println!("Created notez/ for public notes.");
            }
        }),
        Commands::Detach { name } => commands::detach::run(name).map(|_| {
            println!("Detached.");
        }),
        Commands::List => commands::list::run(&config),
        Commands::Sync => commands::sync::run(&config),
        Commands::MigrateFromLegacy { dry_run } => {
            commands::migrate::run(dry_run, &config)
        }
    };

    finish(result)
}

/// Pull the vault before an interactive session opens, so it shows the merged
/// notes. Silent unless the pull stopped; then returns a one-line reason for
/// the session's footer and for [`synced_after`]. Stderr is no use here: the
/// alternate screen wipes it moments later.
fn pulled_before(config: &Config, enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }
    match notez_core::sync::pull_on_open(&config.notez_root_path()) {
        notez_core::sync::AutoSync::Stopped(why) => Some(
            why.lines()
                .next()
                .unwrap_or("vault pull stopped")
                .to_string(),
        ),
        _ => None,
    }
}

/// The footer text for a session whose opening pull stopped.
fn footer_warning(pull_stop: Option<&str>) -> Option<String> {
    pull_stop.map(|why| format!("{why} - local-only this run"))
}

/// Sync the vault once an interactive session has ended cleanly. Quiet unless
/// something happened or went wrong, and never fails the command: the notes are
/// saved either way, and `notez sync` shows git's own output for a stopped sync.
///
/// `pull_stop` is the reason the opening pull stopped, if it did. Then nothing
/// syncs, so the push cannot go over a pull that needs a human, and the reason
/// is repeated on stderr now that the terminal is back.
fn synced_after(
    result: anyhow::Result<()>,
    config: &Config,
    enabled: bool,
    pull_stop: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(why) = pull_stop {
        eprintln!("notez: {why}; nothing was pushed this run, resolve it and run `notez sync`");
        return result;
    }
    if result.is_ok() && enabled {
        match notez_core::sync::auto_sync(&config.notez_root_path()) {
            notez_core::sync::AutoSync::Idle => {}
            notez_core::sync::AutoSync::Done => eprintln!("notez: vault synced"),
            notez_core::sync::AutoSync::Stopped(why) => {
                eprintln!("notez: sync stopped ({why}); run `notez sync` to see it");
            }
        }
    }
    result
}

fn report_created(created: &commands::add::Created, config: &Config, sync: bool) {
    println!("Created: {}", created.path.display());
    // Legacy UX: no inline body means "write it now" - open the editor on
    // the fresh note. With a body, stay silent.
    if !created.had_body {
        commands::add::open_created(&created.path, config);
        let _ = synced_after(Ok(()), config, sync, None);
    }
}

fn finish(result: anyhow::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("notez: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn load_config() -> anyhow::Result<Config> {
    Config::load()
}

/// Map symlink names like `todoz` -> `notez todo`. If argv[0]'s file name is
/// already `notez`, returns argv unchanged.
fn rewrite_for_symlink(mut argv: Vec<String>) -> Vec<String> {
    if argv.is_empty() {
        return argv;
    }
    let bin_name = std::path::Path::new(&argv[0])
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let subcommand = match bin_name.as_str() {
        "todoz" => Some("todoz"),
        "zlog" => Some("zlog"),
        "logz" => Some("logz"),
        "zlogs" => Some("logs"),
        "znote" => Some("znote"),
        "treez" => Some("treez"),
        "editz" => Some("editz"),
        "findz" => Some("findz"),
        _ => None,
    };

    if let Some(sub) = subcommand {
        argv[0] = "notez".to_string();
        argv.insert(1, sub.to_string());
    }

    argv
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = tui::leave();
        default(info);
    }));
}

fn print_help() {
    use console::Style;
    let lavender = Style::new().color256(183).bold();
    let mauve = Style::new().color256(140);
    let sapphire = Style::new().color256(110);
    let overlay = Style::new().color256(244);

    let div = "─".repeat(60);

    println!();
    println!("  {}", overlay.apply_to(&div));
    println!(
        "  {}  {}",
        lavender.apply_to("notez"),
        overlay.apply_to("a local-first note-taking tool"),
    );
    println!("  {}", overlay.apply_to(&div));
    println!();

    let cmd = |name: &str, desc: &str| {
        println!(
            "    {:<32} {}",
            sapphire.apply_to(name),
            overlay.apply_to(desc),
        );
    };

    println!("  {}", mauve.apply_to("Notes"));
    cmd("notez add [title]", "create a public note in the repo");
    cmd("notez add [title] \"body\"", "create with content");
    cmd("notez add --in <dir>", "create inside a subdirectory (bare --in: fzf picker)");
    cmd("notez -p add [title]", "create private note");
    cmd("notez -g add [title]", "create global note");
    cmd("notez quick [title]", "private quick note (same as add quick)");
    cmd("notez edit [term]", "open an existing note (fuzzy match)");
    cmd("--no-sync", "skip pull on open (tree, todo, edit) and sync on exit (also logz, add)");
    cmd("notez rename [term] [title]", "retitle a note, keeping its date prefix");
    println!();

    println!("  {}", mauve.apply_to("Daily Logs"));
    cmd("notez log <message>", "append to today's log");
    cmd("notez logz / logs", "browse daily logs");
    println!();

    println!("  {}", mauve.apply_to("Todos"));
    cmd("notez todo", "interactive todo manager (TUI)");
    cmd("notez todo \"item\"", "quick-add a todo");
    println!();

    println!("  {}", mauve.apply_to("Tree Browser"));
    cmd("notez", "open the browser on the current scope");
    cmd("notez tree / treez", "interactive tree browser (TUI)");
    cmd("notez nav", "pick a vault directory and open it");
    cmd("notez search <term>", "search content");
    cmd("notez mkdir <name>", "create a subdirectory");
    println!();

    println!("  {}", mauve.apply_to("Projects"));
    cmd("notez attach [name]", "register this project on this machine");
    cmd("notez detach <name>", "unregister a project");
    cmd("notez list", "list registered projects");
    println!();

    println!("  {}", mauve.apply_to("Sync"));
    cmd("notez sync", "commit changes, git pull --rebase && git push the global root");
    println!();

    println!("  {}", mauve.apply_to("Setup"));
    cmd("notez setup", "create default config");
    cmd("notez migrate-from-legacy", "one-time import of the old notez-cli layout");
    cmd("notez completions <shell>", "generate shell completions");
    cmd("notez init <shell>", "shell-integration eval snippet");
    println!();

    println!("  {}", mauve.apply_to("Alias binaries"));
    cmd(
        "todoz zlog znote treez logz",
        "standalone names for the same commands",
    );
    cmd("editz findz zlogs", "(installed as symlinks to notez)");
    println!();

    println!(
        "  {}",
        overlay.apply_to("Scope flags (accessibility x binding):"),
    );
    println!(
        "    {} {}",
        sapphire.apply_to("(default)"),
        overlay.apply_to("public+project:   ./notez/ (committed with the repo; ~/notez/ outside a repo)"),
    );
    println!(
        "    {} {}",
        sapphire.apply_to("-p"),
        overlay.apply_to("personal+project: ~/notez/personal/<project>/ (private, syncs across your machines)"),
    );
    println!(
        "    {} {}",
        sapphire.apply_to("-g"),
        overlay.apply_to("personal+global:  ~/notez/ (your notez repo, notes bound to no project)"),
    );
    println!(
        "    {} {}",
        sapphire.apply_to("-l"),
        overlay.apply_to("scratch:          ./.notez/ (gitignored, this machine only, never syncs)"),
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    /// A temp vault with one pending note, tracking a local bare remote.
    fn vault_with_pending_note() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let vault = tmp.path().join("vault");
        std::fs::create_dir(&remote).unwrap();
        std::fs::create_dir(&vault).unwrap();
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "t@example.com"],
            &["config", "user.name", "t"],
            &["config", "commit.gpgsign", "false"],
        ] {
            git(&vault, args);
        }
        std::fs::write(vault.join("a.md"), "base\n").unwrap();
        git(&vault, &["add", "-A"]);
        git(&vault, &["commit", "-q", "-m", "seed"]);
        git(&vault, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git(&vault, &["push", "-q", "-u", "origin", "main"]);
        std::fs::write(vault.join("new.md"), "pending\n").unwrap();
        (tmp, vault, remote)
    }

    fn config_for(vault: &Path) -> Config {
        let mut config = Config::defaults();
        config.paths.notez_root = vault.to_str().unwrap().to_string();
        config
    }

    #[test]
    fn a_stopped_pull_skips_the_exit_sync() {
        let (_tmp, vault, remote) = vault_with_pending_note();
        let before = git(&remote, &["rev-parse", "main"]);

        let result = synced_after(
            Ok(()),
            &config_for(&vault),
            true,
            Some("vault pull hit a conflict, rebase aborted"),
        );

        assert!(result.is_ok());
        assert!(!git(&vault, &["status", "--porcelain"]).is_empty(), "nothing committed");
        assert_eq!(git(&remote, &["rev-parse", "main"]), before, "nothing pushed");
    }

    #[test]
    fn without_a_pull_stop_the_exit_sync_pushes() {
        let (_tmp, vault, remote) = vault_with_pending_note();
        let before = git(&remote, &["rev-parse", "main"]);

        let result = synced_after(Ok(()), &config_for(&vault), true, None);

        assert!(result.is_ok());
        assert!(git(&vault, &["status", "--porcelain"]).is_empty());
        assert_ne!(git(&remote, &["rev-parse", "main"]), before);
    }
}
