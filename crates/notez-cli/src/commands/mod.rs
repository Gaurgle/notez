//! Command implementations.
//!
//! Each submodule implements one or two subcommands and aliases. The argv
//! dispatch lives in `main.rs`.

pub mod add;
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
