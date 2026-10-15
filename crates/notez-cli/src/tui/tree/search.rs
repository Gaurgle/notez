//! Row visibility: what shows for the expansion state and the filter.

use super::*;

// --- Visibility & filtering ---

/// The rows the expansion state shows, in tree order. A row the view scope
/// hides (see [`apply_scope`]) shows neither itself nor any row under it.
pub(super) fn get_visible_nodes(nodes: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.hidden {
            continue;
        }
        if node.depth == 0 {
            visible.push(idx);
            continue;
        }
        let mut ancestor_expanded = true;
        let mut check = node.parent_idx;
        while let Some(p) = check {
            if !nodes[p].expanded || nodes[p].hidden {
                ancestor_expanded = false;
                break;
            }
            check = nodes[p].parent_idx;
        }
        if ancestor_expanded {
            visible.push(idx);
        }
    }
    visible
}

/// Keep-mask for the filter: a node matches on name (text tokens as fuzzy
/// subsequences, see [`text::filter_matches`]) + flags + `@type` tokens
/// (see [`TreeFilter::matches`]), and every match pulls in its ancestor
/// chain so results keep their tree context.
pub(super) fn compute_filter_keep(nodes: &[TreeNode], f: &TreeFilter) -> Vec<bool> {
    let n = nodes.len();
    let mut keep = vec![false; n];
    for (i, node) in nodes.iter().enumerate() {
        if f.matches(node) {
            keep[i] = true;
        }
    }
    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let mut cur = nodes[i].parent_idx;
        while let Some(p) = cur {
            if keep[p] {
                break;
            }
            keep[p] = true;
            cur = nodes[p].parent_idx;
        }
    }
    keep
}

/// Visible-list builder shared by the render pass and the key handler so
/// cursor positions always match the rendered rows.
pub(super) fn compute_visible(nodes: &[TreeNode], search_buffer: &str) -> Vec<usize> {
    let f = parse_filter(search_buffer);
    let mut v = get_visible_nodes(nodes);
    if f.is_empty() {
        return v;
    }
    let keep = compute_filter_keep(nodes, &f);
    v.retain(|&i| keep[i]);
    v
}

