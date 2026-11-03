//! `notez add`, `znote` and `notez quick`: create a new note.
//!
//! A plain note lands in the root of the current scope. A quick note
//! (`notez quick`, or `notez add quick ...`) lands in `00_quick-notes/` and
//! is private: the default public scope is swapped for personal, while
//! `-g` and `-l` still apply.
//!
//! `--in <dir>` targets a subdirectory instead: under the global root by
//! default, under the current scope's root with `--in-local` (legacy
//! semantics). Bare `--in` opens an fzf picker over the existing
//! folders of that root, nested ones included. A first title word ending in
//! `/` is the same as `--in` (see [`take_dir_shorthand`]).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cli;
use notez_core::config::Config;
use notez_core::core::{Note, Scope, project, resolve};
use notez_core::util::sanitize;

/// Result of `add`: where the note landed and whether an inline body was
/// given. Without a body the editor opens on the fresh note (legacy UX);
/// with one, the file is written silently.
pub struct Created {
    pub path: PathBuf,
    pub had_body: bool,
}

/// Keyword that turns `notez add quick ...` into a quick note.
const QUICK_KEYWORD: &str = "quick";

/// Strip a leading `quick` keyword from `add`'s title words. Returns whether
/// it was present, i.e. whether the note is a quick note.
pub fn take_quick_keyword(title_words: &mut Vec<String>) -> bool {
    let is_quick = title_words.first().is_some_and(|w| w == QUICK_KEYWORD);
    if is_quick {
        title_words.remove(0);
    }
    is_quick
}

/// Take the `dir/` shorthand out of `add`'s title words: a first word that
/// ends in `/` and holds no space is the target directory, exactly as
/// `--in <dir>` (`work/meetings/` -> `work/meetings`). A lone `/` gives the
/// empty target, which opens the picker like a bare `--in`. A quoted title
/// stays a title (`"and/or/ notes"`, `"read up on tcp/"`).
pub fn take_dir_shorthand(title_words: &mut Vec<String>) -> Option<String> {
    let first = title_words.first()?;
    if !first.ends_with('/') || first.contains(char::is_whitespace) {
        return None;
    }
    let word = title_words.remove(0);
    Some(word.trim_matches('/').to_string())
}

/// Write the new note to disk and return its absolute path.
pub fn run(
    title_words: Vec<String>,
    in_arg: Option<String>,
    in_local: bool,
    is_quick: bool,
    scope: Scope,
    config: &Config,
) -> Result<Created> {
    let pick = |names: &[String]| fzf_pick(names, config);
    run_with_picker(
        title_words,
        in_arg,
        in_local,
        is_quick,
        scope,
        config,
        &pick,
    )
}

/// [`run`] with the directory picker passed in: it gets the nested folder
/// list and returns the chosen one.
fn run_with_picker(
    title_words: Vec<String>,
    in_arg: Option<String>,
    in_local: bool,
    is_quick: bool,
    scope: Scope,
    config: &Config,
    pick: &dyn Fn(&[String]) -> Result<String>,
) -> Result<Created> {
    let dir = match &in_arg {
        None if is_quick => resolve::quick_notes(quick_scope(scope), config)?,
        None => resolve::root(scope, config)?,
        Some(_) if is_quick => bail!("quick notes always go to quick-notes; drop --in"),
        Some(target) => {
            let root = in_root(in_local, scope, config)?;
            if target.is_empty() {
                pick_directory(&root, pick)?
            } else {
                resolve_target_dir(&root, target)?
            }
        }
    };
    create_in_dir(title_words, &dir, scope, is_quick)
}

