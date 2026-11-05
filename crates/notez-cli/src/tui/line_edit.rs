//! One-line text editing for the todoz prompts: the cursor moves and
//! deletes a shell's line editor has (readline's Ctrl and Alt keys, plus
//! Cmd where the terminal reports it). Enter, Esc and anything else a
//! prompt does with a key stay with that prompt.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::text::{next_char_boundary, prev_char_boundary};

/// What one key did to the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// The text changed.
    Changed,
    /// An editing key that left the text as it was; the cursor may have
    /// moved.
    Moved,
    /// Not an editing key: the text and the cursor are untouched.
    Ignored,
}

/// Whether `c` ends a word for the word moves and deletes.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '/' | '-' | '_' | '.' | ',' | ':')
}

/// The start of the word before `at`: back over separators, then back
/// over the word.
fn word_left(buffer: &str, at: usize) -> usize {
    let before = &buffer[..at];
    let word_end = before.trim_end_matches(is_separator).len();
    before[..word_end].trim_end_matches(|c| !is_separator(c)).len()
}

/// The end of the word after `at`: over separators, then over the word.
fn word_right(buffer: &str, at: usize) -> usize {
    let after = &buffer[at..];
    let word_start = after.len() - after.trim_start_matches(is_separator).len();
    let rest = &after[word_start..];
    at + word_start + (rest.len() - rest.trim_start_matches(|c| !is_separator(c)).len())
}

