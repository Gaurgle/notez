//! Row visibility: what shows for the expansion state and the filter.

use super::*;

// --- Visibility & filtering ---

pub(super) fn get_visible_nodes(nodes: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.depth == 0 {
            visible.push(idx);
            continue;
        }
        let mut ancestor_expanded = true;
        let mut check = node.parent_idx;
        while let Some(p) = check {
            if !nodes[p].expanded {
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

/// Keep-mask for the filter: a node matches on name + flags, and every
/// match pulls in its ancestor chain so results keep their tree context.
pub(super) fn compute_filter_keep(nodes: &[TreeNode], f: &Filter) -> Vec<bool> {
    let n = nodes.len();
    let mut keep = vec![false; n];
    for (i, node) in nodes.iter().enumerate() {
        if f.matches(&node.name, node.flags) {
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
    let f = filter::parse(search_buffer);
    let mut v = get_visible_nodes(nodes);
    if f.is_empty() {
        return v;
    }
    let keep = compute_filter_keep(nodes, &f);
    v.retain(|&i| keep[i]);
    v
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
        let f = filter::parse("target");
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
