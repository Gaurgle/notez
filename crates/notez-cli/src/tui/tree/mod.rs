//! The tree-browser TUI, ported from notez-cli.
//!
//! Only the terminal layer lives here: rendering, key and mouse handling,
//! filter state, preview pane, and the help overlay. The forest is built
//! from [`SectionSpec`]s the caller assembles out of
//! `notez_core::core::aggregate` entries (see `commands::tree`), so the
//! browser shows exactly what the aggregator knows: no symlink walking.
//! Tag flags come from per-root `.tags` files via `notez_core::note_tags`;
//! on quit only roots whose tag maps actually changed are reported back,
//! and unknown keys in an existing `.tags` (entries for files outside this
//! view) are preserved rather than dropped.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use notez_core::config::Config;
use notez_core::core::Scope;
use notez_core::filter::{self, Filter};
use notez_core::note_tags;
use notez_core::tags::FLAG_DEFS;
use notez_core::util::sanitize;

use super::footer::{self, Group, KeyHint, Mode, Slot, Toggle, QUIT_HINT_RESERVED_COLS};
use super::header::{self, SyncState};
use super::help::{self, HelpState};
use super::highlight::{self, Language};
use super::markdown;
use super::move_path;
use super::panes::{self, Pane, Panes, Press};
use super::{enter, leave, open_in_editor, text, TuiTerminal};
use super::{theme, VimCommandMode, VimKey};
use crate::commands::{add, mkdir, rename};

mod bulk;
mod delete;
mod folder;
mod input;
mod model;
mod move_prompt;
mod new_note;
mod preview;
mod rename_prompt;
mod render;
mod search;
#[cfg(test)]
mod test_support;

use bulk::*;
use delete::*;
use folder::*;
use input::*;
use model::*;
use move_prompt::*;
use new_note::*;
use preview::*;
use rename_prompt::*;
use render::*;
use search::*;

/// What the header shows.
pub struct TreeContext {
    pub title: String,
    pub path_display: String,
    /// Shown in the status bar for the whole session, whenever nothing
    /// transient is using the line.
    pub warning: Option<String>,
    /// The vault's sync state as the session opened, for the header.
    pub sync: SyncState,
    /// The project the browser was opened in, if any. Section prompts name
    /// the project only when it differs from this one.
    pub current_project: Option<String>,
    /// Where new notes can go beyond the folder under the cursor: the
    /// scope roots `Tab` cycles through in the new-note prompt, and the
    /// target when the tree has no rows.
    pub new_note_roots: NewNoteRoots,
}

/// The scope roots a new note can target.
#[derive(Debug, Clone, Default)]
pub struct NewNoteRoots {
    /// The global store, `<notez_root>/`.
    pub global: PathBuf,
    /// Each known project's stores in `Tab` order: personal, public, local.
    /// A project whose repository is unknown lists only its personal root.
    pub projects: HashMap<String, Vec<(Scope, PathBuf)>>,
}

/// One top-level section of the forest: a walk root, its display label and
/// icon, and the files (from the aggregator) that live under it. `tag_root`
/// is where this section's `.tags` lives; for personal and global sections
/// that is the notez root itself so keys match the desktop app and the
/// migrated data (`personal/<name>/...`), while public/local/docs stores
/// keep their own per-store `.tags` like notez-cli did.
pub struct SectionSpec {
    pub root: PathBuf,
    pub tag_root: PathBuf,
    pub label: String,
    pub icon: &'static str,
    pub is_doc: bool,
    pub files: Vec<PathBuf>,
    /// Every directory under `root` (absolute paths, hidden names and
    /// symlinks skipped), so a folder with no notes in it still gets a row.
    pub dirs: Vec<PathBuf>,
    /// The scope a note created in this section gets.
    pub scope: Scope,
    /// The project the section belongs to; `None` for the global store.
    pub project: Option<String>,
    /// Where a new note goes when the cursor is on the section itself. Equal
    /// to `root` for note stores; a docs section is not a note store, so for
    /// it this is the project's personal root and nothing is published by
    /// accident.
    pub new_note_root: PathBuf,
    /// Whether the section belongs to the repository the browser was
    /// opened in. The global section and other projects' sections are not.
    pub is_current: bool,
}

/// Run the browser to completion. Returns `(root, final_map)` pairs for
/// every tag root whose `.tags` content changed; the caller persists them.
/// A quit without tag edits returns an empty list (nothing gets written).
/// `rebuild` lists the sections again; the browser calls it after creating
/// or deleting a note so the tree matches the disk.
pub fn run_tree(
    sections: Vec<SectionSpec>,
    ctx: &TreeContext,
    config: &Config,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Result<Vec<(PathBuf, HashMap<String, u8>)>> {
    let (mut nodes, tag_roots) = build_forest(&sections);
    open_current_sections(&mut nodes, &sections);
    let initial: Vec<HashMap<String, u8>> =
        tag_roots.iter().map(|r| note_tags::load_tags(r)).collect();
    apply_tags(&mut nodes, &tag_roots, &initial);
    let mut forest = Forest {
        sections,
        nodes,
        tag_roots,
        initial,
    };
    let mut retired = Vec::new();
    let mut carried = Vec::new();

    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result = event_loop(
        &mut terminal,
        &mut forest,
        &mut retired,
        &mut carried,
        ctx,
        config,
        rebuild,
    );
    super::leave().context("failed to leave TUI")?;
    result?;

    Ok(exit_tag_maps(&forest, &retired, &carried))
}
