//! Markdown rendering for the preview pane.
//!
//! [`render_markdown`] turns a note's markdown into ratatui lines that are
//! already wrapped to the pane width. The caller draws them without
//! ratatui's own wrapping, so `lines.len()` is the exact line count to
//! scroll and clamp against.
//!
//! Parsing is `pulldown-cmark` with tables, strikethrough and task lists
//! on. Nothing is rendered as HTML: raw HTML and tables are shown as their
//! source text.

use std::borrow::Cow;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::theme;

/// What a tab becomes in rendered text. Terminal width of a raw tab is
/// undefined, so it is expanded before measuring.
const TAB: &str = "    ";
/// The indent in front of every code block line.
const CODE_INDENT: &str = "  ";
const QUOTE_BAR: &str = "▎ ";
const BULLET: &str = "• ";

/// Render `text` as markdown, wrapped to `width` columns.
///
/// Every returned line is at most `width` columns wide, except where a
/// single character is wider than the space left. A `width` of 0 is
/// treated as 1. Words wrap on whitespace and are only split when one word
/// is wider than the available width. An empty document gives no lines.
pub fn render_markdown(text: &str, width: u16) -> Vec<Line<'static>> {
    let normalized;
    let source = if text.contains('\r') {
        normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        text
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut renderer = Renderer::new(source, usize::from(width.max(1)));
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        renderer.event(event, range);
    }
    renderer.finish()
}

/// A block that prefixes every line inside it: a block quote's bar or a
/// list item's marker. `first` goes on the item's first line, `rest` on
/// every later one; both have the same width.
struct Container {
    first: Vec<Span<'static>>,
    rest: Vec<Span<'static>>,
    is_first_used: bool,
    width: usize,
}

struct Renderer<'s> {
    source: &'s str,
    width: usize,
    lines: Vec<Line<'static>>,
    containers: Vec<Container>,
    /// The next number of each open list, `None` for a bullet list.
    lists: Vec<Option<u64>>,
    item_depth: usize,
    /// Styled inline text of the current paragraph, heading or list item.
    inline: Vec<(Cow<'s, str>, Style)>,
    styles: Vec<Style>,
    /// Open links: destination and the `inline` index where the text starts.
    links: Vec<(String, usize)>,
    image_depth: usize,
    image_alt: String,
    code: Option<String>,
    html: Option<String>,
    in_table: bool,
    needs_blank: bool,
}