/// Create a note in an explicit directory: the creation path shared by
/// `notez add` and the tree browser's new-note prompt. The first title word
/// group is the title ("untitled" when empty), the rest an optional body, as
/// for `add`. The file is named after the title, with a `YYYY-MM-DD-` prefix
/// only when `dated` (quick notes). Creates `dir` if missing, gitignores the
/// scratch store for [`Scope::Local`], and never overwrites an existing file
/// (`-2`, `-3`, ...).
pub fn create_in_dir(
    title_words: Vec<String>,
    dir: &Path,
    scope: Scope,
    dated: bool,
) -> Result<Created> {
    let (title, body) = cli::split_title_body(title_words);
    let title = title.unwrap_or_else(|| "untitled".to_string());
    let had_body = body.is_some();
    let note = Note::new(title, body);

    std::fs::create_dir_all(dir)
        .with_context(|| format!("failed to create note dir {}", dir.display()))?;
    if scope == Scope::Local {
        project::ensure_scratch_gitignored(dir);
    }

    let path = create_new_note_file(dir, &note.filename(dated), &note.rendered())?;

    Ok(Created { path, had_body })
}

/// Upper bound on same-name notes in one directory before giving up.
const MAX_NAME_ATTEMPTS: u32 = 1000;

/// Write `contents` to the first free name in `dir`, starting from `natural`
/// and then trying `suffixed_name` candidates. Never touches an existing file:
/// `create_new` makes the existence check and the creation one atomic step,
/// so a file that appears concurrently is skipped rather than truncated.
fn create_new_note_file(dir: &Path, natural: &str, contents: &str) -> Result<PathBuf> {
    use std::io::{ErrorKind, Write};

    for n in 1..=MAX_NAME_ATTEMPTS {
        let path = dir.join(suffixed_name(natural, n));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(e).with_context(|| format!("failed to write note {}", path.display()));
            }
        };
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write note {}", path.display()))?;
        return Ok(path);
    }
    bail!(
        "no free file name for {natural} in {} after {MAX_NAME_ATTEMPTS} attempts",
        dir.display()
    )
}

/// Candidate file name for the `n`th note with the same natural name:
/// `n == 1` is the name itself, later ones insert `-n` before the extension
/// (`2026-10-06-call-the-bank-2.md`).
fn suffixed_name(natural: &str, n: u32) -> String {
    if n <= 1 {
        return natural.to_string();
    }
    match natural.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{n}.{ext}"),
        None => format!("{natural}-{n}"),
    }
}

/// Quick notes are private: the default public scope becomes personal.
fn quick_scope(scope: Scope) -> Scope {
    match scope {
        Scope::Public => Scope::Personal,
        other => other,
    }
}

/// Open a freshly created note in the configured editor (nvim lands on the
/// body line in insert mode via `new_note_args`). Best-effort: the note is
/// already on disk, so a missing editor warns instead of failing the add.
pub fn open_created(path: &Path, config: &Config) {
    let status = std::process::Command::new(&config.editor.command)
        .args(&config.editor.new_note_args)
        .arg(path)
        .status();
    if let Err(e) = status {
        eprintln!(
            "notez: could not open editor {:?}: {e}; note is at {}",
            config.editor.command,
            path.display(),
        );
    }
}

/// The root `--in` resolves under: global by default, the current scope's
/// root with `--in-local` (mirrors legacy, where "local" meant the project
/// store; the scope flags pick which one).
fn in_root(in_local: bool, scope: Scope, config: &Config) -> Result<PathBuf> {
    let root_scope = if in_local { scope } else { Scope::Global };
    resolve::root(root_scope, config)
}

