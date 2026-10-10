//! The preview pane: scrolling, its view modes, the suffix indicator,
//! the cache and syntax highlighting.

use super::*;

// --- Preview scrolling ---

/// Lines one mouse wheel notch scrolls the preview.
pub(super) const WHEEL_STEP: i32 = 3;

/// The preview scroll offset after moving `delta` lines from `current`,
/// clamped to `0..=max`, where `max` is the last offset that still fills the
/// pane. Every preview scroll input (`J`/`K`, Shift+Down/Up, PgDn/PgUp, the
/// wheel) goes through here.
pub(super) fn scrolled(current: u16, delta: i32, max: u16) -> u16 {
    let target = i64::from(current) + i64::from(delta);
    target.clamp(0, i64::from(max)) as u16
}

/// Lines PgDn/PgUp scroll a preview pane `height` lines tall: a page minus
/// one, so the line at the edge stays in view; at least one.
pub(super) fn preview_page(height: u16) -> i32 {
    i32::from(height.saturating_sub(1).max(1))
}

// --- Preview rendering ---

/// How the preview shows a file with a language. `Rendered` renders a
/// markdown note and highlights a code file; `Raw` shows a markdown note's
/// source (still highlighted as markdown) and a code file plain. Session
/// state of one browser run, one value for every file: remembered across
/// selections, never saved. Files without a language ignore it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum PreviewMode {
    #[default]
    Rendered,
    Raw,
}

/// The `p` hint for the selected file: the session's mode and whether the
/// file is a markdown note, which picks the hint's wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreviewToggle {
    mode: PreviewMode,
    markdown: bool,
}

impl PreviewToggle {
    /// The toggle on a markdown note.
    #[cfg(test)]
    pub(super) fn markdown(mode: PreviewMode) -> Self {
        PreviewToggle {
            mode,
            markdown: true,
        }
    }

    /// The toggle for the file at `path`; `None` when its extension maps to
    /// no language, since `p` then changes nothing.
    pub(super) fn for_file(path: &Path, mode: PreviewMode) -> Option<Self> {
        file_language(path)?;
        Some(PreviewToggle {
            mode,
            markdown: is_markdown(path),
        })
    }

    /// The footer word of the toggle hint: the view `p` switches to,
    /// `raw` / `rendered` for markdown, `plain` / `highlighted` for code.
    pub(super) fn desc(self) -> &'static str {
        match (self.markdown, self.mode) {
            (true, PreviewMode::Rendered) => "raw",
            (true, PreviewMode::Raw) => "rendered",
            (false, PreviewMode::Rendered) => "plain",
            (false, PreviewMode::Raw) => "highlighted",
        }
    }
}

/// Whether `path` is a markdown note: a `.md` extension, any case.
fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// The language a file's extension maps to (`.md` is markdown).
fn file_language(path: &Path) -> Option<Language> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(Language::from_extension)
}

/// Whether a file of `len` bytes is too large to highlight.
fn is_too_large_to_highlight(len: u64) -> bool {
    len > highlight::MAX_HIGHLIGHT_BYTES
}

/// A file row's suffix indicator on the preview's bottom border.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileSuffix {
    /// The lowercase suffix with its dot (`.rs`, `.md`, `.ts`), or `file`
    /// when the name has no extension.
    text: String,
    /// The highlighting language the extension maps to.
    language: Option<Language>,
    /// Why a file with a language is shown plain anyway: `highlighter
    /// unavailable` when its grammar failed to load, `not highlighted,
    /// large` when it is over the highlighting limit.
    note: Option<&'static str>,
}

/// The suffix indicator for a row; folder and section rows have none.
pub(super) fn file_suffix(path: &Path, is_dir: bool) -> Option<FileSuffix> {
    if is_dir {
        return None;
    }
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());
    let text = match ext.filter(|ext| !ext.is_empty()) {
        Some(ext) => format!(".{ext}"),
        None => "file".to_string(),
    };
    let language = file_language(path);
    let note = language.and_then(|language| {
        if !highlight::language_available(language) {
            return Some("highlighter unavailable");
        }
        let len = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
        is_too_large_to_highlight(len).then_some("not highlighted, large")
    });
    Some(FileSuffix {
        text,
        language,
        note,
    })
}

/// The style of the suffix text. Bold in the language's colour while the
/// renderer or highlighter is on for the file (`Rendered` and a language,
/// no note); plain bold in [`theme::NEUTRAL_SUFFIX`] while it is off, for a
/// suffix without a language and for a large or unavailable file. `file`
/// is a word, not a suffix, so it is not bold.
fn suffix_style(suffix: &FileSuffix, mode: PreviewMode) -> Style {
    let neutral = Style::default().fg(theme::NEUTRAL_SUFFIX);
    if !suffix.text.starts_with('.') {
        return neutral;
    }
    let on = mode == PreviewMode::Rendered && suffix.note.is_none();
    match suffix.language {
        Some(language) if on => neutral
            .fg(theme::language_color(language))
            .add_modifier(Modifier::BOLD),
        _ => neutral.add_modifier(Modifier::BOLD),
    }
}

