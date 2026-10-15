//! The frontmatter block at the top of a note, in the Pinz format.
//!
//! A block starts at byte 0 with `---\n` and ends at the next `\n---`. Inside
//! it, every line holding a `:` is a `key: value` pair, split at the first
//! colon, both sides trimmed; other lines are ignored. No block, or a block
//! that is never closed, means no fields and the whole text is the body.
//! These rules match `pinz-core`'s `split_frontmatter` edge for edge (an
//! empty block `---\n---\n` is unterminated there too, because the closing
//! marker needs a newline before it), so Pinz can later use this module.
//!
//! When a key appears more than once, the last occurrence is the value, as
//! in Pinz, and [`set`] replaces that one.

/// The byte layout of a block: where its lines live and where the body starts.
struct Block<'a> {
    /// The lines between the two markers, without the final newline.
    inner: &'a str,
    /// The text after the closing marker and the newline that ends it.
    body: &'a str,
}

const OPEN: &str = "---\n";
const CLOSE: &str = "\n---";

fn block(text: &str) -> Option<Block<'_>> {
    let after_open = text.strip_prefix(OPEN)?;
    let end = after_open.find(CLOSE)?;
    let (inner, tail) = after_open.split_at(end);
    // Pinz rule: the body starts after one newline following the closing
    // marker; anything else on that line, or no newline at all, means no body.
    let body = tail[CLOSE.len()..].strip_prefix('\n').unwrap_or("");
    Some(Block { inner, body })
}

fn pair(line: &str) -> Option<(&str, &str)> {
    line.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
}

/// Split `text` into its frontmatter pairs, in file order, and the body.
/// Without a terminated block the pairs are empty and the body is `text`.
pub fn split(text: &str) -> (Vec<(String, String)>, &str) {
    let Some(block) = block(text) else {
        return (Vec::new(), text);
    };
    let fields = block
        .inner
        .lines()
        .filter_map(pair)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    (fields, block.body)
}

