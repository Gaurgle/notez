//! Migration from the legacy notez-cli layout (numbered dirs + symlinks +
//! `~/.config/notez/projects`) to notez's scope model.
//!
//! Strategy: a numbered dir `NN_<name>` whose `<name>` matches a legacy
//! project becomes that project's Personal notes — moved to
//! `<notez_root>/personal/<name>/` — and the project is attached to the
//! per-machine registry. Legacy dirs are mirrors full of symlinks into each
//! repo's `.notez/` (private) and `notez/` (public) stores, so after the
//! move the tree is materialized: private targets move here as real files
//! (personal scope owns them now), public targets stay in the repo (that is
//! already notez's public scope) and only the symlink is dropped, dangling
//! links are pruned. Global dirs (quick-notes, daily-logs, _todos) and
//! unknown dirs are left untouched. Existing destinations are merged
//! entry-by-entry; nothing is ever overwritten — collisions are reported
//! and left in place for manual review.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{paths, Config, ProjectRegistry};
use crate::util::tilde;

/// One planned migration step (preview is a `Vec<PlanItem>`).
#[derive(Serialize, Clone)]
pub struct PlanItem {
    pub name: String,
    /// Project repo path on this machine (tilde-contracted).
    pub repo_path: String,
    /// Source numbered dir (tilde-contracted).
    pub from: String,
    /// Destination personal dir (tilde-contracted).
    pub to: String,
    /// Human note, e.g. "ready" or "destination exists — will merge".
    pub note: String,
}

fn legacy_projects_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config/notez/projects")
}

/// Parse the legacy `name=path` projects file.
pub fn read_legacy_projects() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(content) = std::fs::read_to_string(legacy_projects_file()) {
        for line in content.lines() {
            if let Some((name, path)) = line.split_once('=') {
                let (name, path) = (name.trim(), path.trim());
                if !name.is_empty() && !path.is_empty() {
                    map.insert(name.to_string(), path.to_string());
                }
            }
        }
    }
    map
}

/// Strip a leading `NN_` numeric prefix: `03_notez-cli` → `notez-cli`.
fn strip_num_prefix(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() > 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'_' {
        &s[3..]
    } else {
        s
    }
}

fn is_special(dirname: &str) -> bool {
    let name = strip_num_prefix(dirname);
    matches!(dirname, "_todos" | "personal" | ".git")
        || name.starts_with("quick-note")
        || name.starts_with("daily-log")
}

/// Compute the migration plan from the standard legacy projects file.
pub fn plan(config: &Config) -> Vec<PlanItem> {
    plan_with(&read_legacy_projects(), config)
}

/// Compute the migration plan against an explicit legacy project map.
pub fn plan_with(projects: &BTreeMap<String, String>, config: &Config) -> Vec<PlanItem> {
    let root = config.notez_root_path();
    let personal_root = root.join("personal");
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(&root) else {
        return out;
    };
    let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let dirname = entry.file_name().to_string_lossy().to_string();
        if is_special(&dirname) {
            continue;
        }
        let name = strip_num_prefix(&dirname);
        let Some(repo) = projects.get(name) else {
            continue;
        };
        let dest = personal_root.join(name);
        out.push(PlanItem {
            name: name.to_string(),
            repo_path: repo.clone(),
            from: tilde::contract(&entry.path()),
            to: tilde::contract(&dest),
            note: if dest.exists() {
                "destination exists — will merge".to_string()
            } else {
                "ready".to_string()
            },
        });
    }
    out
}

/// Apply the plan from the standard legacy projects file.
pub fn apply(config: &Config) -> std::io::Result<Vec<String>> {
    apply_with(&read_legacy_projects(), config)
}

