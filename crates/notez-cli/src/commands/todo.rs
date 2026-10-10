//! `notez todo` / `todoz`: todo manager.
//!
//! `todoz "text"` quick-adds to the scope's TODO.md. Without an item the
//! interactive board TUI opens: `-g` shows the full aggregated board
//! (global + `_todos` categories + every registered project's scopes),
//! any other scope shows that scope's single TODO.md plus, when inside a
//! project, read-only TODOs scanned from the code.

use std::path::PathBuf;

use anyhow::{Context, Result};

use notez_core::config::{Config, ProjectRegistry};
use notez_core::core::{Project, Scope, resolve};
use notez_core::todo::{self, CheckState, Task};
use notez_core::util::tilde;

use crate::tui::header::SyncState;
use crate::tui::todo::{BoardContext, run_board};

/// Quick-add `item`, or open the board when there is none. `warning`, if any,
/// shows in the board's footer and `sync` in its header; a quick add ignores
/// both.
pub fn run(
    item: Option<String>,
    scope: Scope,
    config: &Config,
    warning: Option<&str>,
    sync: SyncState,
) -> Result<()> {
    match item {
        Some(text) => quick_add(text, scope, config),
        None => launch_tui(scope, config, warning, sync),
    }
}

/// Append `- [ ] <text>` to the scope's `TODO.md`, creating it (with a
/// `# TODO` header) if missing.
fn quick_add(text: String, scope: Scope, config: &Config) -> Result<()> {
    let dir = resolve::root(scope, config)?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;
    if scope == Scope::Local {
        notez_core::core::project::ensure_scratch_gitignored(&dir);
    }

    let path = dir.join("TODO.md");
    // Only a missing file starts fresh; an existing but unreadable one is
    // never replaced.
    let mut content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "# TODO\n".to_string(),
        Err(e) => {
            return Err(e).with_context(|| {
                format!("could not read {}, nothing was changed", path.display())
            });
        }
    };
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("- [ ] ");
    content.push_str(&text);
    content.push('\n');
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write {}", path.display()))?;

    println!("Added to {}", tilde::contract(&path));
    Ok(())
}

/// Assemble the board for the scope, run the TUI, persist only the files
/// the user actually changed (a quit with no edits writes nothing, and
/// untouched files keep any non-todo text they carry).
fn launch_tui(scope: Scope, config: &Config, warning: Option<&str>, sync: SyncState) -> Result<()> {
    let (items, mut ctx) = build_board(scope, config)?;
    ctx.warning = warning.map(str::to_string);
    ctx.sync = sync;
    let outcome = run_board(items, &ctx, config)?;
    todo::save_todos_for(&outcome.items, &outcome.dirty)
        .context("failed to save TODO.md files")?;
    Ok(())
}

fn build_board(scope: Scope, config: &Config) -> Result<(Vec<Task>, BoardContext)> {
    if scope == Scope::Global {
        let registry = ProjectRegistry::load().unwrap_or_default();
        let items = todo::load_board(config, &registry);
        let ctx = BoardContext {
            global: true,
            title: "todoz (global)".to_string(),
            path_display: tilde::contract(&config.notez_root_path()),
            warning: None,
            sync: SyncState::Off,
        };
        return Ok((items, ctx));
    }

    // Single-scope board: one synthesized section header + that scope's
    // TODO.md, so the TUI always has a section to add into.
    let root = resolve::root(scope, config)?;
    let path = root.join("TODO.md");
    let (name, label) = match Project::try_detect() {
        Some(p) => {
            let label = format!("{} ({})", p.name, scope.label());
            (p.name, label)
        }
        None => (scope.to_string(), scope.label().to_string()),
    };

    let mut items = vec![Task {
        text: label,
        state: CheckState::Unchecked,
        source: path.clone(),
        section: name.clone(),
        is_header: true,
        depth: 0,
        has_subtasks: false,
        collapsed: false,
        is_code_todo: false,
        flags: 0,
    }];
    items.extend(todo::load_single_todo(&path, &name));

    // Read-only TODOs scanned from the project's code, shown under their
    // own section. Never serialized (is_code_todo).
    let code_todos = scan_code_todos();
    if !code_todos.is_empty() {
        items.push(Task {
            text: format!("\u{f121} code TODOs  ({} found)", code_todos.len()),
            state: CheckState::Unchecked,
            source: PathBuf::new(),
            section: "code".to_string(),
            is_header: true,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: true,
            flags: 0,
        });
        items.extend(code_todos);
    }

    let ctx = BoardContext {
        global: false,
        title: format!("{} todoz ({})", scope.icon(), name),
        path_display: tilde::contract(&root),
        warning: None,
        sync: SyncState::Off,
    };
    Ok((items, ctx))
}