/// The filter buffer `0` leaves: `buffer` without its `#tag` tokens (as
/// `notez_core::filter::parse` reads them, any token starting with `#`),
/// the free text and `@type` tokens kept in their order, single-spaced.
pub(super) fn clear_tag_tokens(buffer: &str) -> String {
    buffer
        .split_whitespace()
        .filter(|token| !token.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The single row the list shows when the tree has rows but none is
/// visible: [`text::NO_MATCHES`] when the filter hid them, otherwise the
/// view scope shows no section. `None` while a row is visible, or when the
/// tree has no rows at all ([`empty_state_line`] covers that).
pub(super) fn no_rows_text(
    nodes: &[TreeNode],
    visible: &[usize],
    search_buffer: &str,
    scope: ViewScope,
) -> Option<String> {
    if nodes.is_empty() || !visible.is_empty() {
        return None;
    }
    Some(if parse_filter(search_buffer).is_empty() {
        format!("nothing in the {} scope: s shows the next", scope.label())
    } else {
        text::NO_MATCHES.to_string()
    })
}

pub(super) fn find_top_dir(nodes: &[TreeNode], idx: usize) -> Option<usize> {
    if idx >= nodes.len() {
        return None;
    }
    if nodes[idx].depth == 0 {
        return Some(idx);
    }
    let mut cur = nodes[idx].parent_idx;
    while let Some(p) = cur {
        if nodes[p].depth == 0 {
            return Some(p);
        }
        cur = nodes[p].parent_idx;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::FLAG_PRIO;

    #[test]
    fn filter_keeps_matches_and_ancestors() {
        let s = spec("/r", "S", &["dir/target.md", "dir/other.md"]);
        let (nodes, _) = build_forest(&[s]);
        let f = parse_filter("target");
        let keep = compute_filter_keep(&nodes, &f);
        let kept: Vec<&str> = nodes
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(n, _)| n.name.as_str())
            .collect();
        assert!(kept.contains(&"target.md"));
        assert!(kept.contains(&"dir"));
        assert!(kept.contains(&"S"));
        assert!(!kept.contains(&"other.md"));
    }

    #[test]
    fn filter_text_matches_names_by_subsequence_and_keeps_ancestors() {
        let s = spec("/r", "S", &["tools/fleetz.md", "tools/other.md"]);
        let (mut nodes, _) = build_forest(&[s]);
        for node in &mut nodes {
            node.expanded = true;
        }
        let shown: Vec<&str> = compute_visible(&nodes, "flz")
            .into_iter()
            .map(|i| nodes[i].name.as_str())
            .collect();
        assert_eq!(shown, ["S", "tools", "fleetz.md"]);
        // Tag tokens still gate: no note carries #prio.
        assert!(compute_visible(&nodes, "flz #prio").is_empty());
    }

    #[test]
    fn zero_clears_the_tag_tokens_and_keeps_the_free_text() {
        assert_eq!(clear_tag_tokens("buy #prio milk #13"), "buy milk");
        assert_eq!(clear_tag_tokens("#important #"), "");
        assert_eq!(clear_tag_tokens("  flz   md "), "flz md");
        assert_eq!(clear_tag_tokens(""), "");
        let cleared = filter::parse(&clear_tag_tokens("flz #prio #idea"));
        assert_eq!(cleared.text_tokens, vec!["flz"]);
        assert!(cleared.tag_sets.is_empty());
    }

    #[test]
    fn a_filter_that_hides_every_row_shows_one_no_matches_row() {
        let s = spec("/r", "S", &["a.md"]);
        let (nodes, _) = build_forest(&[s]);
        let visible = compute_visible(&nodes, "zzz");
        assert!(visible.is_empty());
        assert_eq!(
            no_rows_text(&nodes, &visible, "zzz", ViewScope::All).as_deref(),
            Some("no matches")
        );
        let visible = compute_visible(&nodes, "a");
        assert_eq!(no_rows_text(&nodes, &visible, "a", ViewScope::All), None);
        // No rows at all is the empty state's, not this row's.
        assert_eq!(no_rows_text(&[], &[], "zzz", ViewScope::All), None);
    }

    #[test]
    fn a_scope_without_sections_says_so_and_an_empty_store_matches_nothing() {
        // An empty store still lists its section row; a filter hides it.
        let mut s = spec("/r", "S", &[]);
        s.is_current = true;
        s.scope = Scope::Local;
        s.project = Some("p".to_string());
        let sections = [s];
        let (mut nodes, _) = build_forest(&sections);
        assert_eq!(compute_visible(&nodes, ""), [0]);
        let visible = compute_visible(&nodes, "zzz");
        assert_eq!(
            no_rows_text(&nodes, &visible, "zzz", ViewScope::Scratch).as_deref(),
            Some("no matches")
        );
        apply_scope(&mut nodes, &sections, ViewScope::Global);
        let visible = compute_visible(&nodes, "");
        assert_eq!(
            no_rows_text(&nodes, &visible, "", ViewScope::Global).as_deref(),
            Some("nothing in the global scope: s shows the next")
        );
        assert_eq!(
            no_rows_text(&nodes, &visible, "zzz", ViewScope::Global).as_deref(),
            Some("no matches")
        );
    }

    #[test]
    fn a_type_token_filters_by_type_and_ands_with_text() {
        let mut sections = project_sections();
        sections[0]
            .files
            .push(PathBuf::from("/n/personal/proj/TODO.md"));
        let (mut nodes, _) = build_forest(&sections);
        apply_scope(&mut nodes, &sections, ViewScope::All);
        let shown = |nodes: &[TreeNode], buffer: &str| -> Vec<String> {
            compute_visible(nodes, buffer)
                .into_iter()
                .filter(|&i| !nodes[i].is_dir)
                .map(|i| nodes[i].path.display().to_string())
                .collect()
        };
        for node in &mut nodes {
            node.expanded = true;
        }
        assert_eq!(shown(&nodes, "@todo"), ["/n/personal/proj/TODO.md"]);
        assert_eq!(shown(&nodes, "@public b"), ["/p/notez/plans/b.md"]);
        assert!(shown(&nodes, "@public zzz").is_empty());
    }

    #[test]
    fn zero_keeps_the_type_tokens() {
        assert_eq!(clear_tag_tokens("@todo #prio buy @nope"), "@todo buy @nope");
    }

    #[test]
    fn derive_dir_flags_aggregates_and_clears() {
        let s = spec("/r", "S", &["dir/a.md"]);
        let (mut nodes, _) = build_forest(&[s]);
        let file = nodes.iter().position(|n| n.name == "a.md").unwrap();
        nodes[file].flags = FLAG_PRIO;
        derive_dir_flags(&mut nodes);
        assert_eq!(nodes[0].flags, FLAG_PRIO);
        nodes[file].flags = 0;
        derive_dir_flags(&mut nodes);
        assert_eq!(nodes[0].flags, 0, "stale bits must drop");
    }
}
