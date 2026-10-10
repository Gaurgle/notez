//! Shared TUI infrastructure.
//!
//! Owns the enter/leave dance for raw mode + alternate screen + mouse
//! capture so that the per-command TUIs (`tree`, `todo`) do not duplicate
//! it. A panic hook is registered in `main` so a crashed TUI never leaves
//! the terminal in raw mode.

#![allow(dead_code)]

pub mod footer;
pub mod header;
pub mod help;
pub mod highlight;
pub mod markdown;
pub mod move_path;
pub mod panes;
pub mod tags;
pub mod text;
pub mod theme;
pub mod todo;
pub mod tree;

use std::io::{Stdout, stdout};

use anyhow::Result;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn enter() -> Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

pub fn leave() -> Result<()> {
    let mut out = stdout();
    execute!(out, DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

/// Suspend the TUI, open `path` in the given editor command, and block
/// until it exits. The caller re-enters with [`enter`] afterwards.
pub fn open_in_editor(editor: &str, path: &std::path::Path) -> Result<()> {
    leave()?;
    std::process::Command::new(editor)
        .arg(path)
        .status()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("failed to launch editor {editor}: {e}"))
}

/// Vim-style `:` command line, carried over from notez-cli so `:q` / `:wq`
/// muscle memory keeps working inside the TUIs.
pub struct VimCommandMode {
    pub active: bool,
    pub buffer: String,
}

impl VimCommandMode {
    pub fn new() -> Self {
        Self {
            active: false,
            buffer: String::new(),
        }
    }

    /// Process a key event. Any key that arrives while the command line is
    /// active, or the `:` that opens it, is consumed: the caller must not
    /// handle it again, even when the key closed the command line (Esc, or
    /// Backspace emptying the buffer).
    pub fn handle_key(&mut self, key: KeyEvent) -> VimKey {
        if !self.active {
            if key.code == KeyCode::Char(':') {
                self.active = true;
                self.buffer.clear();
                self.buffer.push(':');
                return VimKey::Consumed;
            }
            return VimKey::NotConsumed;
        }
        match key.code {
            KeyCode::Enter => {
                let cmd = self.buffer.clone();
                self.active = false;
                self.buffer.clear();
                return VimKey::Command(cmd);
            }
            KeyCode::Esc => {
                self.active = false;
                self.buffer.clear();
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    self.active = false;
                }
            }
            KeyCode::Char(c) => self.buffer.push(c),
            _ => {}
        }
        VimKey::Consumed
    }

    /// True when a completed command means "quit".
    pub fn is_quit(cmd: &str) -> bool {
        matches!(cmd, ":wq" | ":qa" | ":q" | ":q!")
    }
}

/// What `VimCommandMode::handle_key` did with a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimKey {
    /// The command line is closed and the key is not `:`; the view handles it.
    NotConsumed,
    /// The command line took the key; the view must not handle it.
    Consumed,
    /// Enter completed this command; the key is consumed.
    Command(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(vim: &mut VimCommandMode, code: KeyCode) -> VimKey {
        vim.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn opened() -> VimCommandMode {
        let mut vim = VimCommandMode::new();
        assert_eq!(press(&mut vim, KeyCode::Char(':')), VimKey::Consumed);
        assert!(vim.active);
        vim
    }

    #[test]
    fn keys_pass_through_while_closed() {
        let mut vim = VimCommandMode::new();
        assert_eq!(press(&mut vim, KeyCode::Esc), VimKey::NotConsumed);
        assert_eq!(press(&mut vim, KeyCode::Char('q')), VimKey::NotConsumed);
        assert!(!vim.active);
    }

    #[test]
    fn esc_closes_the_command_line_and_is_consumed() {
        let mut vim = opened();
        press(&mut vim, KeyCode::Char('w'));
        assert_eq!(press(&mut vim, KeyCode::Esc), VimKey::Consumed);
        assert!(!vim.active);
        assert!(vim.buffer.is_empty());
    }

    #[test]
    fn backspace_that_empties_the_buffer_closes_and_is_consumed() {
        let mut vim = opened();
        press(&mut vim, KeyCode::Char('q'));
        assert_eq!(press(&mut vim, KeyCode::Backspace), VimKey::Consumed);
        assert!(vim.active);
        assert_eq!(press(&mut vim, KeyCode::Backspace), VimKey::Consumed);
        assert!(!vim.active);
    }

    #[test]
    fn characters_are_consumed_into_the_buffer() {
        let mut vim = opened();
        assert_eq!(press(&mut vim, KeyCode::Char('q')), VimKey::Consumed);
        assert_eq!(press(&mut vim, KeyCode::Down), VimKey::Consumed);
        assert_eq!(vim.buffer, ":q");
    }

    #[test]
    fn enter_returns_the_command_and_quit_commands_quit() {
        for cmd in [":q", ":wq", ":qa", ":q!"] {
            let mut vim = VimCommandMode::new();
            for c in cmd.chars() {
                press(&mut vim, KeyCode::Char(c));
            }
            let got = press(&mut vim, KeyCode::Enter);
            assert_eq!(got, VimKey::Command(cmd.to_string()));
            assert!(!vim.active);
            assert!(VimCommandMode::is_quit(cmd));
        }
        assert!(!VimCommandMode::is_quit(":w"));
    }
}