/// Apply the plan: attach each project, move (or merge) its numbered dir
/// into `personal/<name>/`, then materialize legacy symlinks. Returns a
/// per-step log.
pub fn apply_with(
    projects: &BTreeMap<String, String>,
    config: &Config,
) -> std::io::Result<Vec<String>> {
    let root = config.notez_root_path();
    std::fs::create_dir_all(root.join("personal"))?;
    let mut reg = ProjectRegistry::load().unwrap_or_default();
    let mut log = Vec::new();

    for item in plan_with(projects, config) {
        reg.attach(&item.name, &tilde::expand(&item.repo_path));
        let from = tilde::expand(&item.from);
        let to = tilde::expand(&item.to);
        merge_move(&from, &to, &mut log)?;
        materialize_tree(&to, &mut log)?;
        log.push(format!("migrated {} → personal/{}", item.name, item.name));
    }

    let _ = reg.save_to(&paths::registry_file());
    log.push("attached migrated projects to the registry".to_string());
    Ok(log)
}

/// Move `from` to `to`. If `to` already exists, merge entry-by-entry:
/// entries missing at the destination move over, dirs present in both
/// recurse, anything else collides and is left in place (reported). `from`
/// is removed once emptied.
fn merge_move(from: &Path, to: &Path, log: &mut Vec<String>) -> std::io::Result<()> {
    if !to.exists() && std::fs::symlink_metadata(to).is_err() {
        std::fs::rename(from, to)?;
        return Ok(());
    }
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dest = to.join(entry.file_name());
        let dest_occupied = dest.exists() || std::fs::symlink_metadata(&dest).is_ok();
        if !dest_occupied {
            std::fs::rename(&src, &dest)?;
        } else if entry.file_type()?.is_dir() && dest.is_dir() {
            merge_move(&src, &dest, log)?;
        } else {
            log.push(format!(
                "conflict: {} exists at destination — left in place",
                tilde::contract(&src)
            ));
        }
    }
    // Only removable if every entry moved out.
    let _ = std::fs::remove_dir(from);
    Ok(())
}

/// Walk `dir` and resolve every legacy symlink into the new model.
fn materialize_tree(dir: &Path, log: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            materialize_link(&path, log)?;
        } else if ft.is_dir() {
            materialize_tree(&path, log)?;
        }
    }
    Ok(())
}

