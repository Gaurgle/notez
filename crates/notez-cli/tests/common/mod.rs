//! Fixtures for the storage-layer scenarios in `DESIGN.md`: a temp "world"
//! holding bare git remotes and fake machines, each with its own home and
//! config dir, driving the real `notez` binary and the real `git`.
//!
//! Every child process starts from an empty environment plus `PATH`, so the
//! developer's real `~/notez`, `~/.config/notez`, git config, editor and any
//! `GIT_DIR`-style variables can never leak in.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// One test's sandbox. Dropping it deletes everything.
pub struct World {
    _tmp: TempDir,
    /// Canonical temp root. Canonical because git reports real paths
    /// (`/private/var/...` on macOS), and `HOME` must match them for notez
    /// to store paths tilde-relative.
    pub root: PathBuf,
    /// Empty file used as every process's global git config.
    gitconfig: PathBuf,
}

impl World {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path().canonicalize().expect("canonicalize temp dir");
        let gitconfig = root.join("empty.gitconfig");
        std::fs::write(&gitconfig, "").expect("write empty gitconfig");
        Self {
            _tmp: tmp,
            root,
            gitconfig,
        }
    }

    /// A machine whose user is `user`: home `<tmp>/<user>`, config dir
    /// `<tmp>/xdg-<user>` (outside the home, so the test proves
    /// `XDG_CONFIG_HOME` is honoured rather than the `~/.config` fallback).
    /// Runs `notez setup` so the machine has a real config file.
    pub fn machine(&self, user: &str) -> Machine {
        let machine = Machine {
            user: user.to_string(),
            home: self.root.join(user),
            xdg: self.root.join(format!("xdg-{user}")),
            gitconfig: self.gitconfig.clone(),
        };
        std::fs::create_dir_all(&machine.home).expect("create home");
        std::fs::create_dir_all(&machine.xdg).expect("create xdg dir");
        machine.notez_ok(&machine.home, &["setup"]);
        machine
    }

    /// An empty bare repository at `<tmp>/remotes/<name>.git`.
    pub fn bare_remote(&self, name: &str) -> PathBuf {
        let path = self.root.join("remotes").join(format!("{name}.git"));
        std::fs::create_dir_all(&path).expect("create remote dir");
        let seeder = self.seeder();
        seeder.git_ok(&path, &["init", "-q", "--bare", "-b", "main"]);
        path
    }

    /// A bare project remote seeded with a README and one public note in
    /// `notez/`, as a team repository that already uses notez would be.
    pub fn project_remote(&self, name: &str) -> PathBuf {
        let remote = self.bare_remote(name);
        let seeder = self.seeder();
        let work = self.root.join("seed").join(name);
        std::fs::create_dir_all(work.join("notez")).expect("create seed dirs");
        seeder.git_ok(&work, &["init", "-q", "-b", "main"]);
        std::fs::write(work.join("README.md"), format!("# {name}\n")).unwrap();
        std::fs::write(work.join("notez").join("onboarding.md"), "# Onboarding\n").unwrap();
        seeder.git_ok(&work, &["add", "-A"]);
        seeder.git_ok(&work, &["commit", "-q", "-m", "seed project"]);
        seeder.git_ok(&work, &["remote", "add", "origin", path_str(&remote)]);
        seeder.git_ok(&work, &["push", "-q", "-u", "origin", "main"]);
        remote
    }

    /// A machine-less identity for seeding remotes; never runs notez.
    fn seeder(&self) -> Machine {
        Machine {
            user: "seeder".to_string(),
            home: self.root.join("seeder"),
            xdg: self.root.join("xdg-seeder"),
            gitconfig: self.gitconfig.clone(),
        }
    }
}

/// A fake machine: a home, a config dir and a git identity.
pub struct Machine {
    pub user: String,
    pub home: PathBuf,
    pub xdg: PathBuf,
    gitconfig: PathBuf,
}

impl Machine {
    /// The vault, i.e. the default `notez_root` (`~/notez`) on this machine.
    pub fn vault(&self) -> PathBuf {
        self.home.join("notez")
    }

    /// This machine's per-machine project registry.
    pub fn registry_file(&self) -> PathBuf {
        self.xdg.join("notez").join("registry.toml")
    }

