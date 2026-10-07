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
use crate::commands::tree::View;
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

    let in_project = Project::try_detect().is_some();
    let scope = Scope::from_flags(parsed.global, parsed.private, parsed.local, in_project);
    let has_scope_flag = parsed.has_scope_flag();
    let (nav, sync) = (parsed.nav, !parsed.no_sync);

    // Decided before the config loads, so a mistyped subcommand fails
    // without touching anything.
    let action = match decide(has_scope_flag, scope, in_project, parsed.command, parsed.words) {
        Ok(action) => action,
        Err(hint) => {
            eprintln!("notez: {hint}");
            return ExitCode::from(2);
        }
    };

    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e:#}");
            return ExitCode::FAILURE;
        }
    };

    if nav {
        return finish(commands::nav::run(&config));
    }

    // No subcommand: a bare `notez` (or `tree`) opens the browser on
    // everything, current project first; `-g` / `-p` / `-l` narrow it to
    // that scope, and a scope flag followed by words makes a quick note
    // there. See `decide`.
    let cmd = match action {
        Action::Browse(view) => return finish(browse(view, &config, sync)),
        Action::QuickNote(title) => {
            return finish(
                create_quick(title, scope, &config)
                    .map(|created| report_created(&created, &config, sync)),
            );
        }
        Action::Run(cmd) => cmd,
    };

    let result: anyhow::Result<()> = match cmd {
        Commands::Add { mut title, r#in, in_local }
        | Commands::Znote { mut title, r#in, in_local } => {
            let is_quick = commands::add::take_quick_keyword(&mut title);
            commands::add::run(title, r#in, in_local, is_quick, scope, &config)
                .map(|created| report_created(&created, &config, sync))
        }
        Commands::Quick { title } => create_quick(title, scope, &config)
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
        // `decide` turns these into `Action::Browse`; kept so the match stays
        // exhaustive without a panic.
        Commands::Tree | Commands::Treez => browse(
            View::for_flags(has_scope_flag, scope, in_project),
            &config,
            sync,
        ),
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

/// What an invocation asks for, decided from the parsed arguments alone.
#[derive(Debug)]
enum Action {
    /// Open the tree browser on this view.
    Browse(View),
    /// Create a quick note with these title words, as `notez quick` does.
    QuickNote(Vec<String>),
    /// Run this subcommand.
    Run(Commands),
}

/// Decide what to do with the parsed flags, the optional subcommand and the
/// free words. Free words with no scope flag are refused with a hint rather
/// than made into a note, so a mistyped subcommand never creates one.
fn decide(
    has_scope_flag: bool,
    scope: Scope,
    in_project: bool,
    command: Option<Commands>,
    words: Vec<String>,
) -> Result<Action, String> {
    let view = View::for_flags(has_scope_flag, scope, in_project);
    match command {
        Some(Commands::Tree | Commands::Treez) => Ok(Action::Browse(view)),
        Some(cmd) => Ok(Action::Run(cmd)),
        None if words.is_empty() => Ok(Action::Browse(view)),
        None if has_scope_flag => Ok(Action::QuickNote(words)),
        None => Err(format!(
            "unknown command `{}`. For a quick note use `notez quick <title>` \
             or a scope flag: `notez -p <title>` (also -g, -l)",
            words[0]
        )),
    }
}

/// Open the browser with the vault pulled before and synced after.
fn browse(view: View, config: &Config, sync: bool) -> anyhow::Result<()> {
    let stop = pulled_before(config, sync);
    let warning = footer_warning(stop.as_deref());
    let result = commands::tree::run(view, config, warning.as_deref());
    synced_after(result, config, sync, stop.as_deref())
}

/// Create a quick note. Shared by `notez quick` and a scope flag followed by
/// words, so both land in the same folder under the same file name.
fn create_quick(
    title: Vec<String>,
    scope: Scope,
    config: &Config,
) -> anyhow::Result<commands::add::Created> {
    commands::add::run(title, None, false, true, scope, config)
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
    cmd("notez -g|-p|-l <title>", "quick note in that scope (same as -g quick <title>)");
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
    cmd("notez", "open the browser on this project (global outside one)");
    cmd("notez tree / treez", "same browser; -g global, -p personal, -l scratch");
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

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("notez").chain(args.iter().copied()))
    }

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    /// `decide` for a parsed invocation, the way `main` calls it.
    fn decide_for(cli: Cli, in_project: bool) -> Result<Action, String> {
        let scope = Scope::from_flags(cli.global, cli.private, cli.local, in_project);
        decide(cli.has_scope_flag(), scope, in_project, cli.command, cli.words)
    }

    #[test]
    fn cli_definition_passes_clap_debug_asserts() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn scope_flag_then_words_parse_as_free_words() {
        let cli = parse(&["-g", "call", "the", "bank"]).unwrap();
        assert!(cli.global && cli.command.is_none());
        assert_eq!(cli.words, words(&["call", "the", "bank"]));

        let cli = parse(&["-g", "call the bank"]).unwrap();
        assert_eq!(cli.words, words(&["call the bank"]));
    }

    #[test]
    fn words_before_the_flag_parse_like_the_flag_first_form() {
        let cli = parse(&["call", "the", "bank", "-g"]).unwrap();
        assert!(cli.global && cli.command.is_none());
        assert_eq!(cli.words, words(&["call", "the", "bank"]));
    }

    #[test]
    fn a_subcommand_name_after_a_flag_runs_the_subcommand() {
        let cli = parse(&["-g", "tree"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Tree)));
        assert!(cli.words.is_empty());

        let cli = parse(&["-g", "quick", "tree"]).unwrap();
        assert!(matches!(&cli.command, Some(Commands::Quick { title }) if *title == words(&["tree"])));
    }

    #[test]
    fn dashed_words_need_a_double_dash() {
        assert!(parse(&["-g", "-x"]).is_err());
        let cli = parse(&["-g", "--", "-x", "marks"]).unwrap();
        assert_eq!(cli.words, words(&["-x", "marks"]));
    }

    #[test]
    fn existing_flags_and_subcommands_parse_as_before() {
        for args in [&["-h"][..], &["--help"], &["tree", "-h"], &["add", "--help"]] {
            assert!(parse(args).unwrap().help, "{args:?}");
        }
        let cli = parse(&["todo", "--no-sync", "-g"]).unwrap();
        assert!(cli.no_sync && cli.global && matches!(cli.command, Some(Commands::Todo { item: None })));
        assert!(parse(&["-n"]).unwrap().nav);
        assert!(parse(&["--nav"]).unwrap().nav);
        let cli = parse(&["log", "hello", "-p"]).unwrap();
        assert!(cli.private && matches!(&cli.command, Some(Commands::Log { message }) if *message == words(&["hello"])));
        assert!(matches!(
            parse(&["completions", "zsh"]).unwrap().command,
            Some(Commands::Completions { .. })
        ));
    }

    #[test]
    fn completions_still_generate() {
        use clap::CommandFactory;
        let mut out = Vec::new();
        clap_complete::generate(clap_complete::Shell::Zsh, &mut Cli::command(), "notez", &mut out);
        assert!(String::from_utf8(out).unwrap().contains("treez"));
    }

    #[test]
    fn alias_binaries_select_their_subcommand() {
        let alias = |argv: &[&str]| {
            let argv = rewrite_for_symlink(argv.iter().map(|s| s.to_string()).collect());
            Cli::try_parse_from(argv).unwrap().command
        };
        assert!(matches!(alias(&["/bin/todoz"]), Some(Commands::Todoz { item: None })));
        assert!(matches!(alias(&["/bin/zlog", "hi"]), Some(Commands::Zlog { .. })));
        assert!(matches!(alias(&["/bin/logz"]), Some(Commands::Logz)));
        assert!(matches!(alias(&["/bin/zlogs"]), Some(Commands::Logs)));
        assert!(matches!(alias(&["/bin/znote", "idea"]), Some(Commands::Znote { .. })));
        assert!(matches!(alias(&["/bin/treez", "-g"]), Some(Commands::Treez)));
        assert!(matches!(alias(&["/bin/editz"]), Some(Commands::Editz { term: None })));
        assert!(matches!(alias(&["/bin/findz", "x"]), Some(Commands::Findz { .. })));
    }

    #[test]
    fn no_flag_opens_the_all_view_inside_and_outside_a_project() {
        for args in [&[][..], &["tree"], &["treez"]] {
            let action = decide_for(parse(args).unwrap(), true).unwrap();
            assert!(matches!(action, Action::Browse(View::All)), "{args:?}");
            let action = decide_for(parse(args).unwrap(), false).unwrap();
            assert!(matches!(action, Action::Browse(View::All)), "{args:?}");
        }
    }

    #[test]
    fn personal_flag_outside_a_project_opens_the_all_view() {
        for args in [&["-p"][..], &["-p", "tree"]] {
            let action = decide_for(parse(args).unwrap(), false).unwrap();
            assert!(matches!(action, Action::Browse(View::All)), "{args:?}");
        }
    }

    #[test]
    fn a_scope_flag_alone_opens_that_scope() {
        let cases = [
            ("-g", View::Only(Scope::Global)),
            ("-p", View::Only(Scope::Personal)),
            ("-l", View::Only(Scope::Local)),
        ];
        for (flag, view) in cases {
            for args in [&[flag][..], &[flag, "tree"]] {
                let action = decide_for(parse(args).unwrap(), true).unwrap();
                assert!(matches!(action, Action::Browse(v) if v == view), "{args:?}");
            }
        }
    }

    #[test]
    fn words_without_a_scope_flag_are_refused_with_a_hint() {
        let err = decide_for(parse(&["somthing"]).unwrap(), true).unwrap_err();
        assert!(err.contains("`somthing`"), "{err}");
        assert!(err.contains("notez quick <title>"), "{err}");
        assert!(err.contains("notez -p <title>"), "{err}");
    }

    #[test]
    #[serial_test::serial]
    fn a_scope_flag_with_words_makes_the_same_note_as_quick() {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q"]);
        let config = config_for(root.path());
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let created = |args: &[&str]| {
            let cli = parse(args).unwrap();
            let scope = Scope::from_flags(cli.global, cli.private, cli.local, true);
            let title = match decide_for(cli, true).unwrap() {
                Action::QuickNote(title) => title,
                Action::Run(Commands::Quick { title }) => title,
                other => panic!("{args:?} gave {other:?}"),
            };
            create_quick(title, scope, &config).unwrap().path
        };
        let mut pairs = Vec::new();
        for flag in ["-g", "-p", "-l"] {
            let by_flag = created(&[flag, "call", "the", "bank"]);
            std::fs::remove_file(&by_flag).unwrap();
            let by_quick = created(&[flag, "quick", "call", "the", "bank"]);
            pairs.push((flag, by_flag, by_quick));
        }
        std::env::set_current_dir(saved).unwrap();

        for (flag, by_flag, by_quick) in pairs {
            assert_eq!(by_flag, by_quick, "{flag}");
            assert!(by_quick.exists(), "{flag}");
            assert!(by_quick.parent().unwrap().ends_with("00_quick-notes"), "{flag}");
        }
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
