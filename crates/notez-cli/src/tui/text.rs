//! Text utilities shared by all TUIs.
//!
//! - [`fuzzy_match`] / [`fuzzy_positions`]: case-insensitive subsequence
//!   match (`flz` finds `fleetz`) and the characters it matched.
//! - [`filter_matches`] / [`filter_match_positions`]: a parsed `/` filter
//!   applied with fuzzy text tokens, as the tree browser and todoz use it,
//!   and the characters to highlight; [`highlight_spans`] draws them.
//! - [`prev_char_boundary`] / [`next_char_boundary`]: UTF-8-safe cursor
//!   movement over a `String` buffer. `String::insert` and `String::remove`
//!   require byte indices that fall on character boundaries, so any
//!   left-arrow / right-arrow / backspace logic that touches strings with
//!   non-ASCII characters (`å`, `ö`, emoji) must use these helpers.
//! - [`mouse_x_to_filter_dot`]: maps a mouse column onto the 5-slot filter
//!   strip the tree and todoz both render. The anchor is documented inline.

use notez_core::filter::Filter;
use ratatui::style::Style;
use ratatui::text::Span;

/// The list's single row when the filter hides every row.
pub const NO_MATCHES: &str = "no matches";

/// Case-insensitive subsequence match: every char of `needle` appears in
/// `haystack` in order, not necessarily adjacent. Returns true when
/// `needle` is empty.
pub fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    fuzzy_positions(haystack, needle).is_some()
}

/// The byte index in `haystack` of each char of `needle`, matched
/// case-insensitively and in order, each at the leftmost place still
/// free. `None` when `needle` is not a subsequence of `haystack`; an empty
/// `needle` matches with no positions. Nothing is scored or ranked.
///
/// Both strings are lowercased whole, as `notez_core::filter` lowercases
/// them, so context rules such as the Greek final sigma (`ΣΑΣ` lowercases
/// to `σας`) agree with it. A char may lowercase to several chars; each of
/// them points back at the byte index of the char it came from.
pub fn fuzzy_positions(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    let needle = needle.to_lowercase();
    let mut wanted = needle.chars().peekable();
    let mut positions: Vec<usize> = Vec::new();
    for (idx, ch) in lowercase_with_origins(haystack) {
        let Some(&next) = wanted.peek() else {
            break;
        };
        if ch == next {
            if positions.last() != Some(&idx) {
                positions.push(idx);
            }
            wanted.next();
        }
    }
    wanted.peek().is_none().then_some(positions)
}

/// `text.to_lowercase()` char by char, each with the byte index in `text`
/// of the char it came from. Whole-string lowercasing maps every char to
/// as many chars as lowercasing it alone does (only which chars can depend
/// on context), so the two line up char for char.
fn lowercase_with_origins(text: &str) -> Vec<(usize, char)> {
    let lowercase = text.to_lowercase();
    let mut lowered = lowercase.chars();
    let mut out = Vec::new();
    for (idx, ch) in text.char_indices() {
        for _ in 0..ch.to_lowercase().count() {
            if let Some(low) = lowered.next() {
                out.push((idx, low));
            }
        }
    }
    out
}

/// Whether `text` with tag bits `flags` passes `f`: every text token is a
/// fuzzy subsequence of `text` (see [`fuzzy_match`]), and the tag tokens
/// pass as [`Filter::matches`] rules them, AND across tokens and OR within
/// one. The tags are left to `notez_core::filter` so its rules stay the
/// only copy; only the text half differs from [`Filter::matches`], which
/// matches substrings.
pub fn filter_matches(f: &Filter, text: &str, flags: u8) -> bool {
    let tags_only = Filter {
        text_tokens: Vec::new(),
        tag_sets: f.tag_sets.clone(),
    };
    f.text_tokens.iter().all(|token| fuzzy_match(text, token)) && tags_only.matches(text, flags)
}