    /// Run `notez` in `cwd` with this machine's isolated environment.
    pub fn notez(&self, cwd: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_notez"));
        cmd.args(args).current_dir(cwd);
        self.isolate(&mut cmd);
        cmd.output().expect("spawn notez")
    }

    /// Run `notez` and require success; returns stdout.
    pub fn notez_ok(&self, cwd: &Path, args: &[&str]) -> String {
        let out = self.notez(cwd, args);
        assert!(
            out.status.success(),
            "notez {args:?} in {cwd:?} failed:\n{}",
            describe(&out)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Run `notez` to create a note and return the path from its
    /// `Created: <path>` line.
    pub fn create_note(&self, cwd: &Path, args: &[&str]) -> PathBuf {
        let stdout = self.notez_ok(cwd, args);
        let line = stdout
            .lines()
            .find_map(|l| l.strip_prefix("Created: "))
            .unwrap_or_else(|| panic!("no `Created:` line from notez {args:?}:\n{stdout}"));
        PathBuf::from(line.trim())
    }

    /// Run `git` in `cwd` with this machine's isolated environment.
    pub fn git<S: AsRef<OsStr>>(&self, cwd: &Path, args: &[S]) -> Output {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(cwd);
        self.isolate(&mut cmd);
        cmd.output().expect("spawn git")
    }

    /// Run `git` and require success; returns stdout.
    pub fn git_ok<S: AsRef<OsStr>>(&self, cwd: &Path, args: &[S]) -> String {
        let out = self.git(cwd, args);
        let shown: Vec<_> = args.iter().map(|a| a.as_ref().to_string_lossy()).collect();
        assert!(
            out.status.success(),
            "git {shown:?} in {cwd:?} failed:\n{}",
            describe(&out)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Create this machine's vault at `~/notez` with one seed commit and
    /// push it to the empty bare `remote`, tracking `origin/main`.
    pub fn init_vault(&self, remote: &Path) {
        let vault = self.vault();
        std::fs::create_dir_all(&vault).expect("create vault");
        self.git_ok(&vault, &["init", "-q", "-b", "main"]);
        std::fs::write(vault.join("README.md"), "# notes\n").unwrap();
        self.git_ok(&vault, &["add", "-A"]);
        self.git_ok(&vault, &["commit", "-q", "-m", "seed vault"]);
        self.git_ok(&vault, &["remote", "add", "origin", path_str(remote)]);
        self.git_ok(&vault, &["push", "-q", "-u", "origin", "main"]);
    }

    /// Clone the vault `remote` to `~/notez` on this machine.
    pub fn clone_vault(&self, remote: &Path) {
        self.git_ok(
            &self.home,
            &["clone", "-q", path_str(remote), path_str(&self.vault())],
        );
    }

    /// Clone `remote` to `~/<rel>` and return the clone's path.
    pub fn clone_project(&self, remote: &Path, rel: &str) -> PathBuf {
        let dest = self.home.join(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).expect("create clone parent");
        self.git_ok(
            &self.home,
            &["clone", "-q", path_str(remote), path_str(&dest)],
        );
        dest
    }

    fn isolate(&self, cmd: &mut Command) {
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.xdg)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", &self.user)
            .env("GIT_AUTHOR_EMAIL", format!("{}@example.com", self.user))
            .env("GIT_COMMITTER_NAME", &self.user)
            .env("GIT_COMMITTER_EMAIL", format!("{}@example.com", self.user))
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("EDITOR", "true")
            .env("VISUAL", "true")
            .env("GIT_EDITOR", "true")
            .stdin(Stdio::null());
    }
}

/// Files tracked on `main` in the bare repository `remote`.
pub fn remote_files(machine: &Machine, remote: &Path) -> Vec<String> {
    machine
        .git_ok(remote, &["ls-tree", "-r", "--name-only", "main"])
        .lines()
        .map(str::to_string)
        .collect()
}

/// Every path under `dir` whose file name is `name`, without following
/// symlinks.
pub fn find_named(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name() == name {
                found.push(path.clone());
            }
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path);
            }
        }
    }
    found
}

/// Every symlink under `dir`.
pub fn find_symlinks(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                found.push(entry.path());
            } else if kind.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    found
}

pub fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf-8 temp path")
}

pub fn describe(out: &Output) -> String {
    format!(
        "status: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    )
}
