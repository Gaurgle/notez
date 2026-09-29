//! Tilde-relative path handling.
//!
//! All persisted paths in notez (config, registry, metadata) store paths as
//! tilde-relative (`~/foo/bar`). They are expanded to absolute paths at runtime
//! using the current user's home directory. This is what makes the same config
//! work on machines with different usernames.

use std::path::{Path, PathBuf};

/// Expand a leading `~` in `path` to the current user's home directory.
///
/// Paths without a leading `~` are returned unchanged. If the home directory
/// cannot be determined, returns the input unchanged.
pub fn expand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Contract a path to use `~` if it lives under the home directory.
///
/// Inverse of [`expand`]. Used when serializing paths back to disk so the
/// stored form is portable.
pub fn contract(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.to_string_lossy().into_owned();
    };
    match path.strip_prefix(&home) {
        Ok(rest) => {
            if rest.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rest.to_string_lossy())
            }
        }
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_prefix() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand("~/foo/bar"), home.join("foo/bar"));
    }

    #[test]
    fn expand_bare_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand("~"), home);
    }

    #[test]
    fn expand_passes_through_absolute() {
        assert_eq!(expand("/tmp/foo"), PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn contract_roundtrips_home_paths() {
        let home = dirs::home_dir().unwrap();
        let p = home.join("notez").join("file.md");
        assert_eq!(contract(&p), "~/notez/file.md");
    }

    #[test]
    fn contract_leaves_non_home_paths() {
        let p = PathBuf::from("/tmp/foo");
        assert_eq!(contract(&p), "/tmp/foo");
    }

    #[test]
    fn contract_home_itself() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(contract(&home), "~");
    }

    #[test]
    fn expand_contract_roundtrip() {
        let home = dirs::home_dir().unwrap();
        let original = "~/repos/sigma/App2";
        let expanded = expand(original);
        let contracted = contract(&expanded);
        assert_eq!(contracted, original);
        assert_eq!(expanded, home.join("repos/sigma/App2"));
    }
}
