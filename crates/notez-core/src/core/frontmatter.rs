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
//! Beyond Pinz, so the reader accepts what [`set`] writes: a block may follow
//! a leading UTF-8 BOM, and when the first line ends in `\r\n` the markers
//! must too (`---\r\n` ... `\r\n---\r\n`).
//!
//! When a key appears more than once, the last occurrence is the value, as
//! in Pinz, and [`set`] replaces that one.

fn pair(line: &str) -> Option<(&str, &str)> {
    line.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
}

/// Split `text` into its frontmatter pairs, in file order, and the body.
/// Without a terminated block the pairs are empty and the body is `text`.
pub fn split(text: &str) -> (Vec<(String, String)>, &str) {
    let Some(block) = block(text, line_ending(text)) else {
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

const BOM: char = '\u{feff}';

/// The byte layout of a block, in offsets into `text`.
struct Block<'a> {
    /// The lines between the two markers, without the final line ending.
    inner: &'a str,
    /// Where `inner` starts.
    inner_start: usize,
    /// Where the block starts: 0, or just after a BOM.
    start: usize,
    /// Just past the closing `---` and the line ending that follows it.
    end: usize,
    /// The body as [`split`] reads it: the text after the line ending that
    /// follows the closing marker, or empty without one (the Pinz rule).
    body: &'a str,
}

/// The line ending of `text`: `\r\n` if its first line ends that way.
fn line_ending(text: &str) -> &'static str {
    match text.find('\n') {
        Some(i) if text[..i].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// The block at the top of `text`, read and written alike: it starts at byte
/// 0 or just after a UTF-8 BOM, and its markers end in `eol`. The closing
/// marker needs a line ending before it, so `---\n---\n` is unterminated.
fn block<'a>(text: &'a str, eol: &str) -> Option<Block<'a>> {
    let start = if text.starts_with(BOM) {
        BOM.len_utf8()
    } else {
        0
    };
    let after_open = text[start..].strip_prefix("---")?.strip_prefix(eol)?;
    let inner_start = text.len() - after_open.len();
    let close = format!("{eol}---");
    let len = after_open.find(&close)?;
    let inner = &after_open[..len];
    let after_close = inner_start + len + close.len();
    let (end, body) = match text[after_close..].strip_prefix(eol) {
        Some(body) => (after_close + eol.len(), body),
        None => (after_close, ""),
    };
    Some(Block {
        inner,
        inner_start,
        start,
        end,
        body,
    })
}

/// The block `set` may write into: [`block`] holding at least one
/// `key: value` line, so a note opening with a horizontal rule has none.
fn write_block<'a>(text: &'a str, eol: &str) -> Option<Block<'a>> {
    block(text, eol).filter(|b| b.inner.split('\n').any(|raw| pair(raw).is_some()))
}

/// Return `text` with `key` set to `value`, every other byte unchanged.
///
/// An existing `key` line (the last, if repeated) is replaced by
/// `key: value`; otherwise the line is appended at the end of the block.
/// Without a block holding at least one `key: value` line, a new
/// `---\nkey: value\n---\n` block is put in front of the text, so a note
/// opening with a horizontal rule keeps its prose. A leading UTF-8 BOM stays
/// first, and a text whose first line ends in `\r\n` gets `\r\n` in the
/// lines written.
///
/// An empty (or all-whitespace) `value` removes every `key` line instead, and
/// the whole block with its closing line when no `key: value` line is left;
/// with no such key the text is returned unchanged.
///
/// `key` and `value` must be single-line and `key` must not contain `:`;
/// this function does not validate them.
pub fn set(text: &str, key: &str, value: &str) -> String {
    let eol = line_ending(text);
    let block = write_block(text, eol);
    if value.trim().is_empty() {
        return match block {
            Some(block) => remove(text, &block, key, eol),
            None => text.to_string(),
        };
    }
    let line = format!("{key}: {value}");
    let Some(block) = block else {
        let (bom, rest) = text.split_at(if text.starts_with(BOM) {
            BOM.len_utf8()
        } else {
            0
        });
        return format!("{bom}---{eol}{line}{eol}---{eol}{rest}");
    };
    let inner_end = block.inner_start + block.inner.len();

    let mut target: Option<(usize, usize)> = None;
    let mut offset = block.inner_start;
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

    let mut out = String::with_capacity(text.len() + line.len() + eol.len());
    match target {
        Some((start, end)) => {
            out.push_str(&text[..start]);
            out.push_str(&line);
            out.push_str(&text[end..]);
        }
        None => {
            out.push_str(&text[..inner_end]);
            out.push_str(eol);
            out.push_str(&line);
            out.push_str(&text[inner_end..]);
        }
    }
    out
}