/// Resolve `--in <target>` to a directory under `root`: an existing subdir
/// as-is, else segment by segment, each an existing child or the first
/// child whose name contains it (case-insensitive, legacy fuzzy matching).
/// From the first segment that matches nothing, the rest is created fresh,
/// the first segment sanitized. A lone `.` is `root` itself; any other
/// empty, `.` or `..` segment is refused, so a target never leaves `root`.
fn resolve_target_dir(root: &Path, target: &str) -> Result<PathBuf> {
    let trimmed = target.trim_end_matches('/');
    if trimmed == "." {
        return Ok(root.to_path_buf());
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.iter().any(|s| matches!(*s, "" | "." | "..")) {
        bail!("invalid --in directory name: {target:?}");
    }

    let joined = root.join(trimmed);
    if joined.is_dir() {
        return Ok(joined);
    }

    let mut base = root.to_path_buf();
    for (i, segment) in segments.iter().enumerate() {
        if let Some(child) = matching_child(&base, segment)? {
            base = child;
            continue;
        }
        if i == 0 {
            let cleaned = sanitize::name(segment);
            if cleaned.is_empty() {
                bail!("invalid --in directory name: {target:?}");
            }
            base = base.join(cleaned);
        } else {
            base = base.join(segment);
        }
        return Ok(segments[i + 1..].iter().fold(base, |dir, s| dir.join(s)));
    }
    Ok(base)
}

/// The child dir of `dir` named `segment`, else the first one (sorted) whose
/// name contains it, case-insensitive.
fn matching_child(dir: &Path, segment: &str) -> Result<Option<PathBuf>> {
    let exact = dir.join(segment);
    if exact.is_dir() {
        return Ok(Some(exact));
    }
    let needle = segment.to_lowercase();
    Ok(subdirs(dir)?
        .into_iter()
        .find(|name| name.to_lowercase().contains(&needle))
        .map(|name| dir.join(name)))
}

/// Bare `--in` or a lone `/`: pick one of the nested folders of `root`.
fn pick_directory(root: &Path, pick: &dyn Fn(&[String]) -> Result<String>) -> Result<PathBuf> {
    let names = nested_subdirs(root);
    if names.is_empty() {
        bail!(
            "no subdirectories under {} to pick from; create one with `notez mkdir <name>`",
            root.display()
        );
    }
    Ok(root.join(pick(&names)?))
}

/// The fzf directory picker behind [`pick_directory`].
fn fzf_pick(names: &[String], config: &Config) -> Result<String> {
    if !config.tools.fzf {
        bail!(
            "the directory picker (bare --in or `/`) needs fzf; pass a name instead: \
             --in <dir> or <dir>/"
        );
    }

    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("fzf")
        .args(["--prompt", "directory> "])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to launch fzf")?;
    child
        .stdin
        .as_mut()
        .expect("stdin was piped")
        .write_all(names.join("\n").as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("directory picker cancelled");
    }
    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        bail!("directory picker cancelled");
    }
    Ok(selected)
}

/// The todo store, never a place for a note.
const TODOS_DIR: &str = "_todos";

/// Every visible folder under `root` as a relative path, parents before
/// their children, siblings sorted (`work`, `work/meetings`, `zeta`). Skips
/// dot-dirs and `_todos`, and lists symlinked folders without entering them.
fn nested_subdirs(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    collect_subdirs(root, "", &mut found);
    found
}

fn collect_subdirs(dir: &Path, prefix: &str, found: &mut Vec<String>) {
    for name in subdirs(dir).unwrap_or_default() {
        if prefix.is_empty() && name == TODOS_DIR {
            continue;
        }
        let path = dir.join(&name);
        let relative = format!("{prefix}{name}");
        found.push(relative.clone());
        if !path.is_symlink() {
            collect_subdirs(&path, &format!("{relative}/"), found);
        }
    }
}

