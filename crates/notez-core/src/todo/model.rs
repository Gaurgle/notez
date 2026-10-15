//! todoz data model: a flat list of tasks where hierarchy is encoded by
//! `depth` + adjacency, with synthetic header rows separating sections.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::NoteDate;

/// Tri-state checkbox. `Half` is a derived state for a parent whose children
/// are partially complete; it is never written by the user directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckState {
    Unchecked,
    Half,
    Checked,
}

impl CheckState {
    /// The markdown checkbox for this state, e.g. `[ ]`.
    pub fn marker(self) -> &'static str {
        match self {
            CheckState::Unchecked => "[ ]",
            CheckState::Half => "[/]",
            CheckState::Checked => "[x]",
        }
    }

    /// Parse a checkbox marker (`[ ]`, `[/]`, `[x]`/`[X]`).
    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker {
            "[ ]" => Some(CheckState::Unchecked),
            "[/]" => Some(CheckState::Half),
            "[x]" | "[X]" => Some(CheckState::Checked),
            _ => None,
        }
    }
}

/// A single todo row. Hierarchy is positional: a task's children are the
/// following rows with strictly greater `depth`, up to the next row at the
/// same-or-shallower depth or the next header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub text: String,
    pub state: CheckState,
    /// The `TODO.md` this task is persisted to.
    pub source: PathBuf,
    /// Section label (project or category) this task belongs to.
    pub section: String,
    /// Synthetic section header row (not written back to any file).
    pub is_header: bool,
    /// 0 = top-level, 1 = subtask, 2 = sub-subtask.
    pub depth: u8,
    pub has_subtasks: bool,
    pub collapsed: bool,
    /// Scanned-from-code TODO (read-only; never serialized). Reserved for a
    /// later milestone — the board loader does not produce these yet.
    pub is_code_todo: bool,
    /// 5-bit tag flags, matching [`crate::tags`].
    pub flags: u8,
    /// The `@YYYY-MM-DD` or `@YYYY-MM-DDTHH:MM` token on the line, if any.
    /// Written back after the text and before the tags.
    #[serde(default)]
    pub due: Option<NoteDate>,
}

/// One checkbox line of a `TODO.md`, as read by
/// [`crate::todo::parse_todo_lines`]. `text` has the tags and the due-date
/// token stripped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoLine {
    pub text: String,
    pub state: CheckState,
    /// 0 = top-level, 1 = subtask, 2 = sub-subtask.
    pub depth: u8,
    /// 5-bit tag flags, matching [`crate::tags`].
    pub flags: u8,
    pub due: Option<NoteDate>,
}
