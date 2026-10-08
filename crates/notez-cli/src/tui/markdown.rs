//! Markdown rendering for the preview pane.
//!
//! [`render_markdown`] turns a note's markdown into ratatui lines that are
//! already wrapped to the pane width. The caller draws them without
//! ratatui's own wrapping, so `lines.len()` is the exact line count to
//! scroll and clamp against.
//!
//! Parsing is `pulldown-cmark` with tables, footnotes, strikethrough and
//! task lists on. Nothing is rendered as HTML: raw HTML is shown as its
//! source text.

use std::borrow::Cow;
use std::collections::HashMap;

use pulldown_cmark::{Alignment, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::highlight::{highlight_lines, Language, MAX_HIGHLIGHT_BYTES};
use super::theme;

/// What a tab becomes in rendered text. Terminal width of a raw tab is
/// undefined, so it is expanded before measuring.
const TAB: &str = "    ";
/// The indent in front of every code block line.
const CODE_INDENT: &str = "  ";
const QUOTE_BAR: &str = "▎ ";
const BULLET: &str = "• ";
/// Between two table cells; [`TABLE_JOINT`] is its header rule crossing.
const TABLE_SEPARATOR: &str = " │ ";
const TABLE_JOINT: &str = "─┼─";
/// A table column is shrunk no narrower than this before the table clips.
const MIN_COLUMN_WIDTH: usize = 3;
/// The rule above the footnote definitions, at most this wide.
const FOOTNOTE_RULE_WIDTH: usize = 10;

/// Render `text` as markdown, wrapped to `width` columns.
///
/// Every returned line is at most `width` columns wide, except where a
/// single character is wider than the space left. A `width` of 0 is
/// treated as 1. Words wrap on whitespace and are only split when one word
/// is wider than the available width. Code block lines wrap by character
/// instead, keeping every space, and a fenced block whose tag names a
/// shipped language is syntax highlighted, unless the document is over
/// [`MAX_HIGHLIGHT_BYTES`]. A table is laid out in columns: too wide, its
/// widest columns shrink and wrap, and if even [`MIN_COLUMN_WIDTH`] per
/// column does not fit, its lines are clipped with `…`. A footnote
/// reference shows as `[n]`, numbered in order of first reference, and the
/// definitions follow the document. An empty document gives no lines.
pub fn render_markdown(text: &str, width: u16) -> Vec<Line<'static>> {
    let normalized;
    let source = if text.contains('\r') {
        normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        text
    };
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut renderer = Renderer::new(source, usize::from(width.max(1)));
    for event in Parser::new_ext(source, options) {
        renderer.event(event);
    }
    renderer.finish()
}

/// A table's styled inline text in one cell.
type Cell<'s> = Vec<(Cow<'s, str>, Style)>;

/// A table being collected, laid out once it ends.
struct Table<'s> {
    alignments: Vec<Alignment>,
    /// The header row first, then the body rows.
    rows: Vec<Vec<Cell<'s>>>,
    row: Vec<Cell<'s>>,
}

/// Footnote state. Labels are keyed lowercase, as pulldown-cmark matches
/// them case-insensitively.
#[derive(Default)]
struct Footnotes<'s> {
    /// The definition being read: its key and its events, replayed after
    /// the document.
    open: Option<(String, Vec<Event<'s>>)>,
    /// Definitions in source order; the first one for a label wins.
    definitions: Vec<(String, Vec<Event<'s>>)>,
    definition_index: HashMap<String, usize>,
    /// Keys in numbering order: `order[n - 1]` is footnote `n`.
    order: Vec<String>,
    numbers: HashMap<String, usize>,
}

impl Footnotes<'_> {
    /// The number of footnote `key`, assigning the next one on first use.
    fn number(&mut self, key: String) -> usize {
        if let Some(&number) = self.numbers.get(&key) {
            return number;
        }
        self.order.push(key.clone());
        let number = self.order.len();
        self.numbers.insert(key, number);
        number
    }
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
    /// The language of the open fenced code block, when its tag maps to one.
    code_language: Option<Language>,
    /// Whether fenced code is highlighted at all; off for a large document.
    is_highlighting: bool,
    html: Option<String>,
    table: Option<Table<'s>>,
    footnotes: Footnotes<'s>,
    needs_blank: bool,
}

