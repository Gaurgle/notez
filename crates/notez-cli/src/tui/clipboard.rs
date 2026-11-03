//! Put text on the system clipboard by shelling out, as `fzf` is: `pbcopy`
//! on macOS, `wl-copy` then `xclip -selection clipboard` elsewhere. The tool
//! is chosen at runtime from `PATH`.
//!
//! Callers take the writer as `&mut dyn FnMut(&str) -> Result<()>`, so tests
//! inject one and never touch the real clipboard.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

/// A clipboard tool: the program and its arguments.
type Tool = (&'static str, &'static [&'static str]);

/// The tools this platform tries, in order.
fn candidates() -> &'static [Tool] {
    if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    }
}

/// The first of `tools` that `found` says is installed, or an error naming
/// every tool tried.
fn pick(tools: &'static [Tool], found: impl Fn(&str) -> bool) -> Result<Tool> {
    tools.iter().copied().find(|(name, _)| found(name)).ok_or_else(|| {
        let names: Vec<&str> = tools.iter().map(|(name, _)| *name).collect();
        anyhow!("no clipboard tool found: install {}", names.join(" or "))
    })
}

/// Whether `name` is an executable file in a `PATH` directory.
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| is_file(&dir.join(name)))
    })
}

fn is_file(path: &Path) -> bool {
    path.metadata().is_ok_and(|m| m.is_file())
}

/// Copy `text` to the system clipboard.
pub fn copy(text: &str) -> Result<()> {
    let (program, args) = pick(candidates(), on_path)?;
    // Output goes nowhere: the tree browser owns the screen, and `xclip`
    // keeps its streams open after it forks to serve the selection.
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to run {program}"))?;
    child
        .stdin
        .take()
        .context("no stdin")?
        .write_all(text.as_bytes())
        .with_context(|| format!("failed to write to {program}"))?;
    let status = child.wait().with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} failed ({status})");
    }
    Ok(())
}

/// A byte count as the footer shows it: `512 B`, `1.2 KB`, `3.4 MB`.
pub fn size_label(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: &[Tool] = &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];

    #[test]
    fn a_missing_tool_is_an_error_naming_every_tool_tried() {
        let err = pick(LINUX, |_| false).unwrap_err().to_string();
        assert_eq!(err, "no clipboard tool found: install wl-copy or xclip");
        let err = pick(&[("pbcopy", &[])], |_| false).unwrap_err().to_string();
        assert_eq!(err, "no clipboard tool found: install pbcopy");
    }

    #[test]
    fn wl_copy_wins_over_xclip_and_xclip_is_the_fallback() {
        assert_eq!(pick(LINUX, |_| true).unwrap().0, "wl-copy");
        let (name, args) = pick(LINUX, |n| n == "xclip").unwrap();
        assert_eq!((name, args), ("xclip", &["-selection", "clipboard"][..]));
    }

    #[test]
    fn the_platform_tries_pbcopy_on_macos_and_wl_copy_then_xclip_elsewhere() {
        let names: Vec<&str> = candidates().iter().map(|(n, _)| *n).collect();
        if cfg!(target_os = "macos") {
            assert_eq!(names, ["pbcopy"]);
        } else {
            assert_eq!(names, ["wl-copy", "xclip"]);
        }
    }

    #[test]
    fn sizes_read_in_bytes_then_kilobytes_then_megabytes() {
        assert_eq!(size_label(0), "0 B");
        assert_eq!(size_label(1023), "1023 B");
        assert_eq!(size_label(1229), "1.2 KB");
        assert_eq!(size_label(3 * 1024 * 1024 + 400 * 1024), "3.4 MB");
    }
}
