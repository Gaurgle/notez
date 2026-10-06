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
    let Some(cmd) = parsed.command else {
        return finish(commands::tree::run(scope, &config));
    };

    let result: anyhow::Result<()> = match cmd {
        Commands::Add { mut title, r#in, in_local }
        | Commands::Znote { mut title, r#in, in_local } => {
            let is_quick = commands::add::take_quick_keyword(&mut title);
            commands::add::run(title, r#in, in_local, is_quick, scope, &config)
                .map(|created| report_created(&created, &config))
        }
        Commands::Quick { title } => commands::add::run(title, None, false, true, scope, &config)
            .map(|created| report_created(&created, &config)),
        Commands::Log { message } | Commands::Zlog { message } => {
            commands::log::run(message, scope, &config).map(|p| {
                println!("Appended to: {}", p.display());
            })
        }
        Commands::Logz | Commands::Logs => commands::logz::run(scope, &config),
        Commands::Mkdir { name } => commands::mkdir::run(name, scope, &config).map(|p| {
            println!("Created: {}", p.display());
        }),
        Commands::Search { term } | Commands::Findz { term } => {
            commands::search::run(term, &config)
        }
        Commands::Tree | Commands::Treez => commands::tree::run(scope, &config),
        Commands::Setup => commands::setup::run(),
        Commands::Demo { view: _ } => Err(anyhow::anyhow!(
            "demo is not implemented; it was a screenshot helper in the legacy CLI"
        )),
        Commands::Completions { shell } => commands::completions::run(&shell),
        Commands::Init { shell } => commands::init::run(&shell),
        Commands::Todo { item } | Commands::Todoz { item } => {
            commands::todo::run(item, scope, &config)
        }
        Commands::Edit { term } | Commands::Editz { term } => {
            commands::edit::run(term, scope, &config)
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

fn report_created(created: &commands::add::Created, config: &Config) {
    println!("Created: {}", created.path.display());
    // Legacy UX: no inline body means "write it now" - open the editor on
    // the fresh note. With a body, stay silent.
    if !created.had_body {
        commands::add::open_created(&created.path, config);
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
