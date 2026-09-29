//! Note scope. Two axes (decided 2026-07-06): accessibility (personal vs
//! public) x binding (project vs global), plus a machine-only scratch tier.
//!
//! - `Personal` = personal + project: `<notez_root>/personal/<project>/`,
//!   synced via your own notez remote, invisible to teammates.
//! - `Public` = public + project: `<cwd>/notez/`, committed with the
//!   project, visible to collaborators.
//! - `Global` = personal + global: `<notez_root>/`, notes bound to no
//!   project (the notez repo itself is private).
//! - `Local` = scratch: `<cwd>/.notez/`, gitignored, this machine only,
//!   never syncs. Presented to users as "scratch"; the wire name stays
//!   `local` for compatibility.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The four note scopes.
///
/// Derived from CLI flags: default = `Personal`, `-l` = `Local`,
/// `-p` = `Public`, `-g` = `Global`. Flags are mutually exclusive; if more
/// than one is given, the precedence is global > public > local > personal.
///
/// Serializes lowercase (`"local"`, `"personal"`, …) so the frontend and the
/// `Display` impl agree on the wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Local,
    Personal,
    Public,
    Global,
}

impl Scope {
    /// Resolve from CLI flags. Precedence: `-g` > `-p` > `-l` > default.
    ///
    /// Default (no flag) is `Personal`. Personal is the most common scope
    /// in notez: notes you write about a specific project, syncing across
    /// your own machines but not visible to teammates.
    pub fn from_flags(global: bool, public: bool, local: bool) -> Self {
        if global {
            Self::Global
        } else if public {
            Self::Public
        } else if local {
            Self::Local
        } else {
            Self::Personal
        }
    }

    /// Nerdfont icon. Lock for scratch, user for personal, globe for
    /// public, home for global.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Local => "\u{f023}",    // lock
            Self::Personal => "\u{f007}", // user
            Self::Public => "\u{f0ac}",   // globe
            Self::Global => "\u{f015}",   // home
        }
    }
}

impl Scope {
    /// Human-facing word for the scope (the wire form via `Display` stays
    /// unchanged): scratch / personal / public / notez.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Local => "scratch",
            Self::Personal => "personal",
            Self::Public => "public",
            Self::Global => "notez",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Local => "local",
            Self::Personal => "personal",
            Self::Public => "public",
            Self::Global => "global",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_personal() {
        assert_eq!(Scope::from_flags(false, false, false), Scope::Personal);
    }

    #[test]
    fn local_flag_picks_local() {
        assert_eq!(Scope::from_flags(false, false, true), Scope::Local);
    }

    #[test]
    fn public_flag_picks_public() {
        assert_eq!(Scope::from_flags(false, true, false), Scope::Public);
    }

    #[test]
    fn global_flag_picks_global() {
        assert_eq!(Scope::from_flags(true, false, false), Scope::Global);
    }

    #[test]
    fn precedence_global_over_public_over_local() {
        assert_eq!(Scope::from_flags(true, true, true), Scope::Global);
        assert_eq!(Scope::from_flags(false, true, true), Scope::Public);
        assert_eq!(Scope::from_flags(false, false, true), Scope::Local);
    }

    #[test]
    fn icons_differ_per_scope() {
        let icons: Vec<&str> = [Scope::Local, Scope::Personal, Scope::Public, Scope::Global]
            .iter()
            .map(|s| s.icon())
            .collect();
        // All four icons distinct.
        for i in 0..icons.len() {
            for j in (i + 1)..icons.len() {
                assert_ne!(icons[i], icons[j], "scopes {} and {} share icon", i, j);
            }
        }
    }

    #[test]
    fn display_strings() {
        assert_eq!(format!("{}", Scope::Local), "local");
        assert_eq!(format!("{}", Scope::Personal), "personal");
        assert_eq!(format!("{}", Scope::Public), "public");
        assert_eq!(format!("{}", Scope::Global), "global");
    }
}