/// The value of `key` in `fields`, the last occurrence winning.
pub fn get<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Return `text` with `key` set to `value`, every other byte unchanged.
///
/// An existing `key` line (the last, if repeated) is replaced by
/// `key: value`; otherwise the line is appended at the end of the block.
/// Without a terminated block, a new `---\nkey: value\n---\n` block is put
/// in front of the text. `key` and `value` must be single-line and `key`
/// must not contain `:`; this function does not validate them.
///
/// No caller writes notes with this yet (NZ-48 is read-only); the write
/// path lands with NZ-47c.
pub fn set(text: &str, key: &str, value: &str) -> String {
    let line = format!("{key}: {value}");
    let Some(block) = block(text) else {
        return format!("{OPEN}{line}\n---\n{text}");
    };
    let inner_start = OPEN.len();
    let inner_end = inner_start + block.inner.len();

    let mut target: Option<(usize, usize)> = None;
    let mut offset = inner_start;
    for raw in block.inner.split('\n') {
        let start = offset;
        let end = start + raw.len();
        offset = end + 1;
        // `lines()` in `split` drops a trailing `\r`; match on the same view.
        let view = raw.strip_suffix('\r').unwrap_or(raw);
        if pair(view).is_some_and(|(k, _)| k == key) {
            target = Some((start, start + view.len()));
        }
    }

    let mut out = String::with_capacity(text.len() + line.len() + 1);
    match target {
        Some((start, end)) => {
            out.push_str(&text[..start]);
            out.push_str(&line);
            out.push_str(&text[end..]);
        }
        None => {
            out.push_str(&text[..inner_end]);
            if !block.inner.is_empty() {
                out.push('\n');
            }
            out.push_str(&line);
            out.push_str(&text[inner_end..]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIN: &str = "---\nx: -359\ny: -749\nz: 131\ncolor: peach\n---\n# Title\n\nbody";

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn split_reads_a_pinz_pin() {
        let (fields, body) = split(PIN);
        assert_eq!(
            fields,
            pairs(&[
                ("x", "-359"),
                ("y", "-749"),
                ("z", "131"),
                ("color", "peach")
            ])
        );
        assert_eq!(body, "# Title\n\nbody");
    }

    #[test]
    fn split_without_block_returns_text_unchanged() {
        let text = "# Title\n\nDate: 2026-10-15\n";
        assert_eq!(split(text), (Vec::new(), text));
    }

    #[test]
    fn split_unterminated_block_is_all_body() {
        let text = "---\ndue: 2026-10-20\n# Title\n";
        assert_eq!(split(text), (Vec::new(), text));
    }

    #[test]
    fn split_requires_block_at_byte_zero() {
        let text = "\n---\ndue: 2026-10-20\n---\nbody";
        assert_eq!(split(text), (Vec::new(), text));
        let crlf = "---\r\ndue: 2026-10-20\r\n---\r\nbody";
        assert_eq!(split(crlf), (Vec::new(), crlf));
    }

    #[test]
    fn split_matches_pinz_edge_cases() {
        // An empty block has no newline before its closing marker.
        assert_eq!(split("---\n---\nbody"), (Vec::new(), "---\n---\nbody"));
        // Closing marker at end of file: no body.
        assert_eq!(split("---\na: 1\n---"), (pairs(&[("a", "1")]), ""));
        // Text glued to the closing marker: Pinz drops it, so do we.
        assert_eq!(split("---\na: 1\n----\nbody").1, "");
        // Lines without a colon are skipped; values split at the first colon.
        let (fields, _) = split("---\njunk\n  time :  14:00  \n---\n");
        assert_eq!(fields, pairs(&[("time", "14:00")]));
    }

    #[test]
    fn get_takes_the_last_duplicate() {
        let (fields, _) = split("---\ndue: a\ndue: b\n---\n");
        assert_eq!(get(&fields, "due"), Some("b"));
        assert_eq!(get(&fields, "event"), None);
    }

    #[test]
    fn set_adds_a_key_to_a_pin_keeping_every_other_byte() {
        let out = set(PIN, "due", "2026-10-20");
        assert_eq!(
            out,
            "---\nx: -359\ny: -749\nz: 131\ncolor: peach\ndue: 2026-10-20\n---\n# Title\n\nbody"
        );
        let (fields, body) = split(&out);
        assert_eq!(&fields[..4], &split(PIN).0[..]);
        assert_eq!(body, "# Title\n\nbody");
    }

    #[test]
    fn set_replaces_an_existing_key_in_place() {
        let text = "---\ncreated: 2026-10-15\ndue:   next friday \ncolor: peach\n---\nbody\n";
        assert_eq!(
            set(text, "due", "2026-10-20"),
            "---\ncreated: 2026-10-15\ndue: 2026-10-20\ncolor: peach\n---\nbody\n"
        );
    }

    #[test]
    fn set_replaces_the_last_duplicate() {
        assert_eq!(
            set("---\ndue: a\nx: 1\ndue: b\n---\n", "due", "c"),
            "---\ndue: a\nx: 1\ndue: c\n---\n"
        );
    }

    #[test]
    fn set_keeps_a_carriage_return_on_the_replaced_line() {
        assert_eq!(
            set("---\ndue: a\r\nx: 1\n---\n", "due", "b"),
            "---\ndue: b\r\nx: 1\n---\n"
        );
    }

    #[test]
    fn set_creates_a_block_when_none_exists() {
        let text = "# Title\n\nDate: 2026-10-15\n";
        let out = set(text, "due", "2026-10-20");
        assert_eq!(
            out,
            "---\ndue: 2026-10-20\n---\n# Title\n\nDate: 2026-10-15\n"
        );
        assert_eq!(split(&out), (pairs(&[("due", "2026-10-20")]), text));
    }

    #[test]
    fn set_on_an_unterminated_block_prepends_a_new_one() {
        let text = "---\nbroken\n";
        let out = set(text, "due", "2026-10-20");
        assert_eq!(split(&out), (pairs(&[("due", "2026-10-20")]), text));
    }

    #[test]
    fn set_on_empty_text() {
        assert_eq!(set("", "due", "2026-10-20"), "---\ndue: 2026-10-20\n---\n");
    }
}
