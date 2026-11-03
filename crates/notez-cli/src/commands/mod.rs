//! Command implementations.
//!
//! Each submodule implements one or two subcommands and aliases. The argv
//! dispatch lives in `main.rs`.

pub mod add;
pub mod agenda;
pub mod attach;
pub mod completions;
pub mod detach;
pub mod edit;
pub mod init;
pub mod list;
pub mod log;
pub mod logz;
pub mod migrate;
pub mod mkdir;
pub mod nav;
pub mod picker;
pub mod rename;
pub mod search;
pub mod setup;
pub mod sync;
pub mod todo;
pub mod tree;

use notez_core::config::Config;
use notez_core::core::{Project, Scope};

/// The stderr line for a write asked for personal (`-p`) outside a git
/// repo, where `resolve::root` falls back to the global root. `None` for
/// any other scope and inside a repo.
pub fn personal_fallback_hint(scope: Scope, config: &Config) -> Option<String> {
    (scope == Scope::Personal && Project::try_detect().is_none()).then(|| {
        format!(
            "notez: not in a git repo, -p wrote to global ({})",
            notez_core::util::tilde::contract(&config.notez_root_path()),
        )
    })
}