/// `text` without any `key` line in `block`, and without the block once it
/// holds no `key: value` line.
fn remove(text: &str, block: &Block, key: &str, eol: &str) -> String {
    let is_key = |raw: &str| pair(raw).is_some_and(|(k, _)| k == key);
    if !block.inner.split(eol).any(is_key) {
        return text.to_string();
    }
    let kept: Vec<&str> = block.inner.split(eol).filter(|raw| !is_key(raw)).collect();
    if !kept.iter().any(|raw| pair(raw).is_some()) {
        return format!("{}{}", &text[..block.start], &text[block.end..]);
    }
    let inner_end = block.inner_start + block.inner.len();
    format!(
        "{}{}{}",
        &text[..block.inner_start],
        kept.join(eol),
        &text[inner_end..]
    )
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
    }

    #[test]
    fn split_reads_crlf_and_bom_blocks() {
        let crlf = "---\r\ndue: 2026-10-20\r\nx:  1 \r\n---\r\nbody\r\n";
        assert_eq!(
            split(crlf),
            (pairs(&[("due", "2026-10-20"), ("x", "1")]), "body\r\n")
        );
        let bom = "\u{feff}---\ndue: 2026-10-20\n---\nbody";
        assert_eq!(split(bom), (pairs(&[("due", "2026-10-20")]), "body"));
        let both = "\u{feff}---\r\ndue: 2026-10-20\r\n---\r\nbody";
        assert_eq!(split(both), (pairs(&[("due", "2026-10-20")]), "body"));
        // The BOM is the one exception to byte zero.
        let late = "\u{feff}\n---\ndue: 2026-10-20\n---\nbody";
        assert_eq!(split(late), (Vec::new(), late));
        // A block with no `key: value` line still reads as an empty block.
        assert_eq!(split("---\r\njunk\r\n---\r\nbody"), (Vec::new(), "body"));
    }

    #[test]
    fn split_reads_what_set_writes_on_crlf_and_bom_notes() {
        for text in [
            "# Title\r\n\r\nbody\r\n",
            "---\r\nx: 1\r\ndue: a\r\n---\r\nbody\r\n",
            "\u{feff}# Title\n",
            "\u{feff}---\r\ndue: a\r\n---\r\nbody",
        ] {
            let out = set(text, "due", "2026-10-20");
            let (fields, _) = split(&out);
            assert_eq!(get(&fields, "due"), Some("2026-10-20"), "on {text:?}");
        }
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

    #[test]
    fn set_on_a_horizontal_rule_note_inserts_a_block_above_the_text() {
        let text = "---\n\nSome text\n---\nMore";
        let out = set(text, "due", "2026-10-20");
        assert_eq!(out, format!("---\ndue: 2026-10-20\n---\n{text}"));
        assert_eq!(split(&out), (pairs(&[("due", "2026-10-20")]), text));
    }

    #[test]
    fn set_keeps_crlf_in_a_new_block() {
        let text = "# Title\r\n\r\nbody\r\n";
        assert_eq!(
            set(text, "due", "2026-10-20"),
            "---\r\ndue: 2026-10-20\r\n---\r\n# Title\r\n\r\nbody\r\n"
        );
    }

    #[test]
    fn set_keeps_crlf_in_an_existing_block() {
        let text = "---\r\nx: 1\r\ndue: a\r\n---\r\nbody\r\n";
        assert_eq!(
            set(text, "due", "b"),
            "---\r\nx: 1\r\ndue: b\r\n---\r\nbody\r\n"
        );
        assert_eq!(
            set(text, "event", "c"),
            "---\r\nx: 1\r\ndue: a\r\nevent: c\r\n---\r\nbody\r\n"
        );
    }

    #[test]
    fn set_keeps_the_bom_first() {
        let text = "\u{feff}# Title\n";
        assert_eq!(
            set(text, "due", "2026-10-20"),
            "\u{feff}---\ndue: 2026-10-20\n---\n# Title\n"
        );
        let with_block = "\u{feff}---\ndue: a\n---\nbody";
        assert_eq!(
            set(with_block, "due", "b"),
            "\u{feff}---\ndue: b\n---\nbody"
        );
    }

    #[test]
    fn set_replaces_a_key_keeping_order_and_other_keys() {
        let text = "---\na: 1\ndue: x\nb: 2\n---\nbody";
        assert_eq!(set(text, "due", "y"), "---\na: 1\ndue: y\nb: 2\n---\nbody");
    }

    #[test]
    fn set_appends_a_new_key_at_the_end_of_the_block() {
        let text = "---\na: 1\nb: 2\n---\nbody";
        assert_eq!(set(text, "due", "y"), "---\na: 1\nb: 2\ndue: y\n---\nbody");
    }

    #[test]
    fn set_with_an_empty_value_removes_the_key() {
        let text = "---\na: 1\ndue: x\nb: 2\n---\nbody";
        assert_eq!(set(text, "due", ""), "---\na: 1\nb: 2\n---\nbody");
        // The last line of the block, and every duplicate, so `get` sees none.
        assert_eq!(
            set("---\ndue: x\na: 1\ndue: y\n---\nbody", "due", ""),
            "---\na: 1\n---\nbody"
        );
        let crlf = "---\r\na: 1\r\ndue: x\r\n---\r\nbody";
        assert_eq!(set(crlf, "due", ""), "---\r\na: 1\r\n---\r\nbody");
    }

    #[test]
    fn set_removing_the_last_key_removes_the_block() {
        assert_eq!(set("---\ndue: x\n---\nbody\n", "due", ""), "body\n");
        assert_eq!(
            set("\u{feff}---\r\ndue: x\r\n---\r\nbody", "due", ""),
            "\u{feff}body"
        );
        assert_eq!(set("---\ndue: x\n---", "due", ""), "");
    }

    #[test]
    fn set_removing_an_absent_key_changes_nothing() {
        let text = "---\na: 1\n---\nbody";
        assert_eq!(set(text, "due", ""), text);
        assert_eq!(set("plain\n", "due", ""), "plain\n");
    }

    #[test]
    fn set_leaves_the_body_byte_identical() {
        let body = "# T\r\n\n---\nnot: a block\n\u{feff}\r\n  trailing  ";
        let texts = [
            body.to_string(),
            format!("---\na: 1\ndue: x\n---\n{body}"),
            format!("---\r\na: 1\r\n---\r\n{body}"),
        ];
        for text in &texts {
            let after = &text[text.len() - body.len()..];
            for (key, value) in [("due", "y"), ("new", "z"), ("due", ""), ("a", "")] {
                let out = set(text, key, value);
                assert!(
                    out.ends_with(after),
                    "{key}={value:?} on {text:?} gave {out:?}"
                );
            }
        }
    }
}
