//! Configuration files: per-machine and synced.
//!
//! Three TOML files drive notez:
//!
//! 1. **Per-machine config** at `$XDG_CONFIG_HOME/notez/config.toml`. Stores
//!    the global notez root, subdirectory names, editor preferences and
//!    tool-detection cache. Not synced between machines.
//! 2. **Per-machine registry** at `$XDG_CONFIG_HOME/notez/registry.toml`.
//!    Maps project names to their local on-disk paths. Not synced.
//! 3. **Synced metadata** at `<notez_root>/.notez-config.toml`. Holds project
//!    display names, tags and sort order. Syncs with `~/notez/` via git.
//!
//! All paths stored on disk are tilde-relative (`~/foo`) and expanded at load
//! time using [`crate::util::tilde::expand`].

pub mod metadata;
pub mod paths;
pub mod registry;

pub use metadata::{NotezMetadata, ProjectMetadata};
pub use registry::ProjectRegistry;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The per-machine config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub paths: PathsConfig,
    pub editor: EditorConfig,
    pub tools: ToolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Global notez root. Stored tilde-relative.
    pub notez_root: String,
    /// Subdirectory name for quick notes (e.g. `"00_quick-notes"`).
    pub quick_notes_dir: String,
    /// Subdirectory name for daily logs (e.g. `"01_daily-logs"`).
    pub daily_logs_dir: String,
    /// Surface each registered project's `docs/*.md` alongside its notes
    /// (tagged as docs, not notes). On by default.
    #[serde(default = "default_true")]
    pub project_docs: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub command: String,
    /// Extra args passed when opening a freshly-created note. Defaults to
    /// `["+4", "-c", "startinsert"]` so nvim lands in insert mode on the
    /// body line.
    #[serde(default)]
    pub new_note_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub fzf: bool,
    pub rg: bool,
    pub yazi: bool,
}

impl Config {
    /// Build a fresh default config. Used when no config file exists yet.
    pub fn defaults() -> Self {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
        Self {
            paths: PathsConfig {
                notez_root: "~/notez".to_string(),
                quick_notes_dir: "00_quick-notes".to_string(),
                daily_logs_dir: "01_daily-logs".to_string(),
                project_docs: true,
            },
            editor: EditorConfig {
                command: editor,
                new_note_args: vec![
                    "+4".to_string(),
                    "-c".to_string(),
                    "startinsert".to_string(),
                ],
            },
            tools: ToolsConfig {
                fzf: crate::util::tools::detect("fzf"),
                rg: crate::util::tools::detect("rg"),
                yazi: crate::util::tools::detect("yazi"),
            },
        }
    }

    /// Resolve the absolute path of the global notez root.
    pub fn notez_root_path(&self) -> PathBuf {
        crate::util::tilde::expand(&self.paths.notez_root)
    }

    /// Quick-notes directory under the global root.
    pub fn quick_notes_path(&self) -> PathBuf {
        self.notez_root_path().join(&self.paths.quick_notes_dir)
    }

    /// Daily-logs directory under the global root.
    pub fn daily_logs_path(&self) -> PathBuf {
        self.notez_root_path().join(&self.paths.daily_logs_dir)
    }

    /// Read the config from its standard location, returning defaults if
    /// the file does not exist.
    pub fn load() -> anyhow::Result<Self> {
        let path = paths::config_file();
        if !path.exists() {
            return Ok(Self::defaults());
        }
        let body = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&body)?;
        Ok(cfg)
    }

    /// Save the config to its standard location, creating parent dirs.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = paths::config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = toml::to_string_pretty(self)?;
        std::fs::write(&path, body)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_tilde_relative_root() {
        let c = Config::defaults();
        assert!(c.paths.notez_root.starts_with("~"));
    }

    #[test]
    fn defaults_resolve_to_absolute_root() {
        let c = Config::defaults();
        let p = c.notez_root_path();
        assert!(p.is_absolute(), "got {:?}", p);
    }

    #[test]
    fn toml_roundtrip_preserves_fields() {
        let c = Config::defaults();
        let s = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(c.paths.notez_root, back.paths.notez_root);
        assert_eq!(c.paths.quick_notes_dir, back.paths.quick_notes_dir);
        assert_eq!(c.editor.command, back.editor.command);
        assert_eq!(c.tools.fzf, back.tools.fzf);
    }

    #[test]
    fn quick_notes_path_joins_root_and_subdir() {
        let mut c = Config::defaults();
        c.paths.notez_root = "~/custom-notez".to_string();
        c.paths.quick_notes_dir = "00_quick".to_string();
        let p = c.quick_notes_path();
        let expected = dirs::home_dir().unwrap().join("custom-notez").join("00_quick");
        assert_eq!(p, expected);
    }
}