/// The suffix indicator as drawn, padded by a space on each side: the
/// suffix in [`suffix_style`], then its note in parentheses, dim.
pub(super) fn suffix_spans(suffix: &FileSuffix, mode: PreviewMode) -> Vec<Span<'static>> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(suffix.text.clone(), suffix_style(suffix, mode)),
    ];
    if let Some(note) = suffix.note {
        spans.push(Span::styled(format!(" ({note})"), theme::dimmed()));
    }
    spans.push(Span::raw(" "));
    spans
}

/// The path of `path` relative to its section `root`, as the preview's
/// bottom border shows it (`ideas/plan.md`); the bare file name for a
/// note at the root, or for a path outside the root.
pub(super) fn section_relative_path(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.to_string_lossy().into_owned(),
        _ => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// `path` with the home directory shortened to `~`.
pub(super) fn tilde_path(path: &Path) -> String {
    notez_core::util::tilde::contract(path)
}

/// The display width of `text` in terminal columns.
pub(super) fn text_width(text: &str) -> usize {
    Span::raw(text).width()
}

/// `text` cut from the LEFT to at most `max` columns, starting with `…`
/// when cut, so its end (a file name) stays visible. `None` when it does
/// not fit and there is no room for `…` and at least one column of text.
fn clip_left(text: &str, max: usize) -> Option<String> {
    if text_width(text) <= max {
        return Some(text.to_string());
    }
    if max < 2 {
        return None;
    }
    let mut tail = Vec::new();
    let mut width = 1;
    for ch in text.chars().rev() {
        let ch_width = text_width(ch.encode_utf8(&mut [0u8; 4]));
        if width + ch_width > max {
            break;
        }
        width += ch_width;
        tail.push(ch);
    }
    Some(std::iter::once('…').chain(tail.into_iter().rev()).collect())
}

/// The preview pane's bottom border titles for a file row, between the
/// corners of a border `width` columns wide: `suffix` left-aligned, then
/// the section-relative `path` right-aligned and dim, padded by a space.
/// At least one space separates them; when the pane is narrow the path is
/// cut from the left first, and dropped when even that does not fit.
pub(super) fn preview_bottom_titles(
    suffix: Vec<Span<'static>>,
    path: &str,
    width: usize,
) -> (Line<'static>, Option<Line<'static>>) {
    let left = Line::from(suffix).left_aligned();
    let room = width.saturating_sub(left.width() + 1).saturating_sub(2);
    let right = clip_left(path, room)
        .map(|path| Line::from(Span::styled(format!(" {path} "), theme::dimmed())).right_aligned());
    (left, right)
}

/// The preview title after the pane number: `spans` (the dots and the file
/// name, ending in a space), then the section `root`, dim, cut from the
/// left to fit the `width` columns the title has; left out when not even
/// `…` and one column fit.
pub(super) fn with_section_root(
    mut spans: Vec<Span<'static>>,
    root: &str,
    width: usize,
) -> Vec<Span<'static>> {
    let used: usize = spans.iter().map(Span::width).sum();
    let room = width.saturating_sub(used).saturating_sub(1);
    if let Some(root) = clip_left(root, room) {
        spans.push(Span::styled(format!("{root} "), theme::dimmed()));
    }
    spans
}

/// What a cached file preview was built from. Any difference rebuilds it:
/// another file, a resize (`width`), the toggle (`rendered`), the file
/// being written (`modified`, `len`), or the language the whole file is
/// highlighted as (`language`, `None` for rendered markdown, plain text and
/// files over the limit).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewKey {
    path: PathBuf,
    width: u16,
    rendered: bool,
    modified: Option<std::time::SystemTime>,
    len: u64,
    language: Option<Language>,
}

impl PreviewKey {
    /// The key of `path` as it is on disk now; `None` when it cannot be read.
    fn of(path: &Path, width: u16, rendered: bool) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let len = meta.len();
        let language = file_language(path).filter(|_| !rendered && !is_too_large_to_highlight(len));
        Some(PreviewKey {
            path: path.to_path_buf(),
            width,
            rendered,
            modified: meta.modified().ok(),
            len,
            language,
        })
    }
}

/// The last file preview built, so a draw that changes none of its inputs
/// neither reads nor renders the file again.
#[derive(Default)]
struct PreviewCache {
    key: Option<PreviewKey>,
    lines: Vec<Line<'static>>,
}

impl PreviewCache {
    /// The lines for `key`, from `build` when `key` differs from the cached
    /// one or is `None` (a file that cannot be read is never cached).
    fn get(
        &mut self,
        key: Option<PreviewKey>,
        build: impl FnOnce() -> Vec<Line<'static>>,
    ) -> &[Line<'static>] {
        if key.is_none() || self.key != key {
            self.lines = build();
            self.key = key;
        }
        &self.lines
    }
}

/// The preview's rendering mode and its cached file lines.
#[derive(Default)]
pub(super) struct Preview {
    pub(super) mode: PreviewMode,
    cache: PreviewCache,
}

impl Preview {
    /// `p`: switch between rendered and raw (markdown) or highlighted and
    /// plain (code), only while a file with a language is selected
    /// (`path`), since nothing else shows a difference.
    pub(super) fn toggle(&mut self, path: Option<&Path>) {
        if path.is_some_and(|path| file_language(path).is_some()) {
            self.mode = match self.mode {
                PreviewMode::Rendered => PreviewMode::Raw,
                PreviewMode::Raw => PreviewMode::Rendered,
            };
        }
    }

