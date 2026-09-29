//! notez-core: the GUI-agnostic engine shared by the `notez` CLI and the
//! notez desktop app.
//!
//! Everything here returns plain data and performs no terminal I/O, so the
//! same logic backs the ratatui TUI, the clap CLI, and the Tauri backend.

pub mod config;
pub mod core;
pub mod filter;
pub mod migrate;
pub mod note_tags;
pub mod search;
pub mod sync;
pub mod tags;
pub mod todo;
pub mod util;

pub use crate::config::{Config, NotezMetadata, ProjectRegistry};
pub use crate::core::{Note, Project, Scope};
