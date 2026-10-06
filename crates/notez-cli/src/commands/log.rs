//! `notez log` and `zlog`: append a timestamped entry to today's daily log.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use notez_core::config::Config;
use notez_core::core::{Scope, note, project, resolve};

/// Append a log entry. Returns the file path written to.
pub fn run(message_words: Vec<String>, scope: Scope, config: &Config) -> Result<PathBuf> {
    let message = message_words.join(" ").trim().to_string();
    if message.is_empty() {
        bail!("log message cannot be empty");
    }

    let dir = resolve::daily_logs(scope, config)?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create log dir {}", dir.display()))?;
    if scope == Scope::Local {
        project::ensure_scratch_gitignored(&dir);
    }

    let path = dir.join(note::todays_log_filename());
    // Only a missing log starts empty; an existing but unreadable one is
    // never replaced.
    let existing = match std::fs::read_to_string(&path) {
        Ok(existing) => existing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(e).with_context(|| {
                format!("could not read log {}, nothing was changed", path.display())
            });
        }
    };
    let updated = note::append_log_entry(&existing, &message);
    std::fs::write(&path, updated)
        .with_context(|| format!("failed to write log {}", path.display()))?;

    Ok(path)
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
    fn empty_message_errors() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        let err = run(vec!["".into()], Scope::Global, &config).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn first_call_creates_file_with_header() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let path = run(
            vec!["first".into(), "entry".into()],
            Scope::Global,
            &config,
        )
        .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("# Daily Log - "));
        assert!(body.contains(" - first entry"));
    }

    #[test]
    fn second_call_appends() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        run(vec!["first".into()], Scope::Global, &config).unwrap();
        let path = run(vec!["second".into()], Scope::Global, &config).unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains(" - first"));
        assert!(body.contains(" - second"));
        assert!(body.starts_with("# Daily Log - "));
    }

    #[test]
    fn unreadable_log_is_left_untouched_and_errors() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        let log_dir = resolve::daily_logs(Scope::Global, &config).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();
        let path = log_dir.join(note::todays_log_filename());
        let bytes = [0xff, 0xfe, 0x00, 0x41];
        std::fs::write(&path, bytes).unwrap();

        let err = run(vec!["entry".into()], Scope::Global, &config).unwrap_err();

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let message = format!("{err:#}");
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("nothing was changed"), "{message}");
    }

    #[test]
    #[serial_test::serial]
    fn personal_outside_git_falls_back_to_global() {
        let notez_root = tempdir().unwrap();
        let config = config_in(notez_root.path());

        let cwd = tempdir().unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        let result = run(vec!["hi".into()], Scope::Personal, &config);
        std::env::set_current_dir(saved).unwrap();

        let path = result.unwrap();
        // Personal fallback puts the log under <notez_root>/01_daily-logs/.
        assert_eq!(
            path.parent().unwrap(),
            notez_root.path().join("01_daily-logs"),
        );
    }
}