    /// The preview lines of the file at `path` for a pane `width` columns
    /// wide inside its borders and padding: rendered markdown, already
    /// wrapped to `width`, or the raw lines unwrapped, highlighted when the
    /// extension maps to a language, the file is within the limit, and a
    /// code file is not toggled to plain.
    pub(super) fn file_lines(&mut self, path: &Path, width: u16) -> &[Line<'static>] {
        let rendered = self.mode == PreviewMode::Rendered && is_markdown(path);
        let plain = self.mode == PreviewMode::Raw && !is_markdown(path);
        let key = PreviewKey::of(path, width, rendered).map(|key| {
            if plain {
                PreviewKey {
                    language: None,
                    ..key
                }
            } else {
                key
            }
        });
        let language = key.as_ref().and_then(|key| key.language);
        self.cache
            .get(key, || file_preview_lines(path, rendered, width, language))
    }
}

/// Reads `path` and builds its preview lines; see [`Preview::file_lines`].
fn file_preview_lines(
    path: &Path,
    rendered: bool,
    width: u16,
    language: Option<Language>,
) -> Vec<Line<'static>> {
    match std::fs::read_to_string(path) {
        Ok(content) if rendered => markdown::render_markdown(&content, width),
        Ok(content) => match language {
            Some(language) => highlighted_preview_lines(&content, language),
            None => content.lines().map(raw_preview_line).collect(),
        },
        Err(_) => vec![Line::from(Span::styled(
            "  unable to read file",
            Style::default().fg(theme::OVERLAY),
        ))],
    }
}