impl<'s> Renderer<'s> {
    fn new(source: &str, width: usize) -> Self {
        Self {
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
            code_language: None,
            is_highlighting: source.len() as u64 <= MAX_HIGHLIGHT_BYTES,
            html: None,
            table: None,
            footnotes: Footnotes::default(),
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
        self.emit_footnotes();
        self.lines
    }

    fn event(&mut self, event: Event<'s>) {
        if let Some((_, events)) = self.footnotes.open.as_mut() {
            if !matches!(event, Event::End(TagEnd::FootnoteDefinition)) {
                events.push(event);
                return;
            }
            if let Some((key, events)) = self.footnotes.open.take() {
                if !self.footnotes.definition_index.contains_key(&key) {
                    let index = self.footnotes.definitions.len();
                    self.footnotes.definition_index.insert(key.clone(), index);
                    self.footnotes.definitions.push((key, events));
                }
            }
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text)
            | Event::InlineHtml(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => self.text(text),
            Event::FootnoteReference(label) => {
                let number = self.footnotes.number(label.to_lowercase());
                let reference = format!("[{number}]");
                if self.image_depth > 0 {
                    self.image_alt.push_str(&reference);
                } else {
                    self.inline.push((Cow::Owned(reference), theme::link_url()));
                }
            }
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

    fn start(&mut self, tag: Tag<'s>) {
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
                    self.code_language = Language::from_fence_tag(info);
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
            Tag::Table(alignments) => {
                self.begin_block();
                self.table = Some(Table {
                    alignments,
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableHead => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::FootnoteDefinition(label) => {
                self.footnotes.open = Some((label.to_lowercase(), Vec::new()));
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
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.inline);
                if let Some(table) = self.table.as_mut() {
                    table.row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if tag == TagEnd::TableHead {
                    self.styles.pop();
                }
                if let Some(table) = self.table.as_mut() {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.emit_table(table);
                }
                self.needs_blank = true;
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

    /// A code block's lines, highlighted when its fence tag names a shipped
    /// language, each wrapped by character inside the code indent. In a
    /// highlighted block uncaptured text is plain text
    /// ([`highlighted_code_base`]), so string literals stand out; a block
    /// without a known tag stays in [`theme::code`].
    fn emit_code(&mut self, code: &str) {
        let language = self.code_language.take();
        if code.is_empty() {
            return;
        }
        let body = code.strip_suffix('\n').unwrap_or(code);
        let plain: Vec<&str> = body
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect();
        let highlighted = language
            .filter(|_| self.is_highlighting)
            .map(|language| highlight_lines(code, language))
            .filter(|lines| lines.len() == plain.len());
        match highlighted {
            Some(lines) => {
                for line in lines {
                    let spans: Vec<Span<'static>> = line
                        .into_iter()
                        .map(|span| {
                            let style = highlighted_code_base().patch(span.style);
                            span.style(style)
                        })
                        .collect();
                    self.emit_code_line(&spans);
                }
            }
            None => {
                for line in plain {
                    self.emit_code_line(&[Span::styled(line.to_string(), theme::code())]);
                }
            }
        }
    }

    fn emit_code_line(&mut self, spans: &[Span<'static>]) {
        let avail = self.avail().saturating_sub(str_width(CODE_INDENT)).max(1);
        for content in wrap_chars(spans, avail) {
            let mut line = Vec::with_capacity(content.len() + 1);
            line.push(Span::raw(CODE_INDENT));
            line.extend(content);
            self.emit_line(line);
        }
    }

    /// Source text shown as is (raw HTML), one wrapped line group per
    /// source line.
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

    /// Lay a table out in columns within the available width (see
    /// [`fit_columns`]): cells wrap inside their column, a row is as tall as
    /// its tallest cell, and the header row is followed by a rule.
    fn emit_table(&mut self, table: Table<'s>) {
        let row_width = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        let columns = row_width.max(table.alignments.len());
        if columns == 0 {
            return;
        }
        let mut natural = vec![0; columns];
        for row in &table.rows {
            for (column, cell) in row.iter().enumerate() {
                natural[column] = natural[column].max(cell_width(cell));
            }
        }
        let avail = self.avail();
        let separators = str_width(TABLE_SEPARATOR) * (columns - 1);
        let widths = fit_columns(&natural, avail.saturating_sub(separators));
        for (index, row) in table.rows.iter().enumerate() {
            let mut cells: Vec<Vec<Vec<Span<'static>>>> = (0..columns)
                .map(|column| match row.get(column) {
                    Some(cell) => wrap(cell, widths[column]),
                    None => vec![Vec::new()],
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(1);
            for line in 0..height {
                let mut spans = Vec::new();
                for (column, cell) in cells.iter_mut().enumerate() {
                    if column > 0 {
                        spans.push(Span::styled(TABLE_SEPARATOR, theme::rule()));
                    }
                    let content = cell.get_mut(line).map(std::mem::take).unwrap_or_default();
                    let alignment = table.alignments.get(column).copied();
                    pad_cell(&mut spans, content, widths[column], alignment);
                }
                self.emit_table_line(spans, avail);
            }
            if index == 0 {
                let rule: Vec<String> = widths.iter().map(|&width| "─".repeat(width)).collect();
                let rule = Span::styled(rule.join(TABLE_JOINT), theme::rule());
                self.emit_table_line(vec![rule], avail);
            }
        }
    }

    /// One table line without its trailing padding, clipped with `…` when
    /// it is still wider than `avail`.
    fn emit_table_line(&mut self, mut spans: Vec<Span<'static>>, avail: usize) {
        trim_end_spans(&mut spans);
        let width: usize = spans.iter().map(|span| str_width(&span.content)).sum();
        if width > avail {
            spans = clip_spans(spans, avail);
        }
        self.emit_line(spans);
    }

    /// The footnote definitions under a short rule, in number order, each
    /// prefixed `[n] `. Definitions never referenced follow in source order.
    fn emit_footnotes(&mut self) {
        if self.footnotes.definitions.is_empty() {
            return;
        }
        self.begin_block();
        let rule = "─".repeat(FOOTNOTE_RULE_WIDTH.min(self.avail()));
        self.emit_line(vec![Span::styled(rule, theme::rule())]);
        self.needs_blank = true;
        let mut next = 0;
        let mut unreferenced = 0;
        loop {
            if next == self.footnotes.order.len() {
                let definitions = &self.footnotes.definitions;
                while unreferenced < definitions.len()
                    && self
                        .footnotes
                        .numbers
                        .contains_key(&definitions[unreferenced].0)
                {
                    unreferenced += 1;
                }
                let Some((key, _)) = definitions.get(unreferenced) else {
                    break;
                };
                let key = key.clone();
                self.footnotes.number(key);
            }
            let key = &self.footnotes.order[next];
            next += 1;
            let Some(&index) = self.footnotes.definition_index.get(key) else {
                continue;
            };
            let events = std::mem::take(&mut self.footnotes.definitions[index].1);
            self.emit_footnote(next, events);
        }
    }

    /// Replay one definition's events inside a `[n] ` marker container.
    fn emit_footnote(&mut self, number: usize, events: Vec<Event<'s>>) {
        self.begin_block();
        let marker = format!("[{number}] ");
        let width = str_width(&marker);
        self.containers.push(Container {
            first: vec![Span::styled(marker, theme::link_url())],
            rest: vec![Span::raw(" ".repeat(width))],
            is_first_used: false,
            width,
        });
        for event in events {
            self.event(event);
        }
        self.flush_inline();
        if self.containers.last().is_some_and(|c| !c.is_first_used) {
            self.emit_line(Vec::new());
        }
        self.containers.pop();
        self.needs_blank = true;
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

/// The width of a cell's widest line when it is not wrapped.
fn cell_width(cell: &[(Cow<'_, str>, Style)]) -> usize {
    wrap(cell, usize::MAX)
        .iter()
        .map(|line| line.iter().map(|span| str_width(&span.content)).sum())
        .max()
        .unwrap_or(0)
}

/// Column widths for a table whose columns want `natural` widths and may
/// use `budget` columns in total. Natural widths are kept when they fit.
/// Otherwise the widest columns shrink first: every column is capped at
/// the largest common width that fits, never below [`MIN_COLUMN_WIDTH`],
/// and the columns left over go to the capped columns from the left. When
/// even that minimum does not fit, the result is wider than `budget` and
/// the caller clips.
fn fit_columns(natural: &[usize], budget: usize) -> Vec<usize> {
    let capped = |cap: usize| -> usize { natural.iter().map(|&width| width.min(cap)).sum() };
    if natural.iter().sum::<usize>() <= budget {
        return natural.to_vec();
    }
    if capped(MIN_COLUMN_WIDTH) >= budget {
        return natural
            .iter()
            .map(|&width| width.min(MIN_COLUMN_WIDTH))
            .collect();
    }
    // `low` fits and `high` does not; the natural total is over budget.
    let mut low = MIN_COLUMN_WIDTH;
    let mut high = natural.iter().copied().max().unwrap_or(MIN_COLUMN_WIDTH);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if capped(middle) <= budget {
            low = middle;
        } else {
            high = middle;
        }
    }
    let mut spare = budget - capped(low);
    let mut widths: Vec<usize> = natural.iter().map(|&width| width.min(low)).collect();
    for (width, &wanted) in widths.iter_mut().zip(natural) {
        if spare == 0 {
            break;
        }
        if wanted > low {
            *width += 1;
            spare -= 1;
        }
    }
    widths
}

/// Append one line of a cell, padded to `width` by its alignment.
fn pad_cell(
    spans: &mut Vec<Span<'static>>,
    content: Vec<Span<'static>>,
    width: usize,
    alignment: Option<Alignment>,
) {
    let used: usize = content.iter().map(|span| str_width(&span.content)).sum();
    let gap = width.saturating_sub(used);
    let (left, right) = match alignment {
        Some(Alignment::Right) => (gap, 0),
        Some(Alignment::Center) => (gap / 2, gap - gap / 2),
        _ => (0, gap),
    };
    if left > 0 {
        spans.push(Span::raw(" ".repeat(left)));
    }
    spans.extend(content);
    if right > 0 {
        spans.push(Span::raw(" ".repeat(right)));
    }
}

/// Drop trailing whitespace from the end of a line.
fn trim_end_spans(spans: &mut Vec<Span<'static>>) {
    while let Some(last) = spans.last_mut() {
        let trimmed = last.content.trim_end();
        if trimmed.is_empty() {
            spans.pop();
        } else {
            if trimmed.len() < last.content.len() {
                let trimmed = trimmed.to_string();
                last.content = trimmed.into();
            }
            return;
        }
    }
}

/// Cut a line to `avail - 1` columns and end it with `…` in the style of
/// the span it cuts.
fn clip_spans(spans: Vec<Span<'static>>, avail: usize) -> Vec<Span<'static>> {
    let limit = avail.saturating_sub(1);
    let mut clipped = Vec::new();
    let mut width = 0;
    for span in spans {
        let span_width = str_width(&span.content);
        if width + span_width <= limit {
            width += span_width;
            clipped.push(span);
            continue;
        }
        let mut kept = String::new();
        for ch in span.content.chars() {
            let ch_width = char_width(ch);
            if width + ch_width > limit {
                break;
            }
            width += ch_width;
            kept.push(ch);
        }
        if !kept.is_empty() {
            clipped.push(Span::styled(kept, span.style));
        }
        clipped.push(Span::styled("…", span.style));
        return clipped;
    }
    clipped
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

/// The base style of uncaptured text in a highlighted code block: plain
/// text, not [`theme::code`], whose colour `syntax_string` shares.
fn highlighted_code_base() -> Style {
    Style::default().fg(theme::TEXT)
}

/// Wrap one code line to `avail` columns by character. Every character is
/// kept, spaces included, so leading indentation and inner runs survive the
/// break; a style continues on the next line. Tabs expand to [`TAB`]. A
/// character wider than `avail` gets a line of its own. Always returns at
/// least one line.
fn wrap_chars(spans: &[Span<'static>], avail: usize) -> Vec<Vec<Span<'static>>> {
    let avail = avail.max(1);
    let mut lines = Vec::new();
    let mut line: Vec<Span<'static>> = Vec::new();
    let mut width = 0;
    for span in spans {
        let style = span.style;
        let text: &str = &span.content;
        if text.bytes().all(|b| (0x20..0x7f).contains(&b)) && width + text.len() <= avail {
            width += text.len();
            push_styled(&mut line, text, style);
            continue;
        }
        let mut buf = [0u8; 4];
        for ch in text.chars() {
            let (piece, repeat): (&str, usize) = if ch == '\t' {
                (" ", TAB.len())
            } else {
                (ch.encode_utf8(&mut buf), 1)
            };
            let piece_width = char_width(piece.chars().next().unwrap_or(' '));
            for _ in 0..repeat {
                if width + piece_width > avail && width > 0 {
                    lines.push(std::mem::take(&mut line));
                    width = 0;
                }
                width += piece_width;
                push_styled(&mut line, piece, style);
            }
        }
    }
    lines.push(line);
    lines
}

/// Append `text` to the line, merging into the last span when the style
/// matches.
fn push_styled(line: &mut Vec<Span<'static>>, text: &str, style: Style) {
    if let Some(last) = line.last_mut() {
        if last.style == style {
            last.content.to_mut().push_str(text);
            return;
        }
    }
    line.push(Span::styled(text.to_string(), style));
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
        assert_eq!(span(&lines, "fn").style, theme::syntax_keyword());
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
    fn a_table_renders_as_columns_with_a_rule_under_the_header() {
        let source = "| a | b |\n|---|---|\n| 1 | **2** |\n\nafter";
        let lines = render_markdown(source, 80);
        assert_eq!(texts(&lines), ["a │ b", "──┼──", "1 │ 2", "", "after"]);
        assert!(has(span(&lines, "a").style, Modifier::BOLD));
        assert!(!has(span(&lines, "1").style, Modifier::BOLD));
        assert!(has(span(&lines, "2").style, Modifier::BOLD));
        assert_eq!(span(&lines, " │ ").style, theme::rule());
        assert_eq!(span(&lines, "──┼──").style, theme::rule());
    }

    #[test]
    fn table_columns_honour_left_centre_and_right_alignment() {
        let source = "| Left | Centre | Right |\n|:-----|:------:|------:|\n\
                      | a | b | c |\n| long cell | mid | 1234567 |\n";
        let lines = render_markdown(source, 80);
        assert_eq!(
            texts(&lines),
            [
                "Left      │ Centre │   Right",
                "──────────┼────────┼────────",
                "a         │   b    │       c",
                "long cell │  mid   │ 1234567",
            ]
        );
    }

    #[test]
    fn a_table_wider_than_the_pane_shrinks_the_widest_column_and_wraps_it() {
        let source = "| k | description |\n|---|---|\n| x | one two three four five |\n";
        let lines = render_markdown(source, 20);
        let rule = format!("──┼{}", "─".repeat(17));
        assert_eq!(
            texts(&lines),
            [
                "k │ description",
                rule.as_str(),
                "x │ one two three",
                "  │ four five",
            ]
        );
        for line in &lines {
            assert!(line.width() <= 20, "{:?}", text_of(line));
        }
    }

    #[test]
    fn a_table_that_cannot_fit_is_clipped_with_an_ellipsis() {
        let source = "| aaaaa | bbbbb | ccccc | ddddd |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |\n";
        let lines = render_markdown(source, 10);
        assert_eq!(text_of(&lines[0]), "aaa │ bbb…");
        assert_eq!(text_of(&lines[2]), "────┼────…");
        assert_eq!(lines.len(), 4);
        for line in &lines {
            assert!(line.width() <= 10, "{:?}", text_of(line));
            assert!(text_of(line).ends_with('…'), "{:?}", text_of(line));
        }
    }

    #[test]
    fn inline_code_and_emphasis_inside_a_cell_keep_their_styles() {
        let lines = render_markdown("| h |\n|---|\n| `x` *y* |", 80);
        assert_eq!(texts(&lines), ["h", "───", "x y"]);
        assert_eq!(span(&lines, "x").style, theme::code());
        assert!(has(span(&lines, "y").style, Modifier::ITALIC));
    }

    /// NZ-25 follow-up 2: the table's container prefix appears once.
    #[test]
    fn a_table_inside_a_quote_has_one_bar_per_line() {
        let lines = render_markdown("> | a | b |\n> |---|---|\n> | 1 | 2 |", 80);
        assert_eq!(texts(&lines), ["▎ a │ b", "▎ ──┼──", "▎ 1 │ 2"]);
        for line in &lines {
            assert_eq!(text_of(line).matches('▎').count(), 1);
        }
    }

    #[test]
    fn a_table_inside_a_list_item_is_indented_once() {
        let source = "- item\n\n  | a | b |\n  |---|---|\n  | 1 | 2 |\n\nafter";
        let lines = render_markdown(source, 80);
        assert_eq!(
            texts(&lines),
            ["• item", "", "  a │ b", "  ──┼──", "  1 │ 2", "", "after"]
        );
    }

    #[test]
    fn a_table_with_only_a_header_shows_the_header_and_rule() {
        let lines = render_markdown("before\n\n| a | b |\n|---|---|\n\nafter", 80);
        assert_eq!(texts(&lines), ["before", "", "a │ b", "──┼──", "", "after"]);
    }

    #[test]
    fn a_row_with_fewer_cells_than_the_header_leaves_the_rest_empty() {
        let lines = render_markdown("| a | b | c |\n|---|---|---|\n| 1 |\n| 1 | 2 | 3 |", 80);
        assert_eq!(
            texts(&lines),
            ["a │ b │ c", "──┼───┼──", "1 │   │", "1 │ 2 │ 3"]
        );
    }

    #[test]
    fn a_footnote_reference_is_numbered_and_its_definition_follows_the_document() {
        let source = "[^a]: Defined first.\n\nBody[^a] text.\n\nMore.";
        let lines = render_markdown(source, 40);
        let rule = "─".repeat(10);
        assert_eq!(
            texts(&lines),
            [
                "Body[1] text.",
                "",
                "More.",
                "",
                rule.as_str(),
                "",
                "[1] Defined first."
            ]
        );
        assert_eq!(span(&lines, "[1]").style, theme::link_url());
        assert_eq!(span(&lines, "[1] ").style, theme::link_url());
        assert_eq!(span(&lines, rule.as_str()).style, theme::rule());
    }

    #[test]
    fn footnotes_are_numbered_in_the_order_first_referenced() {
        let source = "x[^b] y[^a] z[^b]\n\n[^a]: Alpha.\n\n[^b]: Beta.";
        let lines = render_markdown(source, 40);
        let rule = "─".repeat(10);
        assert_eq!(
            texts(&lines),
            [
                "x[1] y[2] z[1]",
                "",
                rule.as_str(),
                "",
                "[1] Beta.",
                "",
                "[2] Alpha."
            ]
        );
    }

    #[test]
    fn a_wrapped_footnote_definition_aligns_with_its_text() {
        let lines = render_markdown("a[^1]\n\n[^1]: one two three", 12);
        assert_eq!(texts(&lines)[4..], ["[1] one two", "    three"]);
    }

    #[test]
    fn wide_characters_in_cells_keep_every_table_line_within_the_width() {
        let source = "| 名前 | 説明 |\n|---|---|\n| 漢字 | 漢字漢字漢字漢字漢字漢字 |\n";
        for width in [0, 1, 5, 9, 12, 20, 40] {
            let lines = render_markdown(source, width);
            let limit = usize::from(width.max(1));
            for line in &lines {
                assert!(line.width() <= limit, "{width}: {:?}", text_of(line));
            }
        }
        assert_eq!(
            texts(&render_markdown(source, 40)),
            [
                "名前 │ 説明",
                "─────┼─────────────────────────",
                "漢字 │ 漢字漢字漢字漢字漢字漢字"
            ]
        );
    }

    #[test]
    fn an_unreferenced_footnote_definition_follows_the_referenced_ones() {
        let lines = render_markdown("a[^r]\n\n[^u]: Unused.\n\n[^r]: Used.", 40);
        assert_eq!(
            texts(&lines)[2..],
            [
                "─".repeat(10),
                String::new(),
                "[1] Used.".into(),
                String::new(),
                "[2] Unused.".into()
            ]
        );
    }

    /// With GFM footnotes, pulldown-cmark emits a reference without a
    /// definition as plain text, so it shows as written.
    #[test]
    fn an_undefined_footnote_reference_shows_as_written() {
        let lines = render_markdown("see[^nope] here", 40);
        assert_eq!(texts(&lines), ["see[^nope] here"]);
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
        assert_eq!(
            span(&lines, "# not a heading").style,
            theme::syntax_comment()
        );
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

    /// NZ-25 follow-up 1: code wraps by character, so the leading spaces
    /// and inner runs survive the break. Width 8 leaves 6 columns after the
    /// two-space indent.
    #[test]
    fn code_wraps_by_character_keeping_leading_and_inner_spaces() {
        let lines = render_markdown("```\n        abcdefghijkl\n```", 8);
        assert_eq!(texts(&lines), ["        ", "    abcd", "  efghij", "  kl"]);
        let lines = render_markdown("```\nab    cd  ef\n```", 8);
        assert_eq!(texts(&lines), ["  ab    ", "  cd  ef"]);
        for line in &lines {
            assert_eq!(line.spans[0].content, CODE_INDENT);
            assert_eq!(line.spans[0].style, Style::default());
        }
    }

    #[test]
    fn wrapped_highlighted_code_carries_styles_across_the_break() {
        let lines = render_markdown("```rust\nlet s = 1; // abcdefgh\n```", 12);
        assert_eq!(
            texts(&lines),
            ["  rust", "  let s = 1;", "   // abcdef", "  gh"]
        );
        assert_eq!(span(&lines, "let").style, theme::syntax_keyword());
        let head = lines[2].spans.last().unwrap();
        assert_eq!(head.content, "// abcdef");
        assert_eq!(head.style, theme::syntax_comment());
        let tail = &lines[3].spans[1];
        assert_eq!(tail.content, "gh");
        assert_eq!(tail.style, theme::syntax_comment());
        for line in &lines {
            assert!(line.width() <= 12, "{:?}", text_of(line));
        }
    }

    #[test]
    fn fenced_rust_block_is_highlighted_and_keeps_the_line_count() {
        let source = "# T\n\n```rust\nfn main() {\n    let x = 1; // c\n}\n```\n\nafter\n";
        let lines = render_markdown(source, 80);
        assert_eq!(
            texts(&lines),
            [
                "T",
                "",
                "  rust",
                "  fn main() {",
                "      let x = 1; // c",
                "  }",
                "",
                "after"
            ]
        );
        assert_eq!(span(&lines, "fn").style, theme::syntax_keyword());
        assert_eq!(span(&lines, "let").style, theme::syntax_keyword());
        assert_eq!(span(&lines, "// c").style, theme::syntax_comment());
        assert_eq!(span(&lines, "rust").style, theme::dimmed());
        assert_eq!(lines[3].spans[0].content, CODE_INDENT);
        let plain = lines[3]
            .spans
            .iter()
            .find(|s| s.content == " ")
            .expect("uncaptured space");
        assert_eq!(plain.style, highlighted_code_base());
    }

    #[test]
    fn a_string_in_a_rust_fence_stands_out_from_the_uncaptured_text() {
        let lines = render_markdown("```rust\nlet s = \"hi\";\n```", 80);
        let line = &lines[1];
        let string = line
            .spans
            .iter()
            .position(|s| s.content == "\"hi\"")
            .expect("string span");
        assert_eq!(line.spans[string].style, theme::syntax_string());
        let before = &line.spans[string - 1];
        assert_eq!(before.content, " s = ");
        assert_ne!(before.style.fg, theme::syntax_string().fg);
        assert_ne!(before.style.fg, theme::code().fg);
        assert_eq!(before.style, highlighted_code_base());
    }

    #[test]
    fn fenced_kotlin_block_is_highlighted() {
        let lines = render_markdown("```kotlin\nval n = 1 // c\n```", 80);
        assert_eq!(texts(&lines), ["  kotlin", "  val n = 1 // c"]);
        assert_eq!(span(&lines, "val").style, theme::syntax_keyword());
        assert_eq!(span(&lines, "1").style, theme::syntax_number());
        assert_eq!(span(&lines, "// c").style, theme::syntax_comment());
    }

    #[test]
    fn a_document_over_the_limit_renders_fences_plain() {
        let mut doc = String::from("```rust\nfn main() {}\n```\n\n");
        while (doc.len() as u64) <= MAX_HIGHLIGHT_BYTES {
            doc.push_str("filler text for the size limit.\n\n");
        }
        let lines = render_markdown(&doc, 80);
        assert_eq!(texts(&lines[..2]), ["  rust", "  fn main() {}"]);
        assert_eq!(lines[1].spans[1].content, "fn main() {}");
        assert_eq!(lines[1].spans[1].style, theme::code());
    }

    /// A guard, not a benchmark: 500 KB of prose with 50 fenced Rust
    /// blocks, the shape of a long note with code in it.
    #[test]
    fn a_500_kb_note_with_50_rust_fences_renders_in_bounded_time() {
        let block = "```rust\n/// Doc.\n#[derive(Debug)]\npub struct Item { id: u32, name: String }\n\nfn build(n: u32) -> Vec<Item> {\n    (0..n).map(|id| Item { id, name: format!(\"item {id}\") }).collect()\n}\n```\n\n";
        let prose = "Some *emphasised* text with `code` and a [link](https://example.com) \
                     that goes on for a while so the wrapper has work to do.\n\n";
        let mut doc = String::new();
        for fence in 1..=50 {
            doc.push_str(block);
            while doc.len() < fence * 10 * 1024 {
                doc.push_str(prose);
            }
        }
        let started = Instant::now();
        let lines = render_markdown(&doc, 80);
        let elapsed = started.elapsed();
        eprintln!(
            "500 KB note, 50 rust fences: {} bytes, {} lines, {elapsed:?}",
            doc.len(),
            lines.len()
        );
        assert_eq!(doc.matches("```rust").count(), 50);
        assert!(lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.content == "fn" && s.style == theme::syntax_keyword()));
        assert!(elapsed.as_secs_f64() < 5.0, "rendering took {elapsed:?}");
    }

    #[test]
    fn an_unknown_fence_tag_renders_plain_code() {
        let lines = render_markdown("```yaml\nkey: \"value\" # c\n```", 80);
        assert_eq!(texts(&lines), ["  yaml", "  key: \"value\" # c"]);
        assert_eq!(lines[1].spans.len(), 2);
        assert_eq!(span(&lines, "key: \"value\" # c").style, theme::code());
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

    /// A guard against quadratic table layout: one 2 MB table of 200k rows.
    /// The 10 s bound is loose on purpose, like the guard above.
    #[test]
    fn a_two_megabyte_table_of_200k_rows_renders_in_bounded_time() {
        let mut doc = String::from("| a | b |\n|---|---|\n");
        for _ in 0..200_000 {
            doc.push_str("| x | y |\n");
        }
        let started = Instant::now();
        let lines = render_markdown(&doc, 80);
        let elapsed = started.elapsed();
        eprintln!("200k-row table: {} bytes, {elapsed:?}", doc.len());
        assert_eq!(lines.len(), 200_002);
        assert_eq!(text_of(&lines[2]), "x │ y");
        assert!(
            elapsed.as_secs_f64() < 10.0,
            "rendering the table took {elapsed:?}"
        );
    }
}