/// Immediate visible subdirectories of `root`, sorted.
fn subdirs(root: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = match std::fs::read_dir(root) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    Ok(names)
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

    fn git_init(dir: &std::path::Path) {
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir)
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
    }

    #[test]
    fn add_global_writes_into_scope_root() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let path = run(
            vec!["my".into(), "first".into(), "note".into()],
            None,
            false,
            false,
            Scope::Global,
            &config,
        )
        .unwrap()
        .path;

        assert!(path.exists());
        assert_eq!(path.parent().unwrap(), dir.path());

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# my first note"));
    }

    #[test]
    fn quick_global_writes_into_quick_notes() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let path = run(vec!["idea".into()], None, false, true, Scope::Global, &config)
            .unwrap()
            .path;

        assert_eq!(path.parent().unwrap(), dir.path().join("00_quick-notes"));
    }

    #[test]
    #[serial_test::serial]
    fn add_local_writes_under_dot_notez_in_cwd() {
        let cwd_holder = tempdir().unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd_holder.path()).unwrap();

        let result = run(
            vec!["hello".into()],
            None,
            false,
            false,
            Scope::Local,
            &Config::defaults(),
        );

        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap().path;
        assert!(path.exists());
        assert!(path.parent().unwrap().ends_with(".notez"), "got {:?}", path);
    }

    #[test]
    #[serial_test::serial]
    fn add_personal_falls_back_to_global_outside_git() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());

        let cwd = tempdir().unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();

        let result = run(vec!["hi".into()], None, false, false, Scope::Personal, &config);

        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap().path;
        // No git project => personal falls back to the global notez_root.
        assert_eq!(path.parent().unwrap(), notez_root.path());
    }

    #[test]
    #[serial_test::serial]
    fn add_personal_inside_git_uses_personal_subdir() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());

        let project_dir = tempdir().unwrap();
        git_init(project_dir.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_dir.path()).unwrap();
        let result = run(vec!["note".into()], None, false, false, Scope::Personal, &config);
        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap().path;
        let parent = path.parent().unwrap();
        assert_eq!(parent.parent().unwrap(), notez_root.path().join("personal"));
    }

    #[test]
    #[serial_test::serial]
    fn add_public_inside_git_writes_into_project_notez_root() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());
        let project_dir = tempdir().unwrap();
        git_init(project_dir.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_dir.path()).unwrap();
        let result = run(vec!["plan".into()], None, false, false, Scope::Public, &config);
        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap().path;
        let expected = project_dir.path().canonicalize().unwrap().join("notez");
        assert_eq!(path.parent().unwrap().canonicalize().unwrap(), expected);
    }

    #[test]
    #[serial_test::serial]
    fn quick_in_public_scope_is_private() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());
        let project_dir = tempdir().unwrap();
        git_init(project_dir.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_dir.path()).unwrap();
        let result = run(vec!["jot".into()], None, false, true, Scope::Public, &config);
        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap().path;
        assert!(path.starts_with(notez_root.path().join("personal")), "got {:?}", path);
        assert!(path.parent().unwrap().ends_with("00_quick-notes"));
        assert!(!project_dir.path().join("notez").exists());
    }

    #[test]
    fn plain_note_is_named_after_its_title_without_a_date() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let created =
            run(words("Call the Bank"), None, false, false, Scope::Global, &config).unwrap();

        assert_eq!(file_name(&created.path), "call-the-bank.md");
        let body = std::fs::read_to_string(&created.path).unwrap();
        assert!(body.contains("Date: "), "the date lives inside the note");
    }

    #[test]
    fn quick_note_name_starts_with_todays_date() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let created =
            run(vec!["idea".into()], None, false, true, Scope::Global, &config).unwrap();

        let today = chrono::Local::now().format("%Y-%m-%d");
        assert_eq!(file_name(&created.path), format!("{today}-idea.md"));
    }

    #[test]
    fn note_created_in_a_directory_is_dated_only_when_asked() {
        let root = tempdir().unwrap();

        let plain = create_in_dir(words("ideas"), root.path(), Scope::Global, false).unwrap();
        let dated = create_in_dir(words("ideas"), root.path(), Scope::Global, true).unwrap();

        assert_eq!(file_name(&plain.path), "ideas.md");
        let today = chrono::Local::now().format("%Y-%m-%d");
        assert_eq!(file_name(&dated.path), format!("{today}-ideas.md"));
    }

    #[test]
    fn quick_rejects_in_arg() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let result = run(vec![], Some("ideas".into()), false, true, Scope::Global, &config);
        assert!(result.is_err());
    }

    #[test]
    fn quick_keyword_is_stripped_from_title() {
        let mut words = vec!["quick".to_string(), "my".into(), "idea".into()];
        assert!(take_quick_keyword(&mut words));
        assert_eq!(words, vec!["my".to_string(), "idea".into()]);
    }

    #[test]
    fn quick_keyword_only_counts_as_first_word() {
        let mut words = vec!["a".to_string(), "quick".into(), "fix".into()];
        assert!(!take_quick_keyword(&mut words));
        assert_eq!(words.len(), 3);

        let mut empty: Vec<String> = vec![];
        assert!(!take_quick_keyword(&mut empty));
    }

    #[test]
    fn add_with_body_includes_body() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let created = run(
            vec!["title".into(), "this is the body".into()],
            None,
            false,
            false,
            Scope::Global,
            &config,
        )
        .unwrap();
        assert!(created.had_body);
        let body = std::fs::read_to_string(&created.path).unwrap();
        assert!(body.contains("# title"));
        assert!(body.contains("this is the body"));
    }

    #[test]
    fn empty_title_becomes_untitled() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let created = run(vec![], None, false, false, Scope::Global, &config).unwrap();
        assert!(!created.had_body, "no inline body: the editor should open");
        let body = std::fs::read_to_string(&created.path).unwrap();
        assert!(body.starts_with("# untitled\n"));
    }

    #[test]
    fn in_arg_uses_existing_subdir() {
        let root = tempdir().unwrap();
        std::fs::create_dir(root.path().join("research")).unwrap();
        let dir = resolve_target_dir(root.path(), "research").unwrap();
        assert_eq!(dir, root.path().join("research"));
    }

    #[test]
    fn in_arg_substring_matches_existing_subdir() {
        let root = tempdir().unwrap();
        std::fs::create_dir(root.path().join("jobbansokningar")).unwrap();
        let dir = resolve_target_dir(root.path(), "jobb").unwrap();
        assert_eq!(dir, root.path().join("jobbansokningar"));
    }

    #[test]
    fn in_arg_creates_missing_subdir_sanitized() {
        let root = tempdir().unwrap();
        let dir = resolve_target_dir(root.path(), "New Ideas").unwrap();
        assert_eq!(dir, root.path().join("new-ideas"));
    }

    #[test]
    fn in_arg_resolves_nested_remainder_under_match() {
        let root = tempdir().unwrap();
        std::fs::create_dir(root.path().join("reference")).unwrap();
        let dir = resolve_target_dir(root.path(), "ref/Kotlin").unwrap();
        assert_eq!(dir, root.path().join("reference").join("Kotlin"));
    }

    #[test]
    fn add_with_in_writes_into_subdir() {
        let root = tempdir().unwrap();
        let config = config_in(root.path());
        std::fs::create_dir(root.path().join("ideas")).unwrap();

        let path = run(
            vec!["spark".into()],
            Some("ideas".into()),
            false,
            false,
            Scope::Global,
            &config,
        )
        .unwrap()
        .path;
        assert_eq!(path.parent().unwrap(), root.path().join("ideas"));
    }

    fn word_list(ws: &[&str]) -> Vec<String> {
        ws.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn trailing_slash_word_is_the_target_dir() {
        let mut title = word_list(&["work/meetings/", "standup", "notes"]);
        assert_eq!(take_dir_shorthand(&mut title), Some("work/meetings".into()));
        assert_eq!(title, word_list(&["standup", "notes"]));
    }

    #[test]
    fn word_without_trailing_slash_stays_in_the_title() {
        for ws in [&["work", "x"][..], &["work/x", "y"], &["x", "work/"], &[]] {
            let mut title = word_list(ws);
            assert_eq!(take_dir_shorthand(&mut title), None, "{ws:?}");
            assert_eq!(title, word_list(ws));
        }
    }

    #[test]
    fn quoted_title_with_a_slash_word_stays_a_title() {
        for quoted in ["and/or/ tradeoffs", "read up on tcp/"] {
            let mut title = word_list(&[quoted]);
            assert_eq!(take_dir_shorthand(&mut title), None, "{quoted:?}");
            assert_eq!(title, word_list(&[quoted]));
        }
    }

    #[test]
    fn lone_slash_asks_for_the_picker() {
        let mut title = word_list(&["/", "standup"]);
        assert_eq!(take_dir_shorthand(&mut title), Some(String::new()));
        assert_eq!(title, word_list(&["standup"]));
    }

    #[test]
    fn shorthand_writes_where_in_does() {
        let root = tempdir().unwrap();
        let config = config_in(root.path());
        std::fs::create_dir_all(root.path().join("work/meetings")).unwrap();
        let add = |title_words: Vec<String>, in_arg: Option<String>| {
            run(title_words, in_arg, false, false, Scope::Global, &config)
                .unwrap()
                .path
        };

        let mut title = word_list(&["work/meetings/", "standup"]);
        let target = take_dir_shorthand(&mut title);
        let by_shorthand = add(title, target);
        std::fs::remove_file(&by_shorthand).unwrap();
        let by_in = add(word_list(&["standup"]), Some("work/meetings".into()));

        assert_eq!(by_shorthand, by_in);
        assert_eq!(by_in, root.path().join("work/meetings/standup.md"));
    }

    #[test]
    fn empty_target_runs_the_injected_picker_over_nested_folders() {
        let root = tempdir().unwrap();
        let config = config_in(root.path());
        std::fs::create_dir_all(root.path().join("work/meetings")).unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        let pick = |names: &[String]| {
            seen.replace(names.to_vec());
            Ok("work/meetings".to_string())
        };

        let mut title = word_list(&["/", "standup"]);
        let target = take_dir_shorthand(&mut title);
        let path = run_with_picker(title, target, false, false, Scope::Global, &config, &pick)
            .unwrap()
            .path;

        assert_eq!(seen.into_inner(), word_list(&["work", "work/meetings"]));
        assert_eq!(path, root.path().join("work/meetings/standup.md"));
    }

    #[test]
    fn nested_subdirs_list_parents_first_and_skip_dot_dirs_and_todos() {
        let root = tempdir().unwrap();
        for dir in [
            "zeta",
            "work/meetings",
            "work/archive/2025",
            "work/.hidden",
            "work-log",
            ".git/objects",
            "_todos/home",
            "ideas/_todos",
        ] {
            std::fs::create_dir_all(root.path().join(dir)).unwrap();
        }
        std::fs::write(root.path().join("work/note.md"), "").unwrap();

        assert_eq!(
            nested_subdirs(root.path()),
            word_list(&[
                "ideas",
                "ideas/_todos",
                "work",
                "work/archive",
                "work/archive/2025",
                "work/meetings",
                "work-log",
                "zeta",
            ])
        );
    }

    #[test]
    fn in_arg_fuzzy_matches_each_segment() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("reference/kotlin-notes")).unwrap();
        let dir = resolve_target_dir(root.path(), "ref/Kotlin/coroutines").unwrap();
        assert_eq!(dir, root.path().join("reference/kotlin-notes/coroutines"));
    }

    #[test]
    fn in_arg_prefers_an_exact_segment_over_a_substring_match() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("homework")).unwrap();
        std::fs::create_dir_all(root.path().join("work")).unwrap();
        let dir = resolve_target_dir(root.path(), "work/new").unwrap();
        assert_eq!(dir, root.path().join("work/new"));
    }

    #[test]
    fn in_arg_never_leaves_the_root() {
        let parent = tempdir().unwrap();
        let root = parent.path().join("notez");
        std::fs::create_dir_all(root.join("a")).unwrap();
        for target in [
            "../x", "a/../x", "..", "../", "a/./x", "a//x", "/x", "", "/",
        ] {
            let err = resolve_target_dir(&root, target).unwrap_err();
            assert!(
                err.to_string().contains("invalid --in directory name"),
                "{target:?}: {err}"
            );
        }
        let mut entries: Vec<_> = std::fs::read_dir(parent.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        entries.sort();
        assert_eq!(entries, ["notez"], "nothing created beside the root");
    }

    #[test]
    fn in_arg_lone_dot_is_the_root() {
        let root = tempdir().unwrap();
        assert_eq!(resolve_target_dir(root.path(), ".").unwrap(), root.path());
        let config = config_in(root.path());
        let path = run(
            word_list(&["tcp/"]),
            Some(".".into()),
            false,
            false,
            Scope::Global,
            &config,
        )
        .unwrap()
        .path;
        assert_eq!(path.parent().unwrap(), root.path());
    }

    #[cfg(unix)]
    #[test]
    fn nested_subdirs_list_a_symlink_loop_once_without_entering_it() {
        let root = tempdir().unwrap();
        std::fs::create_dir(root.path().join("a")).unwrap();
        std::os::unix::fs::symlink(root.path().join("a"), root.path().join("a/loop")).unwrap();
        assert_eq!(nested_subdirs(root.path()), word_list(&["a", "a/loop"]));
    }

    #[test]
    #[serial_test::serial]
    fn add_local_gitignores_scratch_store_in_git_repo() {
        let cwd_holder = tempdir().unwrap();
        git_init(cwd_holder.path());
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd_holder.path()).unwrap();

        let result = run(
            vec!["scratch".into()],
            None,
            false,
            false,
            Scope::Local,
            &Config::defaults(),
        );

        std::env::set_current_dir(saved).unwrap();
        result.unwrap();
        let gitignore =
            std::fs::read_to_string(cwd_holder.path().join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == ".notez"));
    }

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    fn file_name(path: &Path) -> String {
        path.file_name().unwrap().to_string_lossy().into_owned()
    }

    /// Create the same note twice after the user edited the first one, and
    /// check that the edit survives and the second note is freshly rendered.
    fn assert_repeat_does_not_overwrite(
        title_words: Vec<String>,
        is_quick: bool,
        expected_stem: &str,
    ) {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let first = run(title_words.clone(), None, false, is_quick, Scope::Global, &config)
            .unwrap()
            .path;
        let fresh = std::fs::read_to_string(&first).unwrap();
        std::fs::write(&first, "user edits, keep me\n").unwrap();

        let second = run(title_words, None, false, is_quick, Scope::Global, &config)
            .unwrap()
            .path;

        assert_ne!(first, second);
        assert_eq!(first.parent(), second.parent());
        let (first_name, second_name) = (file_name(&first), file_name(&second));
        if is_quick {
            assert!(first_name.ends_with(&format!("-{expected_stem}.md")), "got {first_name}");
            assert!(second_name.ends_with(&format!("-{expected_stem}-2.md")), "got {second_name}");
        } else {
            assert_eq!(first_name, format!("{expected_stem}.md"));
            assert_eq!(second_name, format!("{expected_stem}-2.md"));
        }
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "user edits, keep me\n");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), fresh);
    }

    #[test]
    fn repeat_title_never_overwrites_edited_note() {
        assert_repeat_does_not_overwrite(words("call the bank"), false, "call-the-bank");
    }

    #[test]
    fn repeat_quick_note_never_overwrites_edited_note() {
        assert_repeat_does_not_overwrite(words("idea"), true, "idea");
    }

    #[test]
    fn repeat_untitled_note_never_overwrites_edited_note() {
        assert_repeat_does_not_overwrite(vec![], false, "untitled");
    }

    #[test]
    fn repeat_note_with_body_never_overwrites_edited_note() {
        assert_repeat_does_not_overwrite(
            vec!["title".into(), "this is the body".into()],
            false,
            "title",
        );
    }

    #[test]
    fn third_repeat_skips_existing_suffix_two() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let first = run(words("call the bank"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        let name = file_name(&first);
        let taken = dir
            .path()
            .join(name.replace("call-the-bank.md", "call-the-bank-2.md"));
        std::fs::write(&taken, "pre-existing two\n").unwrap();

        let third = run(words("call the bank"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&third), "call-the-bank-3.md");
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "pre-existing two\n");

        let fourth = run(words("call the bank"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&fourth), "call-the-bank-4.md");
    }

    #[test]
    fn title_ending_in_number_never_causes_overwrite() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let numbered = run(words("plan 2"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&numbered), "plan-2.md");
        std::fs::write(&numbered, "plan two edits\n").unwrap();

        let plain = run(words("plan"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&plain), "plan.md");
        std::fs::write(&plain, "plan edits\n").unwrap();

        let repeat = run(words("plan"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&repeat), "plan-3.md");
        assert!(std::fs::read_to_string(&repeat).unwrap().starts_with("# plan\n"));

        let numbered_again = run(words("plan 2"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert_eq!(file_name(&numbered_again), "plan-2-2.md");

        assert_eq!(std::fs::read_to_string(&numbered).unwrap(), "plan two edits\n");
        assert_eq!(std::fs::read_to_string(&plain).unwrap(), "plan edits\n");
    }

    /// The browser creates through `create_in_dir` in the scope root; that
    /// must land beside what `notez add` writes for the same title and
    /// scope, with the same content, never overwriting it.
    #[test]
    #[serial_test::serial]
    fn create_in_dir_matches_add_for_every_scope() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());
        let project_dir = tempdir().unwrap();
        git_init(project_dir.path());
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_dir.path()).unwrap();

        let mut results = Vec::new();
        for scope in [Scope::Global, Scope::Personal, Scope::Public, Scope::Local] {
            let added = run(words("call the bank"), None, false, false, scope, &config);
            let dir = resolve::root(scope, &config);
            let created = dir
                .as_ref()
                .map_err(|e| anyhow::anyhow!("{e}"))
                .and_then(|d| create_in_dir(words("call the bank"), d, scope, false));
            results.push((scope, added, dir, created));
        }
        std::env::set_current_dir(saved).unwrap();

        for (scope, added, dir, created) in results {
            let (added, dir, created) = (added.unwrap().path, dir.unwrap(), created.unwrap().path);
            assert_eq!(added.parent().unwrap(), dir, "{scope:?}");
            assert_eq!(created.parent().unwrap(), dir, "{scope:?}");
            assert_eq!(file_name(&added), "call-the-bank.md", "{scope:?}");
            assert_eq!(file_name(&created), "call-the-bank-2.md", "{scope:?}");
            assert_eq!(
                std::fs::read_to_string(&added).unwrap(),
                std::fs::read_to_string(&created).unwrap(),
                "{scope:?}"
            );
        }
        let gitignore = std::fs::read_to_string(project_dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == ".notez"));
    }

    #[test]
    fn create_in_dir_defaults_to_untitled_and_creates_the_dir() {
        let root = tempdir().unwrap();
        let dir = root.path().join("ideas").join("new");

        let created = create_in_dir(vec![], &dir, Scope::Personal, false).unwrap();

        assert_eq!(created.path.parent().unwrap(), dir);
        assert!(!created.had_body);
        assert_eq!(file_name(&created.path), "untitled.md");
        let body = std::fs::read_to_string(&created.path).unwrap();
        assert!(body.starts_with("# untitled\n"));
    }

    #[test]
    fn suffixed_name_inserts_number_before_extension() {
        assert_eq!(suffixed_name("2026-10-06-a.md", 1), "2026-10-06-a.md");
        assert_eq!(suffixed_name("2026-10-06-a.md", 2), "2026-10-06-a-2.md");
        assert_eq!(suffixed_name("2026-10-06-a.md", 10), "2026-10-06-a-10.md");
    }
}