/// The raw preview of `content` highlighted as `language`. A line no
/// capture touched looks exactly as [`raw_preview_line`] draws it; in a
/// touched line the captured text has its capture style and the rest takes
/// the line's raw style (so a markdown heading's text stays a heading).
fn highlighted_preview_lines(content: &str, language: Language) -> Vec<Line<'static>> {
    let highlighted = highlight::highlight_lines(content, language);
    if highlighted.len() != content.lines().count() {
        return content.lines().map(raw_preview_line).collect();
    }
    content
        .lines()
        .zip(highlighted)
        .map(|(text, spans)| {
            if spans.iter().all(|span| span.style == Style::default()) {
                return raw_preview_line(text);
            }
            let base = raw_line_style(text);
            let spans: Vec<Span<'static>> = spans
                .into_iter()
                .map(|span| {
                    if span.style == Style::default() {
                        span.style(base)
                    } else {
                        span
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// One line of the raw preview, coloured by its leading markdown syntax.
fn raw_preview_line(line: &str) -> Line<'static> {
    Line::from(Span::styled(line.to_string(), raw_line_style(line)))
}

/// The raw preview style of a line, by its leading markdown syntax.
fn raw_line_style(line: &str) -> Style {
    if line.starts_with('#') {
        Style::default()
            .fg(theme::MAUVE)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with("- [") {
        Style::default().fg(theme::SAPPHIRE)
    } else if line.starts_with("- ") || line.starts_with("* ") {
        Style::default().fg(theme::TEXT)
    } else {
        Style::default().fg(theme::SUBTEXT)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    #[test]
    fn scrolled_clamps_at_the_end() {
        assert_eq!(scrolled(10, 1, 10), 10);
        assert_eq!(scrolled(9, 3, 10), 10);
        assert_eq!(scrolled(0, 5, 0), 0);
        assert_eq!(scrolled(u16::MAX, i32::MAX, u16::MAX), u16::MAX);
        // A stale offset past a shrunken end comes back to it.
        assert_eq!(scrolled(15, 0, 10), 10);
    }

    #[test]
    fn scrolled_moves_one_line_for_j_and_k() {
        assert_eq!(scrolled(4, 1, 10), 5);
        assert_eq!(scrolled(4, -1, 10), 3);
    }

    #[test]
    fn wheel_step_is_three_lines() {
        assert_eq!(scrolled(4, WHEEL_STEP, 100), 7);
        assert_eq!(scrolled(4, -WHEEL_STEP, 100), 1);
    }

    #[test]
    fn page_step_is_the_pane_height_minus_one_line() {
        assert_eq!(preview_page(20), 19);
        assert_eq!(scrolled(0, preview_page(20), 100), 19);
        assert_eq!(scrolled(19, -preview_page(20), 100), 0);
        assert_eq!(scrolled(90, preview_page(20), 100), 100);
        // A pane one line tall (or none) still pages by a line.
        assert_eq!(preview_page(1), 1);
        assert_eq!(preview_page(0), 1);
    }

    #[test]
    fn preview_scroll_keys_are_listed_and_j_k_shows_in_the_footer_first_to_drop() {
        let jk = TREE_KEYS.iter().find(|k| k.key == "J/K").unwrap();
        assert_eq!(jk.help, "scroll preview down / up (also Shift+Down/Up)");
        assert_eq!(jk.group, Group::Navigate);
        assert_eq!(jk.modes, BROWSE);
        let page = TREE_KEYS.iter().find(|k| k.key == "PgDn/PgUp").unwrap();
        assert_eq!(page.help, "scroll preview a page");
        assert_eq!(page.group, Group::Navigate);
        assert_eq!(page.modes, BROWSE);
        let Slot::Priority(jk_priority) = jk.slot else {
            panic!("J/K must show in the footer, not {:?}", jk.slot);
        };
        // NZ-4: the pane keys drop before J/K; every other key after it.
        for other in TREE_KEYS.iter().filter(|k| k.key != "J/K") {
            if let Slot::Priority(p) = other.slot {
                if PANE_KEYS.contains(&other.key) && other.modes == BROWSE {
                    assert!(p > jk_priority, "{} drops after J/K", other.key);
                } else {
                    assert!(p < jk_priority, "{} drops before J/K", other.key);
                }
            }
        }
        let first_width = |key: &str| {
            (0..300)
                .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key))
                .unwrap()
        };
        assert!(
            first_width("J/K") > first_width("space"),
            "J/K drops before space"
        );
        assert!(
            first_width("J/K") > first_width("S"),
            "J/K drops first of the list keys"
        );
        for pane_key in PANE_KEYS {
            assert!(
                first_width(pane_key) > first_width("J/K"),
                "{pane_key} drops after J/K"
            );
        }
    }

    // --- Preview rendering (NZ-25) ---

    #[test]
    fn preview_mode_defaults_to_rendered_and_p_toggles_it_on_a_markdown_note() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        let mut preview = Preview::default();
        assert_eq!(preview.mode, PreviewMode::Rendered);
        preview.toggle(Some(&note));
        assert_eq!(preview.mode, PreviewMode::Raw);
        preview.toggle(Some(&note));
        assert_eq!(preview.mode, PreviewMode::Rendered);
    }

    #[test]
    fn p_does_nothing_on_a_file_without_a_language_a_folder_or_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut preview = Preview::default();
        preview.toggle(Some(&dir.path().join("notes.txt")));
        preview.toggle(Some(&dir.path().join("Makefile")));
        preview.toggle(None);
        assert_eq!(preview.mode, PreviewMode::Rendered);
        preview.toggle(Some(&dir.path().join("notes.MD")));
        assert_eq!(preview.mode, PreviewMode::Raw, ".MD is markdown too");
    }

    #[test]
    fn p_on_a_code_file_toggles_highlighted_and_plain_and_the_cache_key_follows() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();
        let mut preview = Preview::default();
        let highlighted = preview.file_lines(&file, 40).to_vec();
        assert_eq!(style_of(&highlighted[0], "fn"), theme::syntax_keyword());
        assert_eq!(
            preview.cache.key.as_ref().unwrap().language,
            Some(Language::Rust)
        );

        preview.toggle(Some(&file));
        assert_eq!(preview.mode, PreviewMode::Raw);
        assert_eq!(
            preview.file_lines(&file, 40).to_vec(),
            vec![raw_preview_line("fn main() {}")],
            "plain"
        );
        assert_eq!(
            preview.cache.key.as_ref().unwrap().language,
            None,
            "the key drops the language"
        );

        preview.toggle(Some(&dir.path().join("Cargo.TOML")));
        assert_eq!(
            preview.mode,
            PreviewMode::Rendered,
            "any file with a language toggles"
        );
        assert_eq!(
            preview.file_lines(&file, 40).to_vec(),
            highlighted,
            "highlighted again"
        );
        assert_eq!(
            preview.cache.key.as_ref().unwrap().language,
            Some(Language::Rust)
        );
    }

    #[test]
    fn the_preview_mode_persists_across_selection_changes() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        std::fs::write(&a, "# A\n").unwrap();
        std::fs::write(&b, "# B\n\n**bold**\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&a, 40)), vec!["A"]);
        preview.toggle(Some(&a));
        assert_eq!(
            plain(preview.file_lines(&b, 40)),
            vec!["# B", "", "**bold**"],
            "raw on the next note"
        );
        assert_eq!(
            plain(preview.file_lines(&a, 40)),
            vec!["# A"],
            "and back on the first"
        );
        preview.toggle(Some(&b));
        assert_eq!(plain(preview.file_lines(&b, 40)), vec!["B", "", "bold"]);
    }

    #[test]
    fn a_non_markdown_file_shows_raw_whatever_the_mode() {
        let dir = tempfile::tempdir().unwrap();
        let toml = dir.path().join("c.toml");
        std::fs::write(&toml, "# comment\n[a]\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(
            plain(preview.file_lines(&toml, 40)),
            vec!["# comment", "[a]"]
        );
        preview.mode = PreviewMode::Raw;
        assert_eq!(
            plain(preview.file_lines(&toml, 40)),
            vec!["# comment", "[a]"]
        );
    }

    #[test]
    fn rendered_lines_wrap_to_the_given_width_so_their_count_is_the_scroll_count() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("w.md");
        std::fs::write(&note, "one two three four five six\n").unwrap();
        let mut preview = Preview::default();
        let lines = plain(preview.file_lines(&note, 10));
        assert!(lines.len() > 1, "{lines:?}");
        assert!(lines.iter().all(|l| l.chars().count() <= 10), "{lines:?}");
        preview.mode = PreviewMode::Raw;
        assert_eq!(
            preview.file_lines(&note, 10).len(),
            1,
            "raw is never wrapped"
        );
    }

    #[test]
    fn an_unreadable_file_shows_the_unreadable_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut preview = Preview::default();
        assert_eq!(
            plain(preview.file_lines(&dir.path().join("gone.md"), 40)),
            vec!["  unable to read file"]
        );
    }

    #[test]
    fn the_cache_reuses_lines_for_the_same_key_and_rebuilds_on_any_change() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        std::fs::write(&note, "# A\n").unwrap();
        let key = |width: u16, rendered: bool| PreviewKey::of(&note, width, rendered);
        assert!(key(40, true).is_some());
        assert_eq!(key(40, true), key(40, true));
        assert_ne!(key(40, true), key(41, true), "width");
        assert_ne!(key(40, true), key(40, false), "mode");
        assert_eq!(PreviewKey::of(&dir.path().join("gone.md"), 40, true), None);

        let mut cache = PreviewCache::default();
        let mut builds = 0;
        let mut get = |cache: &mut PreviewCache, key: Option<PreviewKey>| {
            cache
                .get(key, || {
                    builds += 1;
                    vec![Line::from("x")]
                })
                .len()
        };
        get(&mut cache, key(40, true));
        get(&mut cache, key(40, true));
        get(&mut cache, key(40, true));
        get(&mut cache, key(30, true));
        get(&mut cache, key(30, false));
        get(&mut cache, key(30, true));
        get(&mut cache, None);
        get(&mut cache, None);
        assert_eq!(
            builds, 6,
            "three draws of one key build once; each change and each unkeyed draw builds"
        );
    }

    #[test]
    fn the_cache_rebuilds_when_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        std::fs::write(&note, "# A\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&note, 40)), vec!["A"]);
        std::fs::write(&note, "# Longer title\n").unwrap();
        assert_eq!(plain(preview.file_lines(&note, 40)), vec!["Longer title"]);
    }

    /// The suffix indicator's text as drawn, without its padding.
    fn suffix_text(path: &Path, is_dir: bool) -> Option<String> {
        file_suffix(path, is_dir).map(|suffix| {
            text_of(&Line::from(suffix_spans(&suffix, PreviewMode::Rendered)))
                .trim()
                .to_string()
        })
    }

    #[test]
    fn file_suffix_is_the_lowercase_suffix_with_its_dot_or_file() {
        assert_eq!(
            suffix_text(Path::new("/n/a.md"), false).as_deref(),
            Some(".md")
        );
        assert_eq!(
            suffix_text(Path::new("/n/A.MD"), false).as_deref(),
            Some(".md")
        );
        assert_eq!(
            suffix_text(Path::new("/n/Cargo.toml"), false).as_deref(),
            Some(".toml")
        );
        assert_eq!(
            suffix_text(Path::new("/n/main.RS"), false).as_deref(),
            Some(".rs")
        );
        assert_eq!(
            suffix_text(Path::new("/n/app.ts"), false).as_deref(),
            Some(".ts")
        );
        assert_eq!(
            suffix_text(Path::new("/n/notes.txt"), false).as_deref(),
            Some(".txt")
        );
        assert_eq!(
            suffix_text(Path::new("/n/Makefile"), false).as_deref(),
            Some("file")
        );
        assert_eq!(
            suffix_text(Path::new("/n/.gitignore"), false).as_deref(),
            Some("file")
        );
        assert_eq!(
            suffix_text(Path::new("/n/trailing."), false).as_deref(),
            Some("file")
        );
        assert_eq!(
            file_suffix(Path::new("/n/ideas.md"), true),
            None,
            "a folder"
        );
        assert_eq!(
            file_suffix(Path::new("/n/ideas"), true),
            None,
            "a folder or section row"
        );
    }

    #[test]
    fn file_suffix_carries_the_highlighting_language() {
        let language = |path: &str| file_suffix(Path::new(path), false).unwrap().language;
        assert_eq!(language("/n/main.rs"), Some(Language::Rust));
        assert_eq!(language("/n/main.RS"), Some(Language::Rust));
        assert_eq!(language("/n/App.kt"), Some(Language::Kotlin));
        assert_eq!(language("/n/build.gradle.kts"), Some(Language::Kotlin));
        assert_eq!(
            suffix_text(Path::new("/n/build.gradle.kts"), false).as_deref(),
            Some(".kts")
        );
        assert_eq!(language("/n/a.md"), Some(Language::Markdown));
        assert_eq!(language("/n/a.py"), Some(Language::Python));
        assert_eq!(language("/n/a.txt"), None);
        assert_eq!(language("/n/a.yaml"), None);
        assert_eq!(language("/n/app.ts"), None);
        assert_eq!(language("/n/Makefile"), None);
    }

    #[test]
    fn a_file_over_the_limit_is_shown_plain_and_the_footer_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.rs");
        let line = "fn f() {}\n";
        let count = highlight::MAX_HIGHLIGHT_BYTES as usize / line.len() + 1;
        std::fs::write(&big, line.repeat(count)).unwrap();
        assert_eq!(
            suffix_text(&big, false).as_deref(),
            Some(".rs (not highlighted, large)")
        );
        let mut preview = Preview::default();
        let lines = preview.file_lines(&big, 40);
        assert_eq!(lines.len(), count);
        assert_eq!(lines[0], raw_preview_line("fn f() {}"));
        assert_eq!(PreviewKey::of(&big, 40, false).unwrap().language, None);

        let small = dir.path().join("small.rs");
        std::fs::write(&small, line).unwrap();
        assert_eq!(suffix_text(&small, false).as_deref(), Some(".rs"));
        let big_md = dir.path().join("big.md");
        std::fs::write(&big_md, "text\n".repeat(count * 2)).unwrap();
        assert_eq!(
            suffix_text(&big_md, false).as_deref(),
            Some(".md (not highlighted, large)")
        );
    }

    fn style_of(line: &Line<'static>, text: &str) -> Style {
        line.spans
            .iter()
            .find(|span| span.content == text)
            .unwrap_or_else(|| panic!("no span {text:?} in {line:?}"))
            .style
    }

    #[test]
    fn a_rust_file_is_highlighted_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "// note\nfn main() {\n    let s = \"x\";\n}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(
            plain(&lines),
            vec!["// note", "fn main() {", "    let s = \"x\";", "}"]
        );
        assert_eq!(style_of(&lines[0], "// note"), theme::syntax_comment());
        assert_eq!(style_of(&lines[1], "fn"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[2], "let"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[2], "\"x\""), theme::syntax_string());
        assert_eq!(
            style_of(&lines[2], "    "),
            raw_line_style("    let"),
            "uncaptured text takes the raw style"
        );
    }

    /// A guard, not a benchmark: selecting a 200 KB Rust file highlights it
    /// once; drawing it again comes from the cache.
    #[test]
    fn a_200_kb_rust_file_highlights_on_selection_and_redraws_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.rs");
        let block = "/// Doc comment.\n#[derive(Debug)]\npub struct Item { id: u32, name: String }\n\nfn build(n: u32) -> Vec<Item> {\n    (0..n).map(|id| Item { id, name: format!(\"item {id}\") }).collect()\n}\n\n";
        std::fs::write(&file, block.repeat(200 * 1024 / block.len() + 1)).unwrap();
        let mut preview = Preview::default();
        let started = std::time::Instant::now();
        let count = preview.file_lines(&file, 80).len();
        let first = started.elapsed();
        let started = std::time::Instant::now();
        assert_eq!(preview.file_lines(&file, 80).len(), count);
        let again = started.elapsed();
        eprintln!("200 KB rust: {count} lines, first {first:?}, cached {again:?}");
        assert!(first.as_secs_f64() < 5.0, "first selection took {first:?}");
        assert!(again < first, "cached {again:?} vs first {first:?}");
    }

    #[test]
    fn a_kotlin_file_is_highlighted_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("App.kt");
        std::fs::write(&file, "fun main() {\n    val s = \"x\"\n}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(
            plain(&lines),
            vec!["fun main() {", "    val s = \"x\"", "}"]
        );
        assert_eq!(style_of(&lines[0], "fun"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[1], "val"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[1], "\"x\""), theme::syntax_string());
    }

    #[test]
    fn raw_markdown_is_highlighted_and_untouched_lines_keep_the_raw_style() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("n.md");
        std::fs::write(
            &note,
            "# Title\n\nplain words\n\n```rust\nlet x = 1;\n```\n",
        )
        .unwrap();
        let mut preview = Preview {
            mode: PreviewMode::Raw,
            ..Preview::default()
        };
        let lines = preview.file_lines(&note, 40).to_vec();
        assert_eq!(
            plain(&lines),
            vec![
                "# Title",
                "",
                "plain words",
                "",
                "```rust",
                "let x = 1;",
                "```"
            ]
        );
        assert_eq!(
            lines[2],
            raw_preview_line("plain words"),
            "a line no capture touched"
        );
        assert_eq!(
            style_of(&lines[0], " Title"),
            raw_line_style("# Title"),
            "heading text keeps the heading style"
        );
        assert_eq!(style_of(&lines[5], "let"), theme::syntax_keyword());
    }

    #[test]
    fn a_file_without_a_language_stays_raw() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "# not markdown\nfn main() {}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(
            lines,
            vec![
                raw_preview_line("# not markdown"),
                raw_preview_line("fn main() {}")
            ]
        );
        assert_eq!(PreviewKey::of(&file, 40, false).unwrap().language, None);
    }

    #[test]
    fn the_cache_key_carries_the_language_so_a_different_one_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();
        let key = PreviewKey::of(&file, 40, false).unwrap();
        assert_eq!(key.language, Some(Language::Rust));
        let note = dir.path().join("n.md");
        std::fs::write(&note, "# A\n").unwrap();
        assert_eq!(
            PreviewKey::of(&note, 40, false).unwrap().language,
            Some(Language::Markdown)
        );
        assert_eq!(
            PreviewKey::of(&note, 40, true).unwrap().language,
            None,
            "rendered highlights fences itself"
        );

        let other = PreviewKey {
            language: Some(Language::Kotlin),
            ..key.clone()
        };
        assert_ne!(key, other);
        let mut cache = PreviewCache::default();
        let mut builds = 0;
        for key in [key.clone(), key.clone(), other, key] {
            cache.get(Some(key), || {
                builds += 1;
                Vec::new()
            });
        }
        assert_eq!(builds, 3, "same key reuses, a language change rebuilds");
    }

    #[test]
    fn the_toggle_key_is_a_free_browse_key_listed_once_in_the_view_group() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "p").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].help,
            "toggle the preview: rendered / raw markdown, highlighted / plain code"
        );
        assert_eq!(rows[0].group, Group::View);
        assert_eq!(rows[0].modes, BROWSE);
        for mode in [
            Mode::Tag,
            Mode::Filter,
            Mode::Rename,
            Mode::NewItem,
            Mode::ConfirmDelete,
            Mode::VimCommand,
            Mode::Move,
            Mode::SetScope,
            Mode::ConfirmMove,
        ] {
            assert!(
                !TREE_KEYS
                    .iter()
                    .any(|k| k.key == "p" && k.modes.contains(&mode)),
                "{mode:?}"
            );
        }
    }

    fn shown_with(table: &[KeyHint], width: usize) -> Vec<&'static str> {
        footer::select(table, Mode::Normal, &[], width)
            .left
            .iter()
            .map(|&(i, _)| table[i].key)
            .collect()
    }

    #[test]
    fn the_toggle_hint_names_the_action_shows_only_for_markdown_and_drops_first() {
        assert!(
            !shown_with(&tree_keys(None), 300).contains(&"p"),
            "hidden when the row has no language"
        );
        let rendered = tree_keys(Some(PreviewToggle::markdown(PreviewMode::Rendered)));
        let raw = tree_keys(Some(PreviewToggle::markdown(PreviewMode::Raw)));
        assert_eq!(
            rendered.len(),
            TREE_KEYS.len(),
            "same rows, so help and footer indices agree"
        );
        let line = |table: &[KeyHint]| text_of(&footer::line(table, Mode::Normal, &[], 300));
        assert!(line(&rendered).contains("  p raw  "), "{}", line(&rendered));
        assert!(line(&raw).contains("  p rendered  "), "{}", line(&raw));
        let first_width = |key: &str| {
            (0..300)
                .find(|&w| shown_with(&rendered, w).contains(&key))
                .unwrap()
        };
        assert!(first_width("p") > first_width("J/K"), "p drops before J/K");
        for mode in [Mode::Filter, Mode::Tag, Mode::Rename, Mode::NewItem] {
            let sel = footer::select(&rendered, mode, &[], 300);
            assert!(
                !sel.left.iter().any(|&(i, _)| rendered[i].key == "p"),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn the_browse_footer_leads_with_the_mark_count_and_no_file_type() {
        let keys = tree_keys(Some(PreviewToggle::markdown(PreviewMode::Rendered)));
        let marks = text_of(&browse_footer(&keys, 3, Mode::Normal, &[], 200));
        assert!(marks.starts_with(" 3 marked  open  tags"), "{marks}");
        let hints = browse_footer(&keys, 0, Mode::Normal, &[], 200);
        assert_eq!(
            text_of(&hints),
            text_of(&footer::line(&keys, Mode::Normal, &[], 200)),
            "hints only"
        );
        let neither = browse_footer(&TREE_KEYS.to_vec(), 0, Mode::Normal, &[], 120);
        assert_eq!(
            text_of(&neither),
            text_of(&footer::line(TREE_KEYS, Mode::Normal, &[], 120))
        );
        for width in 0..30 {
            let _ = browse_footer(&keys, 12, Mode::Normal, &[], width);
        }
    }

    // --- Suffix indicator and preview border (NZ-39) ---

    fn rendered_suffix(path: &str, mode: PreviewMode) -> Style {
        suffix_style(&file_suffix(Path::new(path), false).unwrap(), mode)
    }

    #[test]
    fn the_suffix_is_bold_in_its_language_colour_while_the_renderer_or_highlighter_is_on() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        assert_eq!(
            rendered_suffix("/n/a.md", PreviewMode::Rendered),
            bold.fg(theme::language_color(Language::Markdown))
        );
        assert_eq!(
            rendered_suffix("/n/main.RS", PreviewMode::Rendered),
            bold.fg(theme::language_color(Language::Rust))
        );
        assert_eq!(
            rendered_suffix("/n/App.kt", PreviewMode::Rendered),
            bold.fg(theme::language_color(Language::Kotlin))
        );
    }

    #[test]
    fn the_suffix_is_plain_bold_while_off_or_without_a_language() {
        let plain_bold = Style::default()
            .fg(theme::NEUTRAL_SUFFIX)
            .add_modifier(Modifier::BOLD);
        assert_eq!(
            rendered_suffix("/n/a.md", PreviewMode::Raw),
            plain_bold,
            "markdown, raw"
        );
        assert_eq!(
            rendered_suffix("/n/main.rs", PreviewMode::Raw),
            plain_bold,
            "code, plain"
        );
        for mode in [PreviewMode::Rendered, PreviewMode::Raw] {
            assert_eq!(
                rendered_suffix("/n/app.ts", mode),
                plain_bold,
                "unknown suffix, {mode:?}"
            );
            let file = rendered_suffix("/n/Makefile", mode);
            assert!(
                !file.add_modifier.contains(Modifier::BOLD),
                "`file` is a word, not bold"
            );
        }
    }

    #[test]
    fn a_large_or_unavailable_file_shows_plain_bold_and_its_note_dim() {
        let large = FileSuffix {
            text: ".rs".into(),
            language: Some(Language::Rust),
            note: Some("not highlighted, large"),
        };
        let plain_bold = Style::default()
            .fg(theme::NEUTRAL_SUFFIX)
            .add_modifier(Modifier::BOLD);
        assert_eq!(suffix_style(&large, PreviewMode::Rendered), plain_bold);
        let spans = suffix_spans(&large, PreviewMode::Rendered);
        assert_eq!(
            text_of(&Line::from(spans.clone())),
            " .rs (not highlighted, large) "
        );
        let note = spans
            .iter()
            .find(|span| span.content.contains("large"))
            .unwrap();
        assert_eq!(note.style, theme::dimmed());
        let unavailable = FileSuffix {
            note: Some("highlighter unavailable"),
            ..large
        };
        assert_eq!(
            text_of(&Line::from(suffix_spans(&unavailable, PreviewMode::Raw))),
            " .rs (highlighter unavailable) "
        );
    }

    #[test]
    fn the_toggle_hint_names_the_action_per_file_kind() {
        let dir = tempfile::tempdir().unwrap();
        let toggle = |name: &str, mode| {
            PreviewToggle::for_file(&dir.path().join(name), mode).map(PreviewToggle::desc)
        };
        assert_eq!(toggle("a.md", PreviewMode::Rendered), Some("raw"));
        assert_eq!(toggle("a.md", PreviewMode::Raw), Some("rendered"));
        assert_eq!(toggle("main.rs", PreviewMode::Rendered), Some("plain"));
        assert_eq!(toggle("main.rs", PreviewMode::Raw), Some("highlighted"));
        assert_eq!(toggle("Cargo.toml", PreviewMode::Rendered), Some("plain"));
        assert_eq!(toggle("app.ts", PreviewMode::Rendered), None);
        assert_eq!(toggle("Makefile", PreviewMode::Rendered), None);
        let code = tree_keys(PreviewToggle::for_file(
            &dir.path().join("main.rs"),
            PreviewMode::Rendered,
        ));
        let p = code.iter().find(|hint| hint.key == "p").unwrap();
        assert_eq!(p.desc, "plain");
        assert!(
            shown_with(&code, 300).contains(&"p"),
            "shown for a code file"
        );
        let line = text_of(&footer::line(&code, Mode::Normal, &[], 300));
        assert!(
            line.contains("  plain  "),
            "the footer folds `p` into `plain`: {line}"
        );
        let code = tree_keys(PreviewToggle::for_file(
            &dir.path().join("main.rs"),
            PreviewMode::Raw,
        ));
        let line = text_of(&footer::line(&code, Mode::Normal, &[], 300));
        assert!(line.contains("  p highlighted  "), "{line}");
    }

    #[test]
    fn the_bottom_right_path_is_relative_to_the_section_root() {
        let root = Path::new("/home/u/notez/personal/notez");
        assert_eq!(
            section_relative_path(&root.join("plan.md"), root),
            "plan.md",
            "a root-level note"
        );
        assert_eq!(
            section_relative_path(&root.join("ideas/plan.md"), root),
            "ideas/plan.md",
            "a nested note"
        );
        let docs = Path::new("/home/u/Repos/notez/docs");
        assert_eq!(
            section_relative_path(&docs.join("specs/agent-workflow.md"), docs),
            "specs/agent-workflow.md",
            "a docs file"
        );
        assert_eq!(
            section_relative_path(Path::new("/elsewhere/x.md"), root),
            "x.md",
            "outside the root: the bare name"
        );
    }

    fn bottom_texts(
        spans: Vec<Span<'static>>,
        path: &str,
        width: usize,
    ) -> (String, Option<String>) {
        let (left, right) = preview_bottom_titles(spans, path, width);
        (text_of(&left), right.map(|line| text_of(&line)))
    }

    #[test]
    fn the_preview_border_shows_the_suffix_left_and_the_path_right_clipped_from_the_left() {
        let suffix = || vec![Span::raw(" .md ")];
        assert_eq!(
            bottom_texts(suffix(), "ideas/plan.md", 40),
            (" .md ".into(), Some(" ideas/plan.md ".into()))
        );
        // 5 for the suffix, 1 space, then " …plan.md " in the 10 left.
        assert_eq!(
            bottom_texts(suffix(), "ideas/plan.md", 16),
            (" .md ".into(), Some(" …plan.md ".into()))
        );
        for width in 0..40 {
            let (left, right) = bottom_texts(suffix(), "ideas/plan.md", width);
            if let Some(right) = right {
                let used = left.chars().count() + 1 + right.chars().count();
                assert!(used <= width, "width {width}: {left:?} {right:?}");
                assert!(
                    right.ends_with("d "),
                    "the end of the path stays visible: {right:?}"
                );
            }
        }
        assert_eq!(
            bottom_texts(suffix(), "ideas/plan.md", 8).1,
            None,
            "the path gives way first"
        );
        assert_eq!(
            bottom_texts(suffix(), "ideas/plan.md", 3).0,
            " .md ",
            "the suffix stays"
        );
    }

    #[test]
    fn the_preview_title_shows_the_section_root_with_a_tilde_clipped_from_the_left() {
        let root = notez_core::util::tilde::expand("~/notez/personal/notez");
        assert_eq!(tilde_path(&root), "~/notez/personal/notez");
        let name = || vec![Span::raw("plan.md ")];
        let title = |width| {
            text_of(&Line::from(with_section_root(
                name(),
                "~/Repos/notez/docs",
                width,
            )))
        };
        assert_eq!(title(80), "plan.md ~/Repos/notez/docs ");
        assert_eq!(title(18), "plan.md …tez/docs ");
        assert_eq!(
            title(9),
            "plan.md ",
            "no room: the root goes, the name stays"
        );
        let root_span = with_section_root(name(), "~/x", 80).pop().unwrap();
        assert_eq!(root_span.style, theme::dimmed());
    }
}