impl<'s> Renderer<'s> {
    fn new(source: &'s str, width: usize) -> Self {
        Self {
            source,
            width,
            lines: Vec::new(),
            containers: Vec::new(),
            lists: Vec::new(),
            item_depth: 0,
            inline: Vec::new(),
            styles: Vec::new(),
            links: Vec::new(),
            image_depth: 0,
            image_alt: String::new(),
            code: None,
            html: None,
            in_table: false,
            needs_blank: false,
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_inline();
        if let Some(code) = self.code.take() {
            self.emit_code(&code);
        }
        if let Some(html) = self.html.take() {
            self.emit_source(&html);
        }
        self.lines
    }

    fn event(&mut self, event: Event<'s>, range: Range<usize>) {
        if self.in_table {
            if matches!(event, Event::End(TagEnd::Table)) {
                self.in_table = false;
                self.needs_blank = true;
            }
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(tag) => self.end(tag),
            Event::Text(text)
            | Event::InlineHtml(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text)
            | Event::FootnoteReference(text) => self.text(text),
            Event::Html(text) => match self.html.as_mut() {
                Some(html) => html.push_str(&text),
                None => self.text(text),
            },
            Event::Code(text) => {
                if self.image_depth > 0 {
                    self.image_alt.push_str(&text);
                } else {
                    let style = self.style().patch(theme::code());
                    self.inline.push((into_cow(text), style));
                }
            }
            Event::SoftBreak => self.text(CowStr::Borrowed(" ")),
            Event::HardBreak => {
                if self.image_depth > 0 {
                    self.image_alt.push(' ');
                } else {
                    self.inline.push((Cow::Borrowed("\n"), self.style()));
                }
            }
            Event::Rule => {
                self.begin_block();
                let rule = "─".repeat(self.avail());
                self.emit_line(vec![Span::styled(rule, theme::rule())]);
                self.needs_blank = true;
            }
            Event::TaskListMarker(is_done) => {
                let marker = if is_done { "[x] " } else { "[ ] " };
                self.inline.push((Cow::Borrowed(marker), self.style()));
            }
        }
    }

    fn start(&mut self, tag: Tag<'s>, range: Range<usize>) {
        match tag {
            Tag::Paragraph => self.begin_block(),
            Tag::Heading { level, .. } => {
                self.begin_block();
                self.push_style(theme::heading(level as u8));
            }
            Tag::BlockQuote(_) => {
                self.begin_block();
                let bar = vec![Span::styled(QUOTE_BAR, theme::quote())];
                self.containers.push(Container {
                    first: bar.clone(),
                    rest: bar,
                    is_first_used: false,
                    width: str_width(QUOTE_BAR),
                });
                self.push_style(theme::quote());
            }
            Tag::CodeBlock(kind) => {
                self.begin_block();
                if let CodeBlockKind::Fenced(info) = kind {
                    let info = info.trim();
                    if !info.is_empty() {
                        self.emit_wrapped(&[(Cow::Borrowed(info), theme::dimmed())], CODE_INDENT);
                    }
                }
                self.code = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.begin_block();
                self.html = Some(String::new());
            }
            Tag::List(start) => {
                self.begin_block();
                self.lists.push(start);
            }
            Tag::Item => {
                self.begin_block();
                let marker = match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}. ");
                        *number += 1;
                        marker
                    }
                    _ => BULLET.to_string(),
                };
                let width = str_width(&marker);
                self.containers.push(Container {
                    first: vec![Span::raw(marker)],
                    rest: vec![Span::raw(" ".repeat(width))],
                    is_first_used: false,
                    width,
                });
                self.item_depth += 1;
            }
            Tag::Table(_) => {
                self.begin_block();
                let source = self.source;
                self.emit_source(&source[range]);
                self.in_table = true;
            }
            Tag::Emphasis => self.push_style(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => {
                self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT))
            }
            Tag::Link { dest_url, .. } => {
                if self.image_depth == 0 {
                    self.links.push((dest_url.to_string(), self.inline.len()));
                }
            }
            Tag::Image { .. } => self.image_depth += 1,
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_inline();
                self.needs_blank = true;
            }
            TagEnd::Heading(_) => {
                self.flush_inline();
                self.styles.pop();
                self.needs_blank = true;
            }
            TagEnd::BlockQuote(_) => {
                self.flush_inline();
                self.containers.pop();
                self.styles.pop();
                self.needs_blank = true;
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.emit_code(&code);
                }
                self.needs_blank = true;
            }
            TagEnd::HtmlBlock => {
                if let Some(html) = self.html.take() {
                    self.emit_source(&html);
                }
                self.needs_blank = true;
            }
            TagEnd::List(_) => {
                self.flush_inline();
                self.lists.pop();
                if self.item_depth == 0 {
                    self.needs_blank = true;
                }
            }
            TagEnd::Item => {
                self.flush_inline();
                self.containers.pop();
                self.item_depth = self.item_depth.saturating_sub(1);
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                if self.image_depth == 0 {
                    self.end_link();
                }
            }
            TagEnd::Image => {
                self.image_depth = self.image_depth.saturating_sub(1);
                if self.image_depth == 0 {
                    let alt = std::mem::take(&mut self.image_alt);
                    self.inline
                        .push((Cow::Owned(format!("[image: {alt}]")), theme::link_url()));
                }
            }
            _ => {}
        }
    }

    /// Append ` (url)` after the link text, or the url alone when the text
    /// is empty. An autolink, whose text is its url, is not repeated.
    fn end_link(&mut self) {
        let Some((url, start)) = self.links.pop() else {
            return;
        };
        if url.is_empty() {
            return;
        }
        let label: String = self.inline[start.min(self.inline.len())..]
            .iter()
            .map(|(text, _)| text.as_ref())
            .collect();
        if label.is_empty() {
            self.inline.push((Cow::Owned(url), theme::link_url()));
        } else if label != url {
            self.inline
                .push((Cow::Owned(format!(" ({url})")), theme::link_url()));
        }
    }

    fn text(&mut self, text: CowStr<'s>) {
        if let Some(code) = self.code.as_mut() {
            code.push_str(&text);
        } else if let Some(html) = self.html.as_mut() {
            html.push_str(&text);
        } else if self.image_depth > 0 {
            self.image_alt.push_str(&text);
        } else {
            self.inline.push((into_cow(text), self.style()));
        }
    }

    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        let combined = self.style().patch(style);
        self.styles.push(combined);
    }

    /// Close the pending inline text and put the separating blank line in
    /// front of the block that starts now.
    fn begin_block(&mut self) {
        self.flush_inline();
        if self.needs_blank {
            self.needs_blank = false;
            self.emit_blank();
        }
    }

    fn prefix_width(&self) -> usize {
        self.containers.iter().map(|c| c.width).sum()
    }

    /// Columns left for content after the container prefixes, at least 1.
    fn avail(&self) -> usize {
        self.width.saturating_sub(self.prefix_width()).max(1)
    }

    fn flush_inline(&mut self) {
        if self.inline.is_empty() {
            return;
        }
        let segments = std::mem::take(&mut self.inline);
        self.emit_wrapped(&segments, "");
    }

    fn emit_code(&mut self, code: &str) {
        if code.is_empty() {
            return;
        }
        let body = code.strip_suffix('\n').unwrap_or(code);
        for line in body.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            self.emit_wrapped(&[(Cow::Borrowed(line), theme::code())], CODE_INDENT);
        }
    }

    /// Source text shown as is (tables, raw HTML), one wrapped line group
    /// per source line.
    fn emit_source(&mut self, text: &str) {
        let body = text.trim_end_matches('\n');
        if body.is_empty() {
            return;
        }
        for line in body.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            self.emit_wrapped(&[(Cow::Borrowed(line), Style::default())], "");
        }
    }

    fn emit_wrapped(&mut self, segments: &[(Cow<'_, str>, Style)], indent: &'static str) {
        let indent_width = str_width(indent);
        let avail = self.avail().saturating_sub(indent_width).max(1);
        for content in wrap(segments, avail) {
            let mut spans = Vec::with_capacity(content.len() + 1);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.extend(content);
            self.emit_line(spans);
        }
    }

    fn emit_line(&mut self, content: Vec<Span<'static>>) {
        let mut spans = Vec::with_capacity(self.containers.len() + content.len());
        for container in &mut self.containers {
            if container.is_first_used {
                spans.extend(container.rest.iter().cloned());
            } else {
                spans.extend(container.first.iter().cloned());
                container.is_first_used = true;
            }
        }
        spans.extend(content);
        self.lines.push(Line::from(spans));
    }

    /// A blank separator line, keeping quote bars but no trailing spaces.
    /// Never the first line of the output.
    fn emit_blank(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let mut spans: Vec<Span<'static>> = self
            .containers
            .iter()
            .flat_map(|c| c.rest.iter().cloned())
            .collect();
        while let Some(last) = spans.last_mut() {
            let trimmed = last.content.trim_end();
            if trimmed.is_empty() {
                spans.pop();
            } else {
                let trimmed = trimmed.to_string();
                last.content = trimmed.into();
                break;
            }
        }
        self.lines.push(Line::from(spans));
    }
}

/// Display width of `text`, as ratatui measures it. Printable ASCII, the
/// common case, skips the Unicode tables: it is one column per byte.
fn into_cow(text: CowStr<'_>) -> Cow<'_, str> {
    match text {
        CowStr::Borrowed(text) => Cow::Borrowed(text),
        other => Cow::Owned(other.to_string()),
    }
}

fn str_width(text: &str) -> usize {
    if text.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return text.len();
    }
    Span::raw(text).width()
}

fn char_width(ch: char) -> usize {
    let mut buf = [0u8; 4];
    str_width(ch.encode_utf8(&mut buf))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    Word,
    Space,
    Break,
}

/// The run kind of the character starting at byte `index`, its length, and
/// whether it is printable ASCII (one column). ASCII is classified by byte,
/// the hot path on large notes.
fn classify(bytes: &[u8], text: &str, index: usize) -> (Run, usize, bool) {
    let byte = bytes[index];
    if byte < 0x80 {
        return match byte {
            b'\n' => (Run::Break, 1, false),
            b' ' => (Run::Space, 1, true),
            b'\t' | b'\r' | 0x0b | 0x0c => (Run::Space, 1, false),
            0x21..=0x7e => (Run::Word, 1, true),
            _ => (Run::Word, 1, false),
        };
    }
    let ch = text[index..].chars().next().unwrap_or(' ');
    let kind = if ch.is_whitespace() {
        Run::Space
    } else {
        Run::Word
    };
    (kind, ch.len_utf8(), false)
}

/// Word-wrap styled segments to `avail` columns. Always returns at least
/// one line. Whitespace at a soft wrap point is dropped; leading
/// whitespace at the start of the text or after a hard break is kept.
fn wrap(segments: &[(Cow<'_, str>, Style)], avail: usize) -> Vec<Vec<Span<'static>>> {
    let mut wrapper = Wrapper::new(avail);
    for (text, style) in segments {
        let text: &str = text;
        let bytes = text.as_bytes();
        let mut start = 0;
        let mut index = 0;
        let mut current: Option<Run> = None;
        let mut is_plain = true;
        while index < bytes.len() {
            let (kind, len, is_plain_char) = classify(bytes, text, index);
            if current != Some(kind) || kind == Run::Break {
                if let Some(previous) = current {
                    wrapper.run(previous, &text[start..index], *style, is_plain);
                }
                start = index;
                current = Some(kind);
                is_plain = true;
            }
            is_plain &= is_plain_char;
            index += len;
        }
        if let Some(kind) = current {
            wrapper.run(kind, &text[start..], *style, is_plain);
        }
    }
    wrapper.finish()
}

/// One piece of a word or of pending whitespace: text, style, width.
type Piece<'a> = (Cow<'a, str>, Style, usize);

struct Wrapper<'a> {
    avail: usize,
    lines: Vec<Vec<Span<'static>>>,
    line: Vec<Span<'static>>,
    line_width: usize,
    pending: Vec<Piece<'a>>,
    pending_width: usize,
    word: Vec<Piece<'a>>,
    word_width: usize,
    is_soft_start: bool,
}

impl<'a> Wrapper<'a> {
    /// Feed one run. `is_plain` means printable ASCII, so its width is its
    /// length and the Unicode width tables can be skipped.
    fn run(&mut self, kind: Run, text: &'a str, style: Style, is_plain: bool) {
        match kind {
            Run::Break => {
                self.flush_word();
                self.hard_break();
            }
            Run::Space => {
                self.flush_word();
                self.space(text, style, is_plain);
            }
            Run::Word => {
                let width = if is_plain {
                    text.len()
                } else {
                    str_width(text)
                };
                self.word_width += width;
                self.word.push((Cow::Borrowed(text), style, width));
            }
        }
    }

    fn new(avail: usize) -> Self {
        Self {
            avail: avail.max(1),
            lines: Vec::new(),
            line: Vec::new(),
            line_width: 0,
            pending: Vec::new(),
            pending_width: 0,
            word: Vec::new(),
            word_width: 0,
            is_soft_start: false,
        }
    }

    fn space(&mut self, run: &'a str, style: Style, is_plain: bool) {
        if self.is_soft_start && self.line_width == 0 {
            return;
        }
        let (run, width) = if is_plain {
            (Cow::Borrowed(run), run.len())
        } else {
            let run: Cow<'a, str> = Cow::Owned(run.replace('\t', TAB));
            let width = str_width(&run);
            (run, width)
        };
        self.pending_width += width;
        self.pending.push((run, style, width));
    }

    fn flush_word(&mut self) {
        if self.word.is_empty() {
            return;
        }
        let mut word = std::mem::take(&mut self.word);
        let word_width = std::mem::take(&mut self.word_width);
        if self.line_width + self.pending_width + word_width <= self.avail {
            self.commit_pending();
        } else if self.line_width > 0 {
            self.soft_break();
        } else {
            self.clear_pending();
        }
        if self.line_width + word_width <= self.avail {
            for index in 0..word.len() {
                let (text, style, width) = &word[index];
                self.push_piece(text, *style, *width);
            }
        } else {
            let mut buf = [0u8; 4];
            for index in 0..word.len() {
                let (text, style, _) = &word[index];
                for ch in text.chars() {
                    let width = char_width(ch);
                    if self.line_width + width > self.avail && self.line_width > 0 {
                        self.soft_break();
                    }
                    self.push_piece(ch.encode_utf8(&mut buf), *style, width);
                }
            }
        }
        word.clear();
        self.word = word;
    }

    fn commit_pending(&mut self) {
        let mut pending = std::mem::take(&mut self.pending);
        self.pending_width = 0;
        for index in 0..pending.len() {
            let (text, style, width) = &pending[index];
            self.push_piece(text, *style, *width);
        }
        pending.clear();
        self.pending = pending;
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
        self.pending_width = 0;
    }

    fn push_piece(&mut self, text: &str, style: Style, width: usize) {
        self.line_width += width;
        self.is_soft_start = false;
        if let Some(last) = self.line.last_mut() {
            if last.style == style {
                last.content.to_mut().push_str(text);
                return;
            }
        }
        self.line.push(Span::styled(text.to_string(), style));
    }

    fn soft_break(&mut self) {
        self.clear_pending();
        self.lines.push(std::mem::take(&mut self.line));
        self.line_width = 0;
        self.is_soft_start = true;
    }

    fn hard_break(&mut self) {
        self.clear_pending();
        self.lines.push(std::mem::take(&mut self.line));
        self.line_width = 0;
        self.is_soft_start = false;
    }

    fn finish(mut self) -> Vec<Vec<Span<'static>>> {
        self.flush_word();
        self.lines.push(self.line);
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    /// The first span whose text is exactly `needle`.
    fn span<'a>(lines: &'a [Line<'static>], needle: &str) -> &'a Span<'static> {
        lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.content == needle)
            .unwrap_or_else(|| panic!("no span {needle:?} in {:?}", texts(lines)))
    }

    fn has(style: Style, modifier: Modifier) -> bool {
        style.add_modifier.contains(modifier)
    }

    #[test]
    fn headings_one_and_two_have_their_own_style_and_deeper_levels_are_bold() {
        let lines = render_markdown("# One\n\n## Two\n\n### Three\n\n###### Six", 40);
        assert_eq!(texts(&lines), ["One", "", "Two", "", "Three", "", "Six"]);
        assert_eq!(span(&lines, "One").style, theme::heading(1));
        assert_eq!(span(&lines, "Two").style, theme::heading(2));
        assert_ne!(theme::heading(1), theme::heading(2));
        for deep in ["Three", "Six"] {
            let style = span(&lines, deep).style;
            assert!(has(style, Modifier::BOLD), "{deep} not bold");
            assert_eq!(style.fg, None, "{deep} coloured");
        }
        assert!(has(theme::heading(1), Modifier::BOLD));
        assert!(has(theme::heading(2), Modifier::BOLD));
    }

    #[test]
    fn emphasis_is_italic_and_strong_is_bold() {
        let lines = render_markdown("a *it* b **bo** c ***both***", 80);
        assert_eq!(texts(&lines), ["a it b bo c both"]);
        assert!(has(span(&lines, "it").style, Modifier::ITALIC));
        assert!(!has(span(&lines, "it").style, Modifier::BOLD));
        assert!(has(span(&lines, "bo").style, Modifier::BOLD));
        let both = span(&lines, "both").style;
        assert!(has(both, Modifier::BOLD) && has(both, Modifier::ITALIC));
        assert_eq!(span(&lines, "a ").style, Style::default());
    }

    #[test]
    fn inline_code_uses_the_code_style() {
        let lines = render_markdown("run `cargo test` now", 80);
        assert_eq!(texts(&lines), ["run cargo test now"]);
        assert_eq!(span(&lines, "cargo test").style, theme::code());
    }

    #[test]
    fn fenced_code_keeps_the_language_as_a_dim_first_line_and_indents_the_code() {
        let lines = render_markdown("```rust\nfn main() {\n    go();\n}\n```\n", 80);
        assert_eq!(
            texts(&lines),
            ["  rust", "  fn main() {", "      go();", "  }"]
        );
        assert_eq!(span(&lines, "rust").style, theme::dimmed());
        assert_eq!(span(&lines, "fn main() {").style, theme::code());
    }

    #[test]
    fn fenced_code_without_a_language_has_no_language_line() {
        let lines = render_markdown("```\nplain\n\nafter blank\n```", 80);
        assert_eq!(texts(&lines), ["  plain", "  ", "  after blank"]);
        assert_eq!(span(&lines, "plain").style, theme::code());
    }

    #[test]
    fn indented_code_renders_like_a_fence_without_a_language() {
        let lines = render_markdown("text\n\n    let x = 1;\n    let y = 2;\n", 80);
        assert_eq!(texts(&lines), ["text", "", "  let x = 1;", "  let y = 2;"]);
        assert_eq!(span(&lines, "let x = 1;").style, theme::code());
    }

    #[test]
    fn bullet_lists_use_bullets() {
        let lines = render_markdown("- one\n- two\n* three", 80);
        assert_eq!(texts(&lines), ["• one", "• two", "", "• three"]);
    }

    #[test]
    fn numbered_lists_count_from_their_start() {
        let lines = render_markdown("3. three\n4. four\n9. five", 80);
        assert_eq!(texts(&lines), ["3. three", "4. four", "5. five"]);
    }

    #[test]
    fn nested_lists_indent_two_columns_per_level() {
        let lines = render_markdown("- a\n  - b\n    - c\n- d", 80);
        assert_eq!(texts(&lines), ["• a", "  • b", "    • c", "• d"]);
    }

    #[test]
    fn list_item_continuation_lines_align_with_the_text() {
        let lines = render_markdown("- alpha beta gamma", 10);
        assert_eq!(texts(&lines), ["• alpha", "  beta", "  gamma"]);
    }

    #[test]
    fn block_quotes_have_a_bar_in_the_quote_style() {
        let lines = render_markdown("> quoted\n>\n> more", 80);
        assert_eq!(texts(&lines), ["▎ quoted", "▎", "▎ more"]);
        assert_eq!(span(&lines, "▎ ").style, theme::quote());
        assert_eq!(span(&lines, "quoted").style, theme::quote());
    }

    #[test]
    fn nested_quote_gets_two_bars() {
        let lines = render_markdown("> > deep", 80);
        assert_eq!(texts(&lines), ["▎ ▎ deep"]);
    }

    #[test]
    fn horizontal_rule_spans_the_width() {
        let lines = render_markdown("above\n\n---\n\nbelow", 12);
        let rule = "─".repeat(12);
        assert_eq!(texts(&lines), ["above", "", rule.as_str(), "", "below"]);
        assert_eq!(lines[2].spans[0].style, theme::rule());
    }

    #[test]
    fn links_show_their_text_then_the_dimmed_url() {
        let lines = render_markdown("see [the docs](https://x.io/d) here", 80);
        assert_eq!(texts(&lines), ["see the docs (https://x.io/d) here"]);
        assert_eq!(span(&lines, " (https://x.io/d)").style, theme::link_url());
    }

    #[test]
    fn autolinks_show_the_url_once() {
        let lines = render_markdown("<https://x.io>", 80);
        assert_eq!(texts(&lines), ["https://x.io"]);
    }

    #[test]
    fn images_show_an_alt_placeholder() {
        let lines = render_markdown("![a cat](cat.png)", 80);
        assert_eq!(texts(&lines), ["[image: a cat]"]);
        assert_eq!(span(&lines, "[image: a cat]").style, theme::link_url());
    }

    #[test]
    fn tables_pass_through_as_their_source_lines() {
        let source = "| a | b |\n|---|---|\n| 1 | **2** |\n\nafter";
        let lines = render_markdown(source, 80);
        assert_eq!(
            texts(&lines),
            ["| a | b |", "|---|---|", "| 1 | **2** |", "", "after"]
        );
    }

    #[test]
    fn raw_html_blocks_pass_through_as_text() {
        let lines = render_markdown("<div>\n<b>hi</b>\n</div>\n\nafter", 80);
        assert_eq!(texts(&lines), ["<div>", "<b>hi</b>", "</div>", "", "after"]);
    }

    #[test]
    fn inline_html_passes_through_as_text() {
        let lines = render_markdown("a <kbd>K</kbd> b", 80);
        assert_eq!(texts(&lines), ["a <kbd>K</kbd> b"]);
    }

    #[test]
    fn hard_break_starts_a_new_line_and_soft_break_is_a_space() {
        let lines = render_markdown("one\ntwo  \nthree\\\nfour", 80);
        assert_eq!(texts(&lines), ["one two", "three", "four"]);
    }

    #[test]
    fn strikethrough_is_crossed_out() {
        let lines = render_markdown("keep ~~gone~~", 80);
        assert!(has(span(&lines, "gone").style, Modifier::CROSSED_OUT));
        assert!(!has(span(&lines, "keep ").style, Modifier::CROSSED_OUT));
    }

    #[test]
    fn task_list_items_show_checkboxes() {
        let lines = render_markdown("- [ ] todo\n- [x] done", 80);
        assert_eq!(texts(&lines), ["• [ ] todo", "• [x] done"]);
    }

    #[test]
    fn blocks_are_separated_by_one_blank_line() {
        let lines = render_markdown("# H\npara\n- item\n\n> q", 80);
        assert_eq!(texts(&lines), ["H", "", "para", "", "• item", "", "▎ q"]);
    }

    #[test]
    fn paragraphs_wrap_at_width_20_without_breaking_words() {
        let source = "The quick brown fox jumps over the lazy dog and keeps running \
                      until the preview pane runs out of columns entirely.";
        let lines = render_markdown(source, 20);
        let got = texts(&lines);
        assert_eq!(
            got,
            [
                "The quick brown fox",
                "jumps over the lazy",
                "dog and keeps",
                "running until the",
                "preview pane runs",
                "out of columns",
                "entirely.",
            ]
        );
        for line in &lines {
            assert!(line.width() <= 20, "{:?} is wider than 20", text_of(line));
        }
        let words: Vec<&str> = source.split_whitespace().collect();
        let joined = got.join(" ");
        assert_eq!(joined.split_whitespace().collect::<Vec<_>>(), words);
    }

    #[test]
    fn styled_text_inside_a_word_does_not_split_it() {
        let lines = render_markdown("aaaa **bb**cc", 6);
        assert_eq!(texts(&lines), ["aaaa", "bbcc"]);
    }

    #[test]
    fn width_zero_and_one_do_not_panic() {
        let source =
            "# Head\n\nsome words\n\n- a\n  - b\n\n> q\n\n```\ncode\n```\n\n---\n\n| t |\n|---|";
        for width in [0, 1] {
            let lines = render_markdown(source, width);
            assert!(!lines.is_empty());
            assert_eq!(texts(&lines)[0], "H");
        }
        assert_eq!(texts(&render_markdown("ab", 0)), ["a", "b"]);
    }

    #[test]
    fn crlf_input_renders_like_lf() {
        let lf = "# T\n\none\ntwo\n\n```\nx\ny\n```\n\n| a |\n|---|\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(render_markdown(&crlf, 40), render_markdown(lf, 40));
        assert!(texts(&render_markdown(&crlf, 40))
            .iter()
            .all(|l| !l.contains('\r')));
        assert_eq!(
            render_markdown(&lf.replace('\n', "\r"), 40),
            render_markdown(lf, 40)
        );
    }

    #[test]
    fn an_unterminated_fence_renders_the_rest_as_code() {
        let lines = render_markdown("before\n\n```sh\necho hi\n# not a heading\n", 80);
        assert_eq!(
            texts(&lines),
            ["before", "", "  sh", "  echo hi", "  # not a heading"]
        );
        assert_eq!(span(&lines, "# not a heading").style, theme::code());
    }

    #[test]
    fn an_empty_document_renders_no_lines() {
        assert!(render_markdown("", 40).is_empty());
        assert!(render_markdown("\n\n  \n", 40).is_empty());
    }

    #[test]
    fn a_long_single_token_is_split_at_the_width() {
        let token = "x".repeat(25);
        let lines = render_markdown(&format!("a {token} b"), 10);
        assert_eq!(texts(&lines), ["a", "xxxxxxxxxx", "xxxxxxxxxx", "xxxxx b"]);
        let wide = "漢".repeat(7);
        for line in render_markdown(&wide, 5) {
            assert!(line.width() <= 5);
        }
    }

    #[test]
    fn code_lines_wider_than_the_pane_wrap_inside_the_indent() {
        let lines = render_markdown("```\nabcdefghij\n```", 8);
        assert_eq!(texts(&lines), ["  abcdef", "  ghij"]);
    }

    /// A guard against quadratic behaviour, not a benchmark. A debug build
    /// renders this in about 0.45 s locally; the 5 s bound is loose on
    /// purpose so shared CI runners never flake, while a quadratic
    /// regression on 2 MB would still blow far past it.
    #[test]
    fn a_two_megabyte_document_renders_in_bounded_time() {
        let chunk =
            "## Section\n\nSome *emphasised* text with `code` and a [link](https://example.com) \
                     that goes on for a while so the wrapper has work to do.\n\n\
                     - item one\n- item two\n  - nested\n\n> a quote\n\n```rust\nfn f() {}\n```\n\n\
                     | a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\n";
        let mut doc = String::new();
        while doc.len() < 2 * 1024 * 1024 {
            doc.push_str(chunk);
        }
        let started = Instant::now();
        let lines = render_markdown(&doc, 80);
        let elapsed = started.elapsed();
        assert!(lines.len() > 100_000);
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "rendering 2 MB took {elapsed:?}"
        );
    }
}
