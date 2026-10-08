//! Moving a note or folder on disk, across file systems too.
//!
//! [`move_path`] never overwrites, never merges and never leaves zero
//! copies: it refuses an existing destination, tries a plain rename, and
//! only when the rename fails because source and destination are on
//! different file systems (the vault and a repository may sit on different
//! volumes) falls back to copy, verify, then remove the source.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Why a move did not happen, or did not finish.
#[derive(Debug)]
pub enum MoveError {
    /// Something already exists at the destination; nothing was touched.
    Exists(PathBuf),
    /// The destination lies inside the folder being moved; nothing was touched.
    IntoItself(PathBuf),
    /// The copy fallback met a symlink (or another entry that is neither a
    /// file nor a folder) at this path; refused before anything was copied.
    NotPlain(PathBuf),
    /// A failure before anything was created at the destination.
    Io(io::Error),
    /// A failure after the destination was created, before the source was
    /// removed: both copies are in place.
    Copy { error: io::Error, dst: PathBuf },
    /// The copy is complete and verified, but removing the source failed:
    /// the destination is whole, the source may be partly removed.
    RemoveSource { error: io::Error, dst: PathBuf, src: PathBuf },
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exists(dst) => write!(f, "{} already exists", dst.display()),
            Self::IntoItself(dst) => {
                write!(f, "{} is inside the folder being moved", dst.display())
            }
            Self::NotPlain(path) => write!(
                f,
                "{} is a symlink or special file; nothing was copied",
                path.display()
            ),
            Self::Io(error) => write!(f, "{error}"),
            Self::Copy { error, dst } => {
                write!(f, "{error} (destination left at {})", dst.display())
            }
            Self::RemoveSource { error, dst, src } => write!(
                f,
                "{error} (destination left at {}, source partly left at {})",
                dst.display(),
                src.display()
            ),
        }
    }
}

impl std::error::Error for MoveError {}

/// Move `src` (a file or a folder) to `dst`, which must not exist. A plain
/// [`std::fs::rename`] first; across file systems a verified copy, then the
/// source is removed. See [`move_path_with`].
pub fn move_path(src: &Path, dst: &Path) -> Result<(), MoveError> {
    move_path_with(src, dst, |a, b| std::fs::rename(a, b), |a, b| std::fs::copy(a, b))
}

/// [`move_path`] with the rename and the file copy injected, so tests can
/// force the cross-device fallback and a bad copy on one volume.
///
/// Refuses, with nothing touched: an existing `dst` of any kind (checked
/// with `symlink_metadata`, so a dangling symlink counts), and a `dst`
/// inside `src` when `src` is a folder. A rename error other than a
/// cross-device one is returned as is. The fallback first checks the whole
/// source: a symlink anywhere in it refuses the move before anything is
/// copied. It then copies files and folders, reads every copied file back
/// and compares length and contents, and only then removes the source. Any
/// failure before that removal leaves both copies.
pub fn move_path_with(
    src: &Path,
    dst: &Path,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
    copy: impl Fn(&Path, &Path) -> io::Result<u64>,
) -> Result<(), MoveError> {
    if std::fs::symlink_metadata(dst).is_ok() {
        return Err(MoveError::Exists(dst.to_path_buf()));
    }
    let meta = std::fs::symlink_metadata(src).map_err(MoveError::Io)?;
    if meta.is_dir() && is_inside(dst, src) {
        return Err(MoveError::IntoItself(dst.to_path_buf()));
    }
    match rename(src, dst) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {}
        Err(e) => return Err(MoveError::Io(e)),
    }
    check_plain(src)?;
    let copied = if meta.is_dir() { copy_dir(src, dst, &copy) } else { copy_file(src, dst, &copy) };
    copied.map_err(|error| MoveError::Copy { error, dst: dst.to_path_buf() })?;
    let removed = if meta.is_dir() { std::fs::remove_dir_all(src) } else { std::fs::remove_file(src) };
    removed.map_err(|error| MoveError::RemoveSource {
        error,
        dst: dst.to_path_buf(),
        src: src.to_path_buf(),
    })
}

