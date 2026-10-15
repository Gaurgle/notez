//! Core domain types: scope, project, note creation, scope resolution.

pub mod aggregate;
pub mod dates;
pub mod frontmatter;
pub mod note;
pub mod project;
pub mod resolve;
pub mod scope;

pub use dates::{NoteDate, NoteDates};
pub use note::Note;
pub use project::Project;
pub use scope::Scope;