/// Apply `key` to `buffer` with the cursor at byte offset `cursor`. A
/// character typed with Ctrl, Alt or Cmd is never inserted: the ones that
/// are not listed here are ignored.
pub fn edit(buffer: &mut String, cursor: &mut usize, key: KeyEvent) -> Edit {
    let mut at = (*cursor).min(buffer.len());
    if !buffer.is_char_boundary(at) {
        at = prev_char_boundary(buffer, at);
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let cmd = key.modifiers.contains(KeyModifiers::SUPER);
    // The text to delete (a byte range) or the cursor's new place.
    enum Op {
        Delete(usize, usize),
        Move(usize),
        Insert(char),
    }
    let op = match key.code {
        KeyCode::Backspace if cmd => Op::Delete(0, at),
        KeyCode::Backspace if ctrl || alt => Op::Delete(word_left(buffer, at), at),
        KeyCode::Backspace => Op::Delete(prev_char_boundary(buffer, at), at),
        KeyCode::Delete => Op::Delete(at, next_char_boundary(buffer, at)),
        KeyCode::Left if cmd => Op::Move(0),
        KeyCode::Left if ctrl || alt => Op::Move(word_left(buffer, at)),
        KeyCode::Left => Op::Move(prev_char_boundary(buffer, at)),
        KeyCode::Right if cmd => Op::Move(buffer.len()),
        KeyCode::Right if ctrl || alt => Op::Move(word_right(buffer, at)),
        KeyCode::Right => Op::Move(next_char_boundary(buffer, at)),
        KeyCode::Home => Op::Move(0),
        KeyCode::End => Op::Move(buffer.len()),
        KeyCode::Char(c) if ctrl => match c {
            'd' => Op::Delete(at, next_char_boundary(buffer, at)),
            'u' => Op::Delete(0, at),
            'k' => Op::Delete(at, buffer.len()),
            'w' | 'h' => Op::Delete(word_left(buffer, at), at),
            'a' => Op::Move(0),
            'e' => Op::Move(buffer.len()),
            _ => return Edit::Ignored,
        },
        KeyCode::Char(c) if alt => match c {
            'b' => Op::Move(word_left(buffer, at)),
            'f' => Op::Move(word_right(buffer, at)),
            _ => return Edit::Ignored,
        },
        KeyCode::Char(_) if cmd => return Edit::Ignored,
        KeyCode::Char(c) => Op::Insert(c),
        _ => return Edit::Ignored,
    };
    match op {
        Op::Delete(from, to) => {
            *cursor = from;
            if from == to {
                return Edit::Moved;
            }
            buffer.replace_range(from..to, "");
            Edit::Changed
        }
        Op::Move(to) => {
            *cursor = to;
            Edit::Moved
        }
        Op::Insert(c) => {
            buffer.insert(at, c);
            *cursor = at + c.len_utf8();
            Edit::Changed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;
    const CTRL: KeyModifiers = KeyModifiers::CONTROL;
    const ALT: KeyModifiers = KeyModifiers::ALT;
    const CMD: KeyModifiers = KeyModifiers::SUPER;

    /// Apply one key to `text` with the cursor at byte `cursor`; the text,
    /// the cursor and the outcome after it.
    fn run(text: &str, cursor: usize, code: KeyCode, mods: KeyModifiers) -> (String, usize, Edit) {
        let mut buffer = text.to_string();
        let mut at = cursor;
        let outcome = edit(&mut buffer, &mut at, KeyEvent::new(code, mods));
        (buffer, at, outcome)
    }

    fn s(text: &str, cursor: usize, outcome: Edit) -> (String, usize, Edit) {
        (text.to_string(), cursor, outcome)
    }

    // "aå日b": a at 0, å at 1..3, 日 at 3..6, b at 6..7.
    const MB: &str = "a\u{e5}\u{65e5}b";

    #[test]
    fn left_and_right_step_one_char_on_char_boundaries() {
        assert_eq!(run("abc", 0, KeyCode::Left, NONE), s("abc", 0, Edit::Moved));
        assert_eq!(run("abc", 2, KeyCode::Left, NONE), s("abc", 1, Edit::Moved));
        assert_eq!(run("abc", 3, KeyCode::Right, NONE), s("abc", 3, Edit::Moved));
        assert_eq!(run("abc", 1, KeyCode::Right, NONE), s("abc", 2, Edit::Moved));
        assert_eq!(run(MB, 6, KeyCode::Left, NONE), s(MB, 3, Edit::Moved));
        assert_eq!(run(MB, 3, KeyCode::Left, NONE), s(MB, 1, Edit::Moved));
        assert_eq!(run(MB, 1, KeyCode::Right, NONE), s(MB, 3, Edit::Moved));
        assert_eq!(run(MB, 3, KeyCode::Right, NONE), s(MB, 6, Edit::Moved));
        assert_eq!(run(MB, 7, KeyCode::Right, NONE), s(MB, 7, Edit::Moved));
    }

    #[test]
    fn backspace_delete_and_ctrl_d_remove_one_char() {
        assert_eq!(run("abc", 0, KeyCode::Backspace, NONE), s("abc", 0, Edit::Moved));
        assert_eq!(run("abc", 2, KeyCode::Backspace, NONE), s("ac", 1, Edit::Changed));
        assert_eq!(run("abc", 3, KeyCode::Backspace, NONE), s("ab", 2, Edit::Changed));
        assert_eq!(run(MB, 6, KeyCode::Backspace, NONE), s("a\u{e5}b", 3, Edit::Changed));
        assert_eq!(run(MB, 3, KeyCode::Backspace, NONE), s("a\u{65e5}b", 1, Edit::Changed));
        for (code, mods) in [(KeyCode::Delete, NONE), (KeyCode::Char('d'), CTRL)] {
            assert_eq!(run("abc", 0, code, mods), s("bc", 0, Edit::Changed), "{code:?}");
            assert_eq!(run("abc", 1, code, mods), s("ac", 1, Edit::Changed), "{code:?}");
            assert_eq!(run("abc", 3, code, mods), s("abc", 3, Edit::Moved), "{code:?}");
            assert_eq!(run(MB, 3, code, mods), s("a\u{e5}b", 3, Edit::Changed), "{code:?}");
            assert_eq!(run(MB, 1, code, mods), s("a\u{65e5}b", 1, Edit::Changed), "{code:?}");
        }
    }

    #[test]
    fn ctrl_u_and_cmd_backspace_delete_to_the_start_ctrl_k_to_the_end() {
        for (code, mods) in [(KeyCode::Char('u'), CTRL), (KeyCode::Backspace, CMD)] {
            assert_eq!(run("abc", 0, code, mods), s("abc", 0, Edit::Moved), "{code:?}");
            assert_eq!(run("abc", 2, code, mods), s("c", 0, Edit::Changed), "{code:?}");
            assert_eq!(run("abc", 3, code, mods), s("", 0, Edit::Changed), "{code:?}");
            assert_eq!(run(MB, 6, code, mods), s("b", 0, Edit::Changed), "{code:?}");
        }
        let k = (KeyCode::Char('k'), CTRL);
        assert_eq!(run("abc", 0, k.0, k.1), s("", 0, Edit::Changed));
        assert_eq!(run("abc", 1, k.0, k.1), s("a", 1, Edit::Changed));
        assert_eq!(run("abc", 3, k.0, k.1), s("abc", 3, Edit::Moved));
        assert_eq!(run(MB, 3, k.0, k.1), s("a\u{e5}", 3, Edit::Changed));
    }

    #[test]
    fn home_end_ctrl_a_ctrl_e_and_cmd_arrows_jump_to_the_ends() {
        for (code, mods) in [(KeyCode::Home, NONE), (KeyCode::Char('a'), CTRL), (KeyCode::Left, CMD)] {
            for at in [0, 3, 7] {
                assert_eq!(run(MB, at, code, mods), s(MB, 0, Edit::Moved), "{code:?} {mods:?} from {at}");
            }
        }
        for (code, mods) in [(KeyCode::End, NONE), (KeyCode::Char('e'), CTRL), (KeyCode::Right, CMD)] {
            for at in [0, 3, 7] {
                assert_eq!(run(MB, at, code, mods), s(MB, 7, Edit::Moved), "{code:?} {mods:?} from {at}");
            }
        }
    }

    const WORD_BACK: [(KeyCode, KeyModifiers); 4] = [
        (KeyCode::Char('w'), CTRL),
        (KeyCode::Char('h'), CTRL),
        (KeyCode::Backspace, ALT),
        (KeyCode::Backspace, CTRL),
    ];

    #[test]
    fn the_word_deletes_remove_back_to_the_word_start() {
        for (code, mods) in WORD_BACK {
            let why = format!("{code:?} {mods:?}");
            assert_eq!(run("call bank", 0, code, mods), s("call bank", 0, Edit::Moved), "{why}");
            assert_eq!(run("call bank", 9, code, mods), s("call ", 5, Edit::Changed), "{why}");
            assert_eq!(run("call bank", 7, code, mods), s("call nk", 5, Edit::Changed), "{why}");
            assert_eq!(run("call bank  ", 11, code, mods), s("call ", 5, Edit::Changed), "{why}: trailing spaces go too");
            assert_eq!(run("one", 3, code, mods), s("", 0, Edit::Changed), "{why}");
            assert_eq!(run("x \u{e5}\u{65e5}", 7, code, mods), s("x ", 2, Edit::Changed), "{why}");
        }
    }

    #[test]
    fn word_deletes_stop_at_every_separator() {
        for sep in [' ', '\t', '/', '-', '_', '.', ',', ':'] {
            let text = format!("ab{sep}cd");
            for (code, mods) in WORD_BACK {
                assert_eq!(run(&text, 5, code, mods), s(&format!("ab{sep}"), 3, Edit::Changed), "{sep:?} {code:?}");
            }
            assert_eq!(run(&text, 0, KeyCode::Right, ALT), s(&text, 2, Edit::Moved), "{sep:?}");
            assert_eq!(run(&text, 5, KeyCode::Left, ALT), s(&text, 3, Edit::Moved), "{sep:?}");
        }
        assert_eq!(run("a+b", 3, KeyCode::Char('w'), CTRL), s("", 0, Edit::Changed), "+ is not a separator");
    }

    #[test]
    fn the_word_moves_jump_over_one_word() {
        for (code, mods) in [(KeyCode::Left, ALT), (KeyCode::Left, CTRL), (KeyCode::Char('b'), ALT)] {
            let why = format!("{code:?} {mods:?}");
            assert_eq!(run("call the bank", 0, code, mods), s("call the bank", 0, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 13, code, mods), s("call the bank", 9, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 9, code, mods), s("call the bank", 5, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 6, code, mods), s("call the bank", 5, Edit::Moved), "{why}");
            assert_eq!(run("a \u{e5}\u{65e5}", 7, code, mods), s("a \u{e5}\u{65e5}", 2, Edit::Moved), "{why}");
        }
        for (code, mods) in [(KeyCode::Right, ALT), (KeyCode::Right, CTRL), (KeyCode::Char('f'), ALT)] {
            let why = format!("{code:?} {mods:?}");
            assert_eq!(run("call the bank", 0, code, mods), s("call the bank", 4, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 4, code, mods), s("call the bank", 8, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 6, code, mods), s("call the bank", 8, Edit::Moved), "{why}");
            assert_eq!(run("call the bank", 13, code, mods), s("call the bank", 13, Edit::Moved), "{why}");
            assert_eq!(run("\u{e5}\u{65e5} b", 0, code, mods), s("\u{e5}\u{65e5} b", 5, Edit::Moved), "{why}");
        }
    }

    #[test]
    fn a_plain_char_inserts_at_the_cursor() {
        assert_eq!(run("bc", 0, KeyCode::Char('a'), NONE), s("abc", 1, Edit::Changed));
        assert_eq!(run("ac", 1, KeyCode::Char('b'), NONE), s("abc", 2, Edit::Changed));
        assert_eq!(run("ab", 2, KeyCode::Char('C'), KeyModifiers::SHIFT), s("abC", 3, Edit::Changed));
        assert_eq!(run("ab", 1, KeyCode::Char('\u{65e5}'), NONE), s("a\u{65e5}b", 4, Edit::Changed));
        assert_eq!(run(MB, 3, KeyCode::Char('\u{e5}'), NONE), s("a\u{e5}\u{e5}\u{65e5}b", 5, Edit::Changed));
    }

    #[test]
    fn a_ctrl_alt_or_cmd_char_never_inserts() {
        for mods in [CTRL, ALT, CMD, CTRL | ALT, CTRL | KeyModifiers::SHIFT] {
            for c in ['x', 'c', 'v', 'z', 'q', '1', '\u{e5}', 'b', 'f', 'u', 'a'] {
                let (text, _, outcome) = run("ab", 1, KeyCode::Char(c), mods);
                assert!(!text.contains(c) || "ab".contains(c), "{mods:?}+{c:?} inserted: {text:?}");
                assert!(text.len() <= 2, "{mods:?}+{c:?} inserted: {text:?}");
                let listed = if mods.contains(CTRL) { "dukwhae".contains(c) } else { mods == ALT && "bf".contains(c) };
                if !listed {
                    assert_eq!(outcome, Edit::Ignored, "{mods:?}+{c:?}");
                    assert_eq!(text, "ab", "{mods:?}+{c:?}");
                }
            }
        }
    }

    #[test]
    fn other_keys_are_ignored_and_a_stale_cursor_is_clamped() {
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Up, KeyCode::Down, KeyCode::Tab, KeyCode::F(1)] {
            assert_eq!(run("ab", 1, code, NONE), s("ab", 1, Edit::Ignored), "{code:?}");
        }
        assert_eq!(run("ab", 9, KeyCode::Char('c'), NONE), s("abc", 3, Edit::Changed), "past the end");
        assert_eq!(run(MB, 2, KeyCode::Char('x'), NONE), s("ax\u{e5}\u{65e5}b", 2, Edit::Changed), "inside a char");
    }
}
