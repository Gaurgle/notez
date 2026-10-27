//! Document types: the `@type` tokens of the `/` filter and the type stops
//! of the `s` cycle. A row's types follow from its section's kind and, for
//! `todo`, from the row itself (see [`assign_doc_types`]). The `@` tokens
//! are this crate's own: they are taken out of the buffer before the rest
//! goes to `notez_core::filter::parse`, so the core grammar (and epoz,
//! which shares it) is unchanged.

use super::*;

/// One document type a row can have. A row can have several: a `TODO.md`
/// in a personal section is both [`Self::Todo`] and [`Self::Private`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocType {
    /// The `_todos` store and every `TODO.md`.
    Todo,
    /// Personal sections.
    Private,
    /// Public note sections (`notez/`), not docs.
    Public,
    /// The vault's global notes.
    Global,
    /// Docs sections (`docs/`).
    Docs,
    /// Local scratch sections (`.notez/`).
    Scratch,
    /// Reserved for Pinz (NZ-18): accepted, matches no row yet.
    Pinz,
}

impl DocType {
    /// Every type, in the order the help lists their tokens.
    pub(super) const ALL: [Self; 7] = [
        Self::Todo,
        Self::Private,
        Self::Public,
        Self::Global,
        Self::Docs,
        Self::Scratch,
        Self::Pinz,
    ];