/// The byte indices in `text` to highlight for `f`: the union of the
/// [`fuzzy_positions`] of every text token that matches `text`, ascending.
/// A token that does not match adds nothing, so a row kept only as context
/// for a match below it shows what it does match and no more.
pub fn filter_match_positions(f: &Filter, text: &str) -> Vec<usize> {
    let mut positions: Vec<usize> = f
        .text_tokens
        .iter()
        .filter_map(|token| fuzzy_positions(text, token))
        .flatten()
        .collect();
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// `text` as spans in `base`, the chars whose byte index is in `matched`
/// patched with `highlight`. `text` may be a piece of a longer string that
/// starts at byte `start` of it, with `matched` indexing that string, so a
/// wrapped row can highlight each of its lines. Adjacent chars of the same
/// kind share a span.
pub fn highlight_spans(
    text: &str,
    start: usize,
    matched: &[usize],
    base: Style,
    highlight: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_lit = false;
    for (idx, ch) in text.char_indices() {
        let lit = matched.binary_search(&(start + idx)).is_ok();
        if lit != run_lit && !run.is_empty() {
            let style = if run_lit { base.patch(highlight) } else { base };
            spans.push(Span::styled(std::mem::take(&mut run), style));
        }
        run_lit = lit;
        run.push(ch);
    }
    if !run.is_empty() || spans.is_empty() {
        let style = if run_lit { base.patch(highlight) } else { base };
        spans.push(Span::styled(run, style));
    }
    spans
}

/// Return the byte index of the previous char boundary at or before `idx`.
///
/// Returns 0 when `idx` is 0 or already past the start.
pub fn prev_char_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut i = idx.min(s.len()).saturating_sub(1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Return the byte index of the next char boundary after `idx`.
///
/// Returns `s.len()` when `idx` is at or past the end.
pub fn next_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Map a mouse x-column onto one of the 5 tag-filter dot positions.
///
/// The filter strip renders five dots at fixed positions starting at
/// `column_offset`. Each dot occupies one cell and has one cell of trailing
/// space, so the strip spans 10 columns total. Returns `Some(0..5)` when
/// the mouse falls on a dot, `None` when it falls between dots or outside
/// the strip.
///
/// notez-cli anchors the filter strip at `column_offset = x_origin + 5`:
/// 4 columns of highlight indent on each row plus 1 column for the
/// flag-leading space. That alignment makes the strip's dots line up
/// vertically with each row's flag dots, so a click on the strip's column 3
/// also matches column 3 of any row.
pub fn mouse_x_to_filter_dot(mouse_x: u16, strip_x: u16) -> Option<u8> {
    if mouse_x < strip_x {
        return None;
    }
    let rel = (mouse_x - strip_x) as usize;
    if rel >= 10 {
        return None;
    }
    // Even offsets (0, 2, 4, 6, 8) are dot positions.
    if rel % 2 != 0 {
        return None;
    }
    Some((rel / 2) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_empty_needle_matches_anything() {
        assert!(fuzzy_match("anything", ""));
        assert!(fuzzy_match("", ""));
    }

    #[test]
    fn fuzzy_match_is_case_insensitive() {
        assert!(fuzzy_match("Hello World", "hello"));
        assert!(fuzzy_match("hello world", "WORLD"));
    }

    #[test]
    fn fuzzy_match_substring() {
        assert!(fuzzy_match("hello-world.md", "world"));
        assert!(!fuzzy_match("hello-world.md", "earth"));
    }

    #[test]
    fn fuzzy_match_finds_a_subsequence() {
        assert!(fuzzy_match("fleetz", "flz"));
        assert!(fuzzy_match("Fleetz.md", "FLZ"));
        assert!(!fuzzy_match("fleetz", "zfl"), "order matters");
        assert!(!fuzzy_match("fleetz", "flzz"), "each char used once");
    }

    #[test]
    fn fuzzy_positions_are_the_leftmost_byte_indices_of_each_char() {
        assert_eq!(fuzzy_positions("fleetz", "flz"), Some(vec![0, 1, 5]));
        assert_eq!(fuzzy_positions("Fleetz", "fE"), Some(vec![0, 2]));
        assert_eq!(fuzzy_positions("fleetz", ""), Some(vec![]));
        assert_eq!(fuzzy_positions("fleetz", "q"), None);
        assert_eq!(fuzzy_positions("", "a"), None);
        // Byte indices, not char counts: "å" takes two bytes.
        assert_eq!(fuzzy_positions("åsa", "ÅA"), Some(vec![0, 3]));
    }

    #[test]
    fn fuzzy_matching_lowercases_whole_strings_as_the_core_filter_does() {
        use notez_core::filter::parse;
        // A final sigma: "ΣΑΣ" lowercases to "σας" as a whole string.
        assert!(parse("σας").matches("ΣΑΣ", 0), "core matches it");
        assert!(fuzzy_match("ΣΑΣ", "σας"));
        assert!(filter_matches(&parse("σας"), "ΣΑΣ", 0));
        assert_eq!(fuzzy_positions("ΣΑΣ", "σας"), Some(vec![0, 2, 4]));
        assert_eq!(fuzzy_positions("ΣΑΣ", "ΣΑΣ"), Some(vec![0, 2, 4]));
        // A char that lowercases to two chars points both back at itself.
        assert_eq!(fuzzy_positions("İx", "x"), Some(vec![2]));
        assert_eq!(fuzzy_positions("İx", "i"), Some(vec![0]));
    }

    #[test]
    fn filter_text_tokens_match_fuzzily_and_tags_keep_their_rules() {
        use notez_core::filter::parse;
        use notez_core::tags::{FLAG_IMPORTANT, FLAG_LONGTERM, FLAG_PRIO};
        assert!(filter_matches(&parse("flz"), "fleetz.md", 0));
        // Text tokens AND: both must match.
        assert!(filter_matches(&parse("flz md"), "fleetz.md", 0));
        assert!(!filter_matches(&parse("flz qq"), "fleetz.md", 0));
        // Tag tokens AND across, OR within, as in notez_core::filter.
        assert!(filter_matches(&parse("flz #prio"), "fleetz.md", FLAG_PRIO));
        assert!(!filter_matches(&parse("flz #prio"), "fleetz.md", 0));
        assert!(!filter_matches(&parse("#prio #important"), "x", FLAG_PRIO));
        assert!(filter_matches(&parse("#13"), "x", FLAG_LONGTERM));
        assert!(filter_matches(&parse("#13"), "x", FLAG_IMPORTANT));
        assert!(!filter_matches(&parse("#13"), "x", FLAG_PRIO));
        assert!(filter_matches(&parse(""), "anything", 0));
    }

    #[test]
    fn filter_match_positions_join_every_matching_token() {
        use notez_core::filter::parse;
        assert_eq!(
            filter_match_positions(&parse("flz md"), "fleetz.md"),
            vec![0, 1, 5, 7, 8]
        );
        // A token that does not match lights nothing; tags light nothing.
        assert_eq!(
            filter_match_positions(&parse("fl qq #prio"), "fleetz"),
            vec![0, 1]
        );
        assert!(filter_match_positions(&parse(""), "fleetz").is_empty());
    }

    #[test]
    fn highlight_spans_split_runs_at_the_matched_chars() {
        let base = Style::default().fg(ratatui::style::Color::White);
        let lit = Style::default().fg(ratatui::style::Color::Yellow);
        let runs = |spans: Vec<Span<'static>>| -> Vec<(String, Style)> {
            spans
                .into_iter()
                .map(|s| (s.content.into_owned(), s.style))
                .collect()
        };
        let got = runs(highlight_spans("fleetz", 0, &[0, 1, 5], base, lit));
        assert_eq!(
            got,
            [
                ("fl".to_string(), base.patch(lit)),
                ("eet".to_string(), base),
                ("z".to_string(), base.patch(lit))
            ]
        );
        // A piece starting at byte 3 of the matched string.
        let got = runs(highlight_spans("etz", 3, &[0, 1, 5], base, lit));
        assert_eq!(
            got,
            [("et".to_string(), base), ("z".to_string(), base.patch(lit))]
        );
        // No match: one span in the base style; empty text keeps one span.
        assert_eq!(highlight_spans("abc", 0, &[], base, lit).len(), 1);
        assert_eq!(highlight_spans("", 0, &[], base, lit)[0].content, "");
    }

    #[test]
    fn prev_boundary_handles_ascii() {
        let s = "abcdef";
        assert_eq!(prev_char_boundary(s, 0), 0);
        assert_eq!(prev_char_boundary(s, 1), 0);
        assert_eq!(prev_char_boundary(s, 3), 2);
        assert_eq!(prev_char_boundary(s, 6), 5);
        assert_eq!(prev_char_boundary(s, 100), 5);
    }

    #[test]
    fn prev_boundary_handles_multibyte() {
        // "åö" is 4 bytes: å = 2, ö = 2.
        let s = "åö";
        assert_eq!(s.len(), 4);
        // From byte 4 (end), prev boundary is 2 (start of ö).
        assert_eq!(prev_char_boundary(s, 4), 2);
        // From byte 2 (start of ö), prev boundary is 0.
        assert_eq!(prev_char_boundary(s, 2), 0);
        // From byte 3 (middle of ö), prev boundary is 2.
        assert_eq!(prev_char_boundary(s, 3), 2);
    }

    #[test]
    fn next_boundary_handles_ascii() {
        let s = "abc";
        assert_eq!(next_char_boundary(s, 0), 1);
        assert_eq!(next_char_boundary(s, 2), 3);
        assert_eq!(next_char_boundary(s, 3), 3);
        assert_eq!(next_char_boundary(s, 100), 3);
    }

    #[test]
    fn next_boundary_handles_multibyte() {
        let s = "åö"; // 4 bytes
        // From 0, next boundary is 2 (after å).
        assert_eq!(next_char_boundary(s, 0), 2);
        // From 2, next boundary is 4.
        assert_eq!(next_char_boundary(s, 2), 4);
        // From 1 (middle of å), next boundary is 2.
        assert_eq!(next_char_boundary(s, 1), 2);
    }

    #[test]
    fn mouse_dot_maps_to_each_of_five_positions() {
        // Strip starts at column 5. Dots are at columns 5, 7, 9, 11, 13.
        assert_eq!(mouse_x_to_filter_dot(5, 5), Some(0));
        assert_eq!(mouse_x_to_filter_dot(7, 5), Some(1));
        assert_eq!(mouse_x_to_filter_dot(9, 5), Some(2));
        assert_eq!(mouse_x_to_filter_dot(11, 5), Some(3));
        assert_eq!(mouse_x_to_filter_dot(13, 5), Some(4));
    }

    #[test]
    fn mouse_dot_returns_none_between_dots() {
        assert_eq!(mouse_x_to_filter_dot(6, 5), None);
        assert_eq!(mouse_x_to_filter_dot(8, 5), None);
        assert_eq!(mouse_x_to_filter_dot(12, 5), None);
    }

    #[test]
    fn mouse_dot_returns_none_before_strip() {
        assert_eq!(mouse_x_to_filter_dot(0, 5), None);
        assert_eq!(mouse_x_to_filter_dot(4, 5), None);
    }

    #[test]
    fn mouse_dot_returns_none_past_strip() {
        // Strip occupies columns 5..15 (10 cells), so anything at 15+ is past.
        assert_eq!(mouse_x_to_filter_dot(15, 5), None);
        assert_eq!(mouse_x_to_filter_dot(100, 5), None);
    }
}