/// Scan the current directory's code for `TODO` comments via `rg` (fallback
/// `grep`) and return them as read-only board rows.
fn scan_code_todos() -> Vec<Task> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut results = Vec::new();

    let output = std::process::Command::new("rg")
        .args([
            "--line-number",
            "--no-heading",
            "--glob",
            "!.notez/**",
            "--glob",
            "!notez/**",
            "--glob",
            "!node_modules/**",
            "--glob",
            "!target/**",
            "--glob",
            "!.git/**",
            "--glob",
            "!*.md",
            r"(?://|#|--|/\*|<!--)\s*TODO\b",
        ])
        .current_dir(&cwd)
        .output();

    let lines = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => {
            let o = std::process::Command::new("grep")
                .args([
                    "-rn",
                    "--include=*.rs",
                    "--include=*.ts",
                    "--include=*.js",
                    "--include=*.py",
                    "--include=*.go",
                    "--include=*.java",
                    "--include=*.kt",
                    "--include=*.c",
                    "--include=*.cpp",
                    "--include=*.h",
                    "--include=*.sh",
                    "--include=*.toml",
                    "--include=*.yaml",
                    "--include=*.yml",
                    "TODO",
                ])
                .current_dir(&cwd)
                .output();
            match o {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                Err(_) => return results,
            }
        }
    };

    for line in lines.lines() {
        // rg/grep format: file:line:content
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let file = parts[0];
        let line_num = parts[1];
        let content = parts[2].trim();

        let todo_text = content
            .trim_start_matches("//")
            .trim_start_matches('#')
            .trim_start_matches("--")
            .trim_start_matches("/*")
            .trim_start_matches("<!--")
            .trim()
            .trim_start_matches("TODO")
            .trim_start_matches(':')
            .trim();

        let short_file = std::path::Path::new(file)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string());
        // Truncate on a char boundary; a byte slice can split a multi-byte
        // char (å, ö) and panic.
        let truncated: String = if todo_text.chars().count() > 60 {
            let head: String = todo_text.chars().take(60).collect();
            format!("{}\u{2026}", head.trim_end())
        } else {
            todo_text.to_string()
        };
        let display = if truncated.is_empty() {
            format!("{}:{}", short_file, line_num)
        } else {
            format!("{}:{} {}", short_file, line_num, truncated)
        };

        results.push(Task {
            text: display,
            state: CheckState::Unchecked,
            source: PathBuf::from(file),
            section: "code".to_string(),
            is_header: false,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: true,
            flags: 0,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_in(root: &std::path::Path) -> Config {
        let mut c = Config::defaults();
        c.paths.notez_root = root.to_string_lossy().into_owned();
        c
    }

    #[test]
    fn quick_add_creates_todo_with_header() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        quick_add("first item".into(), Scope::Global, &config).unwrap();

        let content = std::fs::read_to_string(dir.path().join("TODO.md")).unwrap();
        assert_eq!(content, "# TODO\n- [ ] first item\n");
    }

    #[test]
    fn quick_add_appends_without_clobbering() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::write(dir.path().join("TODO.md"), "# TODO\n- [x] done #prio").unwrap();

        quick_add("next".into(), Scope::Global, &config).unwrap();

        let content = std::fs::read_to_string(dir.path().join("TODO.md")).unwrap();
        assert_eq!(content, "# TODO\n- [x] done #prio\n- [ ] next\n");
    }

    #[test]
    fn quick_add_leaves_unreadable_todo_untouched_and_errors() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        let path = dir.path().join("TODO.md");
        let bytes = [0xff, 0xfe, 0x00, 0x41];
        std::fs::write(&path, bytes).unwrap();

        let err = quick_add("next".into(), Scope::Global, &config).unwrap_err();

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let message = format!("{err:#}");
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("nothing was changed"), "{message}");
    }

    #[test]
    fn global_board_builds_with_global_context() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::write(dir.path().join("TODO.md"), "# TODO\n\n- [ ] a\n").unwrap();

        let (items, ctx) = build_board(Scope::Global, &config).unwrap();
        assert!(ctx.global);
        assert!(items.iter().any(|t| t.is_header && t.text == "NOTEZ"));
        assert!(items.iter().any(|t| !t.is_header && t.text == "a"));
    }
}