/// Whether `dst` is `src` or lies under it, compared lexically and, when
/// the paths resolve, through their canonical forms (`dst` itself does not
/// exist yet, so its parent is resolved).
fn is_inside(dst: &Path, src: &Path) -> bool {
    if dst.starts_with(src) {
        return true;
    }
    let (Some(parent), Some(name)) = (dst.parent(), dst.file_name()) else {
        return false;
    };
    match (std::fs::canonicalize(parent), std::fs::canonicalize(src)) {
        (Ok(parent), Ok(src)) => parent.join(name).starts_with(src),
        _ => false,
    }
}

/// Refuse a source the copy fallback cannot reproduce exactly: anything
/// but plain files and folders, at any depth.
fn check_plain(path: &Path) -> Result<(), MoveError> {
    let meta = std::fs::symlink_metadata(path).map_err(MoveError::Io)?;
    if meta.is_file() {
        return Ok(());
    }
    if !meta.is_dir() {
        return Err(MoveError::NotPlain(path.to_path_buf()));
    }
    for entry in std::fs::read_dir(path).map_err(MoveError::Io)? {
        check_plain(&entry.map_err(MoveError::Io)?.path())?;
    }
    Ok(())
}

/// Copy the folder `src` to the new folder `dst`, every file verified.
fn copy_dir(src: &Path, dst: &Path, copy: &impl Fn(&Path, &Path) -> io::Result<u64>) -> io::Result<()> {
    std::fs::create_dir(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to, copy)?;
        } else {
            copy_file(&from, &to, copy)?;
        }
    }
    Ok(())
}

