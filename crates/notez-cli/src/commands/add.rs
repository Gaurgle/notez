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
//! subdirectories of that root.

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

/// Write the new note to disk and return its absolute path.
pub fn run(
    title_words: Vec<String>,
    in_arg: Option<String>,
    in_local: bool,
    is_quick: bool,
    scope: Scope,
    config: &Config,
) -> Result<Created> {
    let dir = match &in_arg {
        None if is_quick => resolve::quick_notes(quick_scope(scope), config)?,
        None => resolve::root(scope, config)?,
        Some(_) if is_quick => bail!("quick notes always go to quick-notes; drop --in"),
        Some(target) => {
            let root = in_root(in_local, scope, config)?;
            if target.is_empty() {
                pick_directory(&root, config)?
            } else {
                resolve_target_dir(&root, target)?
            }
        }
    };
    create_in_dir(title_words, &dir, scope)
}

/// Create a note in an explicit directory: the creation path shared by
/// `notez add` and the tree browser's new-note prompt. The first title word
/// group is the title ("untitled" when empty), the rest an optional body, as
/// for `add`. Creates `dir` if missing, gitignores the scratch store for
/// [`Scope::Local`], and never overwrites an existing file (`-2`, `-3`, ...).
pub fn create_in_dir(title_words: Vec<String>, dir: &Path, scope: Scope) -> Result<Created> {
    let (title, body) = cli::split_title_body(title_words);
    let title = title.unwrap_or_else(|| "untitled".to_string());
    let had_body = body.is_some();
    let note = Note::new(title, body);

    std::fs::create_dir_all(dir)
        .with_context(|| format!("failed to create note dir {}", dir.display()))?;
    if scope == Scope::Local {
        project::ensure_scratch_gitignored(dir);
    }

    let path = create_new_note_file(dir, &note.filename(), &note.rendered())?;

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
/// as-is, else a case-insensitive substring match on the first path segment
/// (legacy fuzzy matching), else the sanitized target is created fresh.
fn resolve_target_dir(root: &Path, target: &str) -> Result<PathBuf> {
    let joined = root.join(target);
    if joined.is_dir() {
        return Ok(joined);
    }

    let (head, rest) = match target.split_once('/') {
        Some((h, r)) => (h, Some(r)),
        None => (target, None),
    };
    let needle = head.to_lowercase();
    let matched = subdirs(root)?
        .into_iter()
        .find(|name| name.to_lowercase().contains(&needle));

    let base = match matched {
        Some(name) => root.join(name),
        None => {
            let cleaned = sanitize::name(head);
            if cleaned.is_empty() {
                bail!("invalid --in directory name: {target:?}");
            }
            root.join(cleaned)
        }
    };
    Ok(match rest {
        Some(rest) if !rest.is_empty() => base.join(rest),
        _ => base,
    })
}

/// Bare `--in`: fzf over the existing subdirectories of `root`.
fn pick_directory(root: &Path, config: &Config) -> Result<PathBuf> {
    let names = subdirs(root)?;
    if names.is_empty() {
        bail!(
            "no subdirectories under {} to pick from; create one with `notez mkdir <name>`",
            root.display()
        );
    }
    if !config.tools.fzf {
        bail!("bare --in needs fzf for the directory picker; pass a name instead: --in <dir>");
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
    Ok(root.join(selected))
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
        assert!(file_name(&first).ends_with(&format!("-{expected_stem}.md")));
        assert!(
            file_name(&second).ends_with(&format!("-{expected_stem}-2.md")),
            "got {:?}",
            second
        );
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
        assert!(file_name(&third).ends_with("-call-the-bank-3.md"), "got {:?}", third);
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "pre-existing two\n");

        let fourth = run(words("call the bank"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert!(file_name(&fourth).ends_with("-call-the-bank-4.md"), "got {:?}", fourth);
    }

    #[test]
    fn title_ending_in_number_never_causes_overwrite() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let numbered = run(words("plan 2"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert!(file_name(&numbered).ends_with("-plan-2.md"));
        std::fs::write(&numbered, "plan two edits\n").unwrap();

        let plain = run(words("plan"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert!(file_name(&plain).ends_with("-plan.md"), "got {:?}", plain);
        std::fs::write(&plain, "plan edits\n").unwrap();

        let repeat = run(words("plan"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert!(file_name(&repeat).ends_with("-plan-3.md"), "got {:?}", repeat);
        assert!(std::fs::read_to_string(&repeat).unwrap().starts_with("# plan\n"));

        let numbered_again = run(words("plan 2"), None, false, false, Scope::Global, &config)
            .unwrap()
            .path;
        assert!(
            file_name(&numbered_again).ends_with("-plan-2-2.md"),
            "got {:?}",
            numbered_again
        );

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
                .and_then(|d| create_in_dir(words("call the bank"), d, scope));
            results.push((scope, added, dir, created));
        }
        std::env::set_current_dir(saved).unwrap();

        for (scope, added, dir, created) in results {
            let (added, dir, created) = (added.unwrap().path, dir.unwrap(), created.unwrap().path);
            assert_eq!(added.parent().unwrap(), dir, "{scope:?}");
            assert_eq!(created.parent().unwrap(), dir, "{scope:?}");
            assert!(file_name(&added).ends_with("-call-the-bank.md"), "{scope:?}");
            assert!(file_name(&created).ends_with("-call-the-bank-2.md"), "{scope:?}");
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

        let created = create_in_dir(vec![], &dir, Scope::Personal).unwrap();

        assert_eq!(created.path.parent().unwrap(), dir);
        assert!(!created.had_body);
        assert!(file_name(&created.path).ends_with("-untitled.md"));
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
