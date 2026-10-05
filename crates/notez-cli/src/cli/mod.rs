//! Command-line interface definition.
//!
//! Mirrors notez-cli's surface 1:1: same subcommand names, same global flags,
//! same aliases. The argv-0 dispatch (`todoz`, `zlog`, `znote`, ...) lives in
//! [`crate::main`].

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "notez",
    about = "A local-first CLI note-taking tool",
    version,
    disable_help_flag = true,
)]
pub struct Cli {
    /// Show help
    #[arg(short = 'h', long = "help", global = true)]
    pub help: bool,

    /// personal+global: `~/notez/` (your notez repo, notes bound to no project)
    #[arg(short = 'g', long = "global", global = true)]
    pub global: bool,

    /// personal+project: `~/notez/personal/<project>/` (private, syncs across your machines)
    #[arg(short = 'p', long = "private", global = true)]
    pub private: bool,

    /// scratch: `./.notez/` (gitignored, this machine only, never syncs)
    #[arg(short = 'l', long = "local", global = true)]
    pub local: bool,

    /// Open the global directory picker, then launch yazi (alias for `nav`)
    #[arg(short = 'n', long = "nav", global = true)]
    pub nav: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new note in the scope root (`add quick ...` makes a quick note)
    Add {
        /// Note title (defaults to "untitled")
        title: Vec<String>,
        /// Target subdirectory (bare --in opens an fzf picker).
        /// Resolves under the global `~/notez/` root by default.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        r#in: Option<String>,
        /// With `--in`: resolve under the current scope's root instead of global.
        #[arg(long = "in-local")]
        in_local: bool,
    },

    /// Append a timestamped entry to today's daily log
    Log {
        /// Log message
        message: Vec<String>,
    },

    /// Open daily logs directory
    Logz,

    /// Open daily logs directory (alias for `logz`)
    Logs,

    /// Create a new numbered subdirectory
    Mkdir {
        /// Directory name
        name: Vec<String>,
    },

    /// Search notes content
    Search {
        /// Search term
        term: String,
    },

    /// Show directory tree (interactive TUI)
    Tree,

    /// Run the setup wizard
    Setup,

    /// Create a demo project for screenshots
    Demo {
        /// Launch a demo view: todo, tree, todo-g, tree-g
        view: Option<String>,
    },

    /// Quick log entry (alias for `log`)
    Zlog {
        /// Log message
        message: Vec<String>,
    },

    /// Show directory tree (alias for `tree`)
    Treez,

    /// Open an existing note (alias for `edit`)
    Editz {
        /// Search term to fuzzy-match note filename
        term: Option<String>,
    },

    /// Search notes content (alias for `search`)
    Findz {
        /// Search term
        term: String,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate for (zsh, bash, fish)
        shell: String,
    },

    /// Print shell integration (`eval "$(notez init zsh)"` from .zshrc)
    Init {
        /// Shell to generate integration for (zsh, bash, fish)
        shell: String,
    },

    /// Manage project todos (interactive TUI)
    Todo {
        /// Quick-add a todo item
        item: Option<String>,
    },

    /// Open an existing note
    Edit {
        /// Search term to fuzzy-match note filename
        term: Option<String>,
    },

    /// Interactive todo manager (alias for `todo`)
    Todoz {
        /// Quick-add a todo item
        item: Option<String>,
    },

    /// Create a private quick note in `00_quick-notes/`
    Quick {
        /// Note title (defaults to "untitled")
        title: Vec<String>,
    },

    /// New note (alias for `add`)
    Znote {
        /// Note title (defaults to "untitled")
        title: Vec<String>,
        /// Target subdirectory (bare --in opens an fzf picker).
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        r#in: Option<String>,
        /// With `--in`: resolve under the current scope's root instead of global.
        #[arg(long = "in-local")]
        in_local: bool,
    },

    /// Open the global directory picker, then launch yazi
    #[command(alias = "n")]
    Nav,

    // Project registry:

    /// Register a project on this machine. Without args: uses current dir.
    Attach {
        /// Project name (defaults to git toplevel or dir name)
        name: Option<String>,
        /// Project root path (defaults to current dir)
        #[arg(long)]
        path: Option<String>,
    },

    /// Unregister a project from this machine. Does not touch notes.
    Detach {
        /// Project name to remove
        name: String,
    },

    /// List registered projects on this machine.
    List,

    /// Sync the global notez root with its git remote.
    Sync,

    /// One-time migration of the legacy notez-cli layout (numbered dirs +
    /// symlinks) into the scope model. Repo-private notes become personal.
    MigrateFromLegacy {
        /// Print the plan without changing anything
        #[arg(long)]
        dry_run: bool,
    },
}

/// Split free-form args into title words and an optional quoted body.
///
/// notez-cli's trick: any arg that contains a space must have been quoted by
/// the shell, so it is the body. All spaceless args concatenated with spaces
/// form the title. Lets users write `notez add my idea "this is the body"`
/// without needing a `--body` flag.
pub fn split_title_body(args: Vec<String>) -> (Option<String>, Option<String>) {
    if args.is_empty() {
        return (None, None);
    }

    let mut title_parts: Vec<String> = Vec::new();
    let mut body: Option<String> = None;

    for arg in args {
        if arg.contains(' ') {
            body = Some(arg);
        } else {
            title_parts.push(arg);
        }
    }

    let title = if title_parts.is_empty() {
        None
    } else {
        Some(title_parts.join(" "))
    };

    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_args_returns_none() {
        let (t, b) = split_title_body(vec![]);
        assert!(t.is_none());
        assert!(b.is_none());
    }

    #[test]
    fn single_words_become_title() {
        let (t, b) = split_title_body(vec!["my".into(), "cool".into(), "idea".into()]);
        assert_eq!(t, Some("my cool idea".to_string()));
        assert!(b.is_none());
    }

    #[test]
    fn quoted_arg_becomes_body() {
        let (t, b) = split_title_body(vec![
            "my".into(),
            "idea".into(),
            "this is the body".into(),
        ]);
        assert_eq!(t, Some("my idea".to_string()));
        assert_eq!(b, Some("this is the body".to_string()));
    }

    #[test]
    fn only_body_no_title() {
        let (t, b) = split_title_body(vec!["body only".into()]);
        assert!(t.is_none());
        assert_eq!(b, Some("body only".to_string()));
    }
}