/// Copy the file `src` to `dst`, then read both back and compare.
fn copy_file(src: &Path, dst: &Path, copy: &impl Fn(&Path, &Path) -> io::Result<u64>) -> io::Result<()> {
    copy(src, dst)?;
    let original = std::fs::read(src)?;
    let copied = std::fs::read(dst)?;
    if original.len() != copied.len() || original != copied {
        return Err(io::Error::other(format!(
            "the copy of {} does not match the original",
            src.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cross_device(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::CrossesDevices))
    }

    fn std_copy(a: &Path, b: &Path) -> io::Result<u64> {
        std::fs::copy(a, b)
    }

    /// `dir/src` holding `a.md` and `deep/deeper/n.md`, plus an empty
    /// folder `dir/to`. Returns the temp dir, the source and `dir/to`.
    fn tree() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("deep/deeper")).unwrap();
        std::fs::write(src.join("a.md"), "# a\n").unwrap();
        std::fs::write(src.join("deep/deeper/n.md"), "# n\nbody\n").unwrap();
        let to = dir.path().join("to");
        std::fs::create_dir(&to).unwrap();
        (dir, src, to)
    }

    #[test]
    fn a_rename_moves_a_file_and_a_folder() {
        let (_dir, src, to) = tree();
        move_path(&src.join("a.md"), &to.join("a.md")).unwrap();
        assert!(!src.join("a.md").exists());
        assert_eq!(std::fs::read_to_string(to.join("a.md")).unwrap(), "# a\n");

        move_path(&src, &to.join("src")).unwrap();
        assert!(!src.exists());
        assert_eq!(std::fs::read_to_string(to.join("src/deep/deeper/n.md")).unwrap(), "# n\nbody\n");
    }

    #[test]
    fn the_cross_device_fallback_copies_a_file_then_removes_it() {
        let (_dir, src, to) = tree();
        move_path_with(&src.join("a.md"), &to.join("a.md"), cross_device, std_copy).unwrap();
        assert!(!src.join("a.md").exists());
        assert_eq!(std::fs::read_to_string(to.join("a.md")).unwrap(), "# a\n");
    }

    #[test]
    fn the_cross_device_fallback_copies_a_folder_tree_then_removes_it() {
        let (_dir, src, to) = tree();
        std::fs::create_dir(src.join("empty")).unwrap();
        let dst = to.join("moved");
        move_path_with(&src, &dst, cross_device, std_copy).unwrap();
        assert!(!src.exists());
        assert_eq!(std::fs::read_to_string(dst.join("a.md")).unwrap(), "# a\n");
        assert_eq!(std::fs::read_to_string(dst.join("deep/deeper/n.md")).unwrap(), "# n\nbody\n");
        assert!(dst.join("empty").is_dir());
    }

    #[test]
    fn a_copy_that_does_not_match_leaves_both_copies_and_names_the_destination() {
        let bad_copy = |a: &Path, b: &Path| {
            std::fs::copy(a, b)?;
            std::fs::write(b, "garbled")?;
            Ok(7)
        };
        let (_dir, src, to) = tree();
        let dst = to.join("a.md");
        let err = move_path_with(&src.join("a.md"), &dst, cross_device, bad_copy).unwrap_err();
        assert!(matches!(err, MoveError::Copy { .. }), "{err:?}");
        assert!(err.to_string().contains(&format!("destination left at {}", dst.display())), "{err}");
        assert_eq!(std::fs::read_to_string(src.join("a.md")).unwrap(), "# a\n");
        assert!(dst.exists());

        let dst = to.join("tree");
        let err = move_path_with(&src, &dst, cross_device, bad_copy).unwrap_err();
        assert!(matches!(err, MoveError::Copy { .. }), "{err:?}");
        assert!(src.join("deep/deeper/n.md").is_file(), "the source folder is whole");
        assert!(dst.exists());
    }

    #[test]
    fn a_failing_copy_leaves_the_source_whole() {
        let failing = |_: &Path, _: &Path| Err(io::Error::other("disk full"));
        let (_dir, src, to) = tree();
        let err = move_path_with(&src, &to.join("tree"), cross_device, failing).unwrap_err();
        assert!(err.to_string().starts_with("disk full (destination left at"), "{err}");
        assert!(src.join("a.md").is_file() && src.join("deep/deeper/n.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_a_folder_refuses_the_fallback_before_anything_is_copied() {
        let (_dir, src, to) = tree();
        std::os::unix::fs::symlink(src.join("a.md"), src.join("deep/link.md")).unwrap();
        let dst = to.join("tree");
        let err = move_path_with(&src, &dst, cross_device, std_copy).unwrap_err();
        assert!(matches!(err, MoveError::NotPlain(ref p) if *p == src.join("deep/link.md")), "{err:?}");
        assert!(!dst.exists(), "nothing was copied");
        assert!(src.join("deep/link.md").symlink_metadata().is_ok());
    }

    #[test]
    fn an_existing_destination_of_any_kind_refuses() {
        let (_dir, src, to) = tree();
        std::fs::write(to.join("a.md"), "other").unwrap();
        std::fs::create_dir(to.join("dir")).unwrap();
        let renamed = |_: &Path, _: &Path| -> io::Result<()> { panic!("must not rename") };
        for dst in [to.join("a.md"), to.join("dir")] {
            let err = move_path_with(&src.join("a.md"), &dst, renamed, std_copy).unwrap_err();
            assert!(matches!(err, MoveError::Exists(_)), "{err:?}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(to.join("missing"), to.join("dangling")).unwrap();
            let err = move_path_with(&src.join("a.md"), &to.join("dangling"), renamed, std_copy).unwrap_err();
            assert!(matches!(err, MoveError::Exists(_)), "{err:?}");
        }
        assert_eq!(std::fs::read_to_string(to.join("a.md")).unwrap(), "other");
        assert_eq!(std::fs::read_to_string(src.join("a.md")).unwrap(), "# a\n");
    }

    #[test]
    fn a_destination_inside_the_source_folder_refuses() {
        let (_dir, src, _to) = tree();
        let renamed = |_: &Path, _: &Path| -> io::Result<()> { panic!("must not rename") };
        for dst in [src.join("deep/src"), src.join("inner")] {
            let err = move_path_with(&src, &dst, renamed, std_copy).unwrap_err();
            assert!(matches!(err, MoveError::IntoItself(_)), "{err:?}");
        }
        std::fs::create_dir(src.join("x")).unwrap();
        let dotted = src.join("x/../deep/inner");
        let err = move_path_with(&src.join("deep"), &dotted, renamed, std_copy).unwrap_err();
        assert!(matches!(err, MoveError::IntoItself(_)), "{err:?}");
        assert!(src.join("deep/deeper/n.md").is_file());
    }

    #[test]
    fn another_rename_error_is_returned_and_nothing_is_copied() {
        let denied = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::PermissionDenied));
        let (_dir, src, to) = tree();
        let err = move_path_with(&src.join("a.md"), &to.join("a.md"), denied, std_copy).unwrap_err();
        assert!(matches!(err, MoveError::Io(_)), "{err:?}");
        assert!(!to.join("a.md").exists());
        assert!(src.join("a.md").is_file());
    }
}