/// Resolve one symlink: private store targets are moved here as real files,
/// public store targets stay in the repo, dangling links are pruned.
fn materialize_link(link: &Path, log: &mut Vec<String>) -> std::io::Result<()> {
    let Ok(target) = std::fs::canonicalize(link) else {
        std::fs::remove_file(link)?;
        log.push(format!("pruned dangling link {}", tilde::contract(link)));
        return Ok(());
    };
    let has_component = |name: &str| target.components().any(|c| c.as_os_str() == name);
    if has_component(".notez") {
        std::fs::remove_file(link)?;
        std::fs::rename(&target, link)?;
        log.push(format!(
            "materialized {} ← {}",
            tilde::contract(link),
            tilde::contract(&target)
        ));
    } else if has_component("notez") {
        std::fs::remove_file(link)?;
        log.push(format!(
            "dropped public-store link {} (file stays in the repo)",
            tilde::contract(link)
        ));
    } else {
        log.push(format!(
            "left unknown link {} → {}",
            tilde::contract(link),
            tilde::contract(&target)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_numeric_prefix() {
        assert_eq!(strip_num_prefix("03_notez-cli"), "notez-cli");
        assert_eq!(strip_num_prefix("plain"), "plain");
        assert_eq!(strip_num_prefix("9_x"), "9_x"); // single digit isn't NN_
    }

    #[test]
    fn specials_are_skipped() {
        assert!(is_special("00_quick-notes"));
        assert!(is_special("01_daily-logs"));
        assert!(is_special("_todos"));
        assert!(is_special("personal"));
        assert!(!is_special("03_notez-cli"));
    }

    #[cfg(unix)]
    mod apply {
        use super::super::*;
        use serial_test::serial;
        use std::fs;
        use std::os::unix::fs::symlink;

        struct Fixture {
            _tmp: tempfile::TempDir,
            config: Config,
            projects: BTreeMap<String, String>,
            root: PathBuf,
            repo: PathBuf,
        }

        /// Legacy layout: a repo with private + public stores, and a
        /// numbered mirror dir in the notez root linking into both.
        fn fixture() -> Fixture {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path().to_path_buf();
            unsafe { std::env::set_var("XDG_CONFIG_HOME", base.join("xdg")) };

            let repo = base.join("repo");
            fs::create_dir_all(repo.join(".notez/00_quick-notes")).unwrap();
            fs::create_dir_all(repo.join("notez")).unwrap();
            fs::write(repo.join(".notez/TODO.md"), "- [ ] private").unwrap();
            fs::write(repo.join(".notez/00_quick-notes/a.md"), "note a").unwrap();
            fs::write(repo.join("notez/pub.md"), "public").unwrap();

            let root = base.join("notezroot");
            let legacy = root.join("03_myproj");
            fs::create_dir_all(&legacy).unwrap();
            symlink(repo.join(".notez/TODO.md"), legacy.join("TODO.md")).unwrap();
            symlink(
                repo.join(".notez/00_quick-notes"),
                legacy.join("00_quick-notes"),
            )
            .unwrap();
            symlink(repo.join("notez/pub.md"), legacy.join("pub.md")).unwrap();
            symlink(repo.join(".notez/missing.md"), legacy.join("gone.md")).unwrap();

            let mut config = Config::defaults();
            config.paths.notez_root = root.to_string_lossy().into_owned();

            let mut projects = BTreeMap::new();
            projects.insert(
                "myproj".to_string(),
                repo.to_string_lossy().into_owned(),
            );

            Fixture { _tmp: tmp, config, projects, root, repo }
        }

        #[test]
        #[serial]
        fn materializes_private_drops_public_prunes_dangling() {
            let f = fixture();
            let log = apply_with(&f.projects, &f.config).unwrap();

            let dest = f.root.join("personal/myproj");
            // Private targets became real files here…
            assert_eq!(fs::read_to_string(dest.join("TODO.md")).unwrap(), "- [ ] private");
            assert_eq!(
                fs::read_to_string(dest.join("00_quick-notes/a.md")).unwrap(),
                "note a"
            );
            assert!(!dest.join("TODO.md").is_symlink());
            // …and left the repo's private store.
            assert!(!f.repo.join(".notez/TODO.md").exists());
            // Public file stayed in the repo; its link is gone.
            assert_eq!(fs::read_to_string(f.repo.join("notez/pub.md")).unwrap(), "public");
            assert!(fs::symlink_metadata(dest.join("pub.md")).is_err());
            // Dangling link pruned, legacy dir gone.
            assert!(fs::symlink_metadata(dest.join("gone.md")).is_err());
            assert!(!f.root.join("03_myproj").exists());
            assert!(log.iter().any(|l| l.contains("materialized")));
        }

        #[test]
        #[serial]
        fn merges_into_existing_destination_without_overwriting() {
            let f = fixture();
            let dest = f.root.join("personal/myproj");
            fs::create_dir_all(&dest).unwrap();
            fs::write(dest.join("TODO.md"), "already here").unwrap();

            let log = apply_with(&f.projects, &f.config).unwrap();

            // Collision: both sides untouched, reported.
            assert_eq!(fs::read_to_string(dest.join("TODO.md")).unwrap(), "already here");
            assert_eq!(
                fs::read_to_string(f.repo.join(".notez/TODO.md")).unwrap(),
                "- [ ] private"
            );
            assert!(log.iter().any(|l| l.contains("conflict")));
            // Non-colliding entries still migrated + materialized.
            assert_eq!(
                fs::read_to_string(dest.join("00_quick-notes/a.md")).unwrap(),
                "note a"
            );
            // Legacy dir kept because the conflicting link remains.
            assert!(f.root.join("03_myproj").exists());
        }

        #[test]
        #[serial]
        fn attaches_migrated_project_to_registry() {
            let f = fixture();
            apply_with(&f.projects, &f.config).unwrap();
            let reg = fs::read_to_string(paths::registry_file()).unwrap();
            assert!(reg.contains("myproj"), "registry: {reg}");
        }
    }
}