    /// The word after `@` that names this type.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Private => "private",
            Self::Public => "public",
            Self::Global => "global",
            Self::Docs => "docs",
            Self::Scratch => "scratch",
            Self::Pinz => "pinz",
        }
    }

    /// The type `word` (without its `@`) names, ignoring ASCII case.
    pub(super) fn from_name(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(word))
    }

    /// This type's bit in [`TreeNode::doc_types`].
    pub(super) fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A parsed tree filter: the `@type` tokens as a set of type bits (OR
/// within the set, 0 when there are none) and everything else as
/// `notez_core::filter` reads it.
pub(super) struct TreeFilter {
    pub(super) filter: Filter,
    pub(super) types: u8,
}

impl TreeFilter {
    /// No text, tag or type constraint.
    pub(super) fn is_empty(&self) -> bool {
        self.filter.is_empty() && self.types == 0
    }

    /// Whether `node` matches on its own: the text and tag tokens as
    /// [`text::filter_matches`] rules them and, with type tokens, one of
    /// the types. A folder or section row has no type, so under a type
    /// token it is only kept as context for a matching note below it.
    pub(super) fn matches(&self, node: &TreeNode) -> bool {
        (self.types == 0 || node.doc_types & self.types != 0)
            && text::filter_matches(&self.filter, &node.name, node.flags)
    }
}

/// Split the filter `buffer` into its `@type` tokens and the rest. A token
/// is a type token only when the word after `@` names a [`DocType`]; any
/// other `@word` stays plain text.
pub(super) fn parse_filter(buffer: &str) -> TreeFilter {
    let mut types = 0u8;
    let mut rest: Vec<&str> = Vec::new();
    for token in buffer.split_whitespace() {
        match token.strip_prefix('@').and_then(DocType::from_name) {
            Some(doc_type) => types |= doc_type.bit(),
            None => rest.push(token),
        }
    }
    TreeFilter {
        filter: filter::parse(&rest.join(" ")),
        types,
    }
}

/// The filter buffer `s` leaves when it cycles back to all: `buffer`
/// without its `@type` tokens, every other token kept in its order,
/// single-spaced. An unknown `@word` is text and stays. A buffer with no
/// `@type` token comes back byte for byte, so its spacing survives and the
/// filter does not count as changed.
pub(super) fn clear_type_tokens(buffer: &str) -> String {
    let is_type_token = |token: &str| {
        token
            .strip_prefix('@')
            .and_then(DocType::from_name)
            .is_some()
    };
    if !buffer.split_whitespace().any(is_type_token) {
        return buffer.to_string();
    }
    buffer
        .split_whitespace()
        .filter(|token| !is_type_token(token))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The type bits of the rows of a section `spec`, below its section row.
fn section_types(spec: &SectionSpec) -> u8 {
    let doc_type = if spec.is_doc {
        DocType::Docs
    } else {
        match spec.scope {
            Scope::Personal => DocType::Private,
            Scope::Public => DocType::Public,
            Scope::Local => DocType::Scratch,
            Scope::Global if spec.project.is_none() => DocType::Global,
            Scope::Global => return 0,
        }
    };
    doc_type.bit()
}

/// Whether the note `node` of section `spec` is a todo: a `TODO.md`
/// anywhere, or any note under the global store's `_todos/`.
fn is_todo(node: &TreeNode, spec: &SectionSpec) -> bool {
    !node.is_dir
        && (node.name == "TODO.md"
            || (section_types(spec) == DocType::Global.bit()
                && node.path.starts_with(spec.root.join("_todos"))))
}

/// Whether `node` is a todo document by [`is_todo`]'s rule, as
/// [`assign_doc_types`] recorded it in [`TreeNode::doc_types`].
pub(super) fn is_todo_note(node: &TreeNode) -> bool {
    node.doc_types & DocType::Todo.bit() != 0
}

/// Set [`TreeNode::doc_types`] on every note from its section and itself.
/// Folders and section rows get none, so a type matches them only as the
/// ancestors of a matching note (as text tokens do), and a type filter
/// expands only the folders on the path to one. Runs on every build and
/// every pass of [`apply_scope`], so a note renamed in place is typed by
/// its new name.
pub(super) fn assign_doc_types(nodes: &mut [TreeNode], sections: &[SectionSpec]) {
    for node in nodes.iter_mut() {
        node.doc_types = match sections.get(node.section) {
            Some(spec) if !node.is_dir => {
                let todo = if is_todo(node, spec) {
                    DocType::Todo.bit()
                } else {
                    0
                };
                section_types(spec) | todo
            }
            _ => 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::FLAG_PRIO;

    #[test]
    fn every_type_token_parses_and_is_taken_out_of_the_text() {
        for doc_type in DocType::ALL {
            let f = parse_filter(&format!("@{}", doc_type.name()));
            assert_eq!(f.types, doc_type.bit(), "{doc_type:?}");
            assert!(f.filter.is_empty(), "{doc_type:?}");
            assert!(!f.is_empty(), "{doc_type:?}");
        }
        assert_eq!(parse_filter("@TODO").types, DocType::Todo.bit());
        assert!(parse_filter("").is_empty());
    }

    #[test]
    fn several_type_tokens_or_together() {
        let f = parse_filter("@todo @public");
        assert_eq!(f.types, DocType::Todo.bit() | DocType::Public.bit());
    }

    #[test]
    fn an_unknown_type_token_is_plain_text() {
        let f = parse_filter("@nope @ x@todo");
        assert_eq!(f.types, 0);
        assert_eq!(f.filter.text_tokens, ["@nope", "@", "x@todo"]);
    }

    #[test]
    fn type_tokens_combine_with_tag_and_text_tokens() {
        let f = parse_filter("plan @public #prio @docs flz");
        assert_eq!(f.types, DocType::Public.bit() | DocType::Docs.bit());
        assert_eq!(f.filter.text_tokens, ["plan", "flz"]);
        assert_eq!(f.filter.tag_sets, [FLAG_PRIO]);
    }

    #[test]
    fn cycling_back_to_all_drops_only_the_type_tokens() {
        assert_eq!(
            clear_type_tokens("@todo buy #prio @Public @nope"),
            "buy #prio @nope"
        );
        assert_eq!(clear_type_tokens("@docs"), "");
        assert_eq!(clear_type_tokens(""), "");
    }

    #[test]
    fn cycling_back_to_all_without_type_tokens_leaves_the_buffer_as_typed() {
        assert_eq!(clear_type_tokens("foo  bar "), "foo  bar ");
        assert_eq!(clear_type_tokens(" @nope  #prio"), " @nope  #prio");
    }

    #[test]
    fn a_type_token_expands_only_folders_on_the_path_to_a_matching_note() {
        let mut sections = every_kind();
        sections[1].files.push(PathBuf::from("/p/notez/plans/b.md"));
        sections[1].dirs = vec![
            PathBuf::from("/p/notez/empty"),
            PathBuf::from("/p/notez/plans"),
        ];
        let (mut nodes, _) = build_forest(&sections);
        // The auto-expand step of the event loop, then the filter cleared.
        let keep = compute_filter_keep(&nodes, &parse_filter("@public"));
        for (node, keep) in nodes.iter_mut().zip(keep) {
            if keep && node.is_dir {
                node.expanded = true;
            }
        }
        assert!(nodes[row(&nodes, "/p/notez/plans")].expanded);
        assert!(nodes[row(&nodes, "/p/notez")].expanded);
        assert!(!nodes[row(&nodes, "/p/notez/empty")].expanded);
        assert!(!nodes[row(&nodes, "/n/_todos")].expanded);
    }

    /// The notes `buffer` shows in [`every_kind`] with every folder open.
    fn notes(buffer: &str) -> Vec<String> {
        let sections = every_kind();
        let (mut nodes, _) = build_forest(&sections);
        apply_scope(&mut nodes, &sections, ViewScope::All);
        for node in &mut nodes {
            node.expanded = true;
        }
        compute_visible(&nodes, buffer)
            .into_iter()
            .filter(|&i| !nodes[i].is_dir)
            .map(|i| nodes[i].path.display().to_string())
            .collect()
    }

    #[test]
    fn each_type_matches_its_own_rows() {
        assert_eq!(
            notes("@todo"),
            [
                "/n/personal/proj/TODO.md",
                "/p/notez/TODO.md",
                "/p/.notez/TODO.md",
                "/n/_todos/work/TODO.md",
                "/n/_todos/work/w.md",
                "/n/TODO.md"
            ]
        );
        assert_eq!(
            notes("@private"),
            ["/n/personal/proj/TODO.md", "/n/personal/proj/p.md"]
        );
        assert_eq!(
            notes("@public"),
            ["/p/notez/TODO.md", "/p/notez/u.md", "/o/notez/o.md"]
        );
        assert_eq!(
            notes("@global"),
            [
                "/n/_todos/work/TODO.md",
                "/n/_todos/work/w.md",
                "/n/TODO.md",
                "/n/g.md"
            ]
        );
        assert_eq!(notes("@docs"), ["/p/docs/c.md"]);
        assert_eq!(notes("@scratch"), ["/p/.notez/TODO.md", "/p/.notez/l.md"]);
        assert!(notes("@pinz").is_empty());
    }

    #[test]
    fn the_todo_notes_are_the_todo_type_rows() {
        let sections = every_kind();
        let (nodes, _) = build_forest(&sections);
        let mut todo: Vec<String> = nodes
            .iter()
            .filter(|n| is_todo_note(n))
            .map(|n| n.path.display().to_string())
            .collect();
        todo.sort();
        assert_eq!(
            todo,
            [
                "/n/TODO.md",
                "/n/_todos/work/TODO.md",
                "/n/_todos/work/w.md",
                "/n/personal/proj/TODO.md",
                "/p/.notez/TODO.md",
                "/p/notez/TODO.md"
            ]
        );
    }

    #[test]
    fn types_combine_with_each_other_and_with_text() {
        assert_eq!(
            notes("@docs @scratch"),
            ["/p/docs/c.md", "/p/.notez/TODO.md", "/p/.notez/l.md"]
        );
        assert_eq!(
            notes("@todo @private"),
            [
                "/n/personal/proj/TODO.md",
                "/n/personal/proj/p.md",
                "/p/notez/TODO.md",
                "/p/.notez/TODO.md",
                "/n/_todos/work/TODO.md",
                "/n/_todos/work/w.md",
                "/n/TODO.md"
            ]
        );
        assert_eq!(
            notes("@global todo"),
            ["/n/_todos/work/TODO.md", "/n/TODO.md"]
        );
        assert!(notes("@docs zzz").is_empty());
    }

    #[test]
    fn a_section_with_no_matching_row_is_hidden_under_a_type_token() {
        let mut sections = every_kind();
        sections[3].files.clear(); // an empty scratch store
        let (nodes, _) = build_forest(&sections);
        let shown: Vec<&str> = compute_visible(&nodes, "@scratch")
            .into_iter()
            .map(|i| nodes[i].name.as_str())
            .collect();
        assert!(shown.is_empty(), "{shown:?}");
        let shown: Vec<&str> = compute_visible(&nodes, "@docs")
            .into_iter()
            .map(|i| nodes[i].name.as_str())
            .collect();
        assert_eq!(shown, ["/p/docs"], "only the section with docs");
    }

    #[test]
    fn a_renamed_row_is_typed_by_its_new_name() {
        let sections = every_kind();
        let (mut nodes, _) = build_forest(&sections);
        let i = row(&nodes, "/p/notez/u.md");
        assert_eq!(nodes[i].doc_types, DocType::Public.bit());
        nodes[i].name = "TODO.md".to_string();
        apply_scope(&mut nodes, &sections, ViewScope::All);
        assert_eq!(
            nodes[i].doc_types,
            DocType::Public.bit() | DocType::Todo.bit()
        );
    }
}
