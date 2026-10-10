//! Diagnostics for the preview pane: what is off in the selected file.
//!
//! [`lint`] combines two in-process sources and runs no external tool:
//!
//! - Syntax diagnostics: every `ERROR` and `MISSING` node in the tree-sitter
//!   parse of a code file (the NZ-27 grammars, see [`highlight::parse`]).
//! - Markdown checks: structural checks over the pulldown-cmark events of a
//!   markdown note, one function each in [`MARKDOWN_CHECKS`].
//!
//! The preview is read only, so a diagnostic only points at a line; fixing
//! happens in the editor.

use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{
    BrokenLink, CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

use super::highlight::{self, Language, MAX_HIGHLIGHT_BYTES};

/// How serious a [`Diagnostic`] is. Syntax diagnostics are errors, markdown
/// checks are warnings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// One issue on one source line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// 0-based source line, as `str::lines` counts them.
    pub line: usize,
    pub severity: Severity,
    /// Short and lowercase, for an issue list (`syntax error`,
    /// `missing }`, `trailing whitespace`).
    pub message: String,
}

/// Lint `text`, the contents of the file at `path`, as `language`.
///
/// - `Some` code language: one [`Severity::Error`] per tree-sitter `ERROR`
///   node (`syntax error`) and `MISSING` node (`missing <kind>`). A grammar
///   that fails to load yields no syntax diagnostics.
/// - `Some(Language::Markdown)`: the [`MARKDOWN_CHECKS`], all
///   [`Severity::Warning`]. Markdown is deliberately not also parsed with
///   tree-sitter: its block grammar recovers from almost anything, the few
///   errors it does report (an unclosed fence) duplicate a markdown check,
///   and fenced code would need the injection machinery to be linted
///   per language. One source per file keeps every issue reported once.
/// - `None`: nothing.
///
/// `path` is used only by the markdown checks: its file name selects the
/// `TODO.md` task check, and its parent directory resolves relative links.
/// The relative link check stats candidate files (see [`link_probe_path`]
/// for which targets are never probed); no other check touches the
/// filesystem.
///
/// Text over [`MAX_HIGHLIGHT_BYTES`] (1 MiB) returns no diagnostics, the
/// same bound that keeps highlighting off for large files, so selecting a
/// large file stays fast. The result is sorted by line, then by message.
pub fn lint(path: &Path, text: &str, language: Option<Language>) -> Vec<Diagnostic> {
    if text.len() as u64 > MAX_HIGHLIGHT_BYTES {
        return Vec::new();
    }
    let mut diagnostics = match language {
        None => Vec::new(),
        Some(Language::Markdown) => markdown_diagnostics(path, text),
        Some(language) => syntax_diagnostics(text, language),
    };
    diagnostics.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.message.cmp(&b.message)));
    diagnostics
}

/// Walk the parse tree depth first with a cursor (no recursion, so deeply
/// nested input cannot overflow the stack) and report every `ERROR` and
/// `MISSING` node.
fn syntax_diagnostics(text: &str, language: Language) -> Vec<Diagnostic> {
    let Some(tree) = highlight::parse(text, language) else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    if !tree.root_node().has_error() {
        return diagnostics;
    }
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let message = if node.is_error() {
            Some("syntax error".to_string())
        } else if node.is_missing() {
            Some(format!("missing {}", node.kind()))
        } else {
            None
        };
        if let Some(message) = message {
            diagnostics.push(Diagnostic {
                line: node.start_position().row,
                severity: Severity::Error,
                message,
            });
        }
        // Subtrees without errors cannot contain an ERROR or MISSING node.
        if node.has_error() && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return diagnostics;
            }
        }
    }
}

/// A markdown check: reads the parsed note, appends its warnings.
type MarkdownCheck = fn(&Note, &mut Vec<Diagnostic>);

/// Every markdown check, by name. Remove a row to turn a check
/// off; [`lint`] runs them in this order and sorts the result.
const MARKDOWN_CHECKS: &[(&str, MarkdownCheck)] = &[
    ("heading-level-jump", heading_level_jump),
    ("duplicate-heading", duplicate_heading),
    ("unterminated-fence", unterminated_fence),
    ("missing-link-target", missing_link_target),
    ("undefined-reference", undefined_reference),
    ("trailing-whitespace", trailing_whitespace),
    ("todo-task-line", todo_task_line),
    ("no-title-heading", no_title_heading),
];

/// The options `tui/markdown.rs` renders with, so the checks see the same
/// document structure the reader does.
const MARKDOWN_OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_FOOTNOTES)
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS);

/// A markdown note parsed once and shared by every check.
struct Note<'a> {
    path: &'a Path,
    text: &'a str,
    /// Byte offset where each source line starts.
    line_starts: Vec<usize>,
    events: Vec<(Event<'a>, Range<usize>)>,
    /// `[text][ref]` and `[text][]` links whose reference has no
    /// definition, with their source span and reference label.
    undefined_references: Vec<(Range<usize>, String)>,
}

impl<'a> Note<'a> {
    fn parse(path: &'a Path, text: &'a str) -> Note<'a> {
        let mut undefined_references = Vec::new();
        let callback = |link: BrokenLink<'a>| -> Option<(CowStr<'a>, CowStr<'a>)> {
            if matches!(
                link.link_type,
                LinkType::Reference
                    | LinkType::ReferenceUnknown
                    | LinkType::Collapsed
                    | LinkType::CollapsedUnknown
            ) {
                undefined_references.push((link.span, link.reference.to_string()));
            }
            None
        };
        let events = Parser::new_with_broken_link_callback(text, MARKDOWN_OPTIONS, Some(callback))
            .into_offset_iter()
            .collect();
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Note {
            path,
            text,
            line_starts,
            events,
            undefined_references,
        }
    }

    /// The 0-based line containing byte `offset`.
    fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    fn warn(&self, out: &mut Vec<Diagnostic>, offset: usize, message: String) {
        out.push(Diagnostic {
            line: self.line_of(offset),
            severity: Severity::Warning,
            message,
        });
    }

    /// Each heading's level, source offset and plain text.
    fn headings(&self) -> Vec<(HeadingLevel, usize, String)> {
        let mut headings = Vec::new();
        let mut current: Option<(HeadingLevel, usize, String)> = None;
        for (event, range) in &self.events {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    current = Some((*level, range.start, String::new()));
                }
                Event::Text(text) | Event::Code(text) => {
                    if let Some((_, _, heading)) = current.as_mut() {
                        heading.push_str(text);
                    }
                }
                Event::End(TagEnd::Heading(_)) => headings.extend(current.take()),
                _ => {}
            }
        }
        headings
    }
}

fn markdown_diagnostics(path: &Path, text: &str) -> Vec<Diagnostic> {
    let note = Note::parse(path, text);
    let mut diagnostics = Vec::new();
    for (_, check) in MARKDOWN_CHECKS {
        check(&note, &mut diagnostics);
    }
    diagnostics
}

fn hashes(level: HeadingLevel) -> String {
    "#".repeat(level as usize)
}

/// A heading more than one level deeper than the heading before it
/// (`#` straight to `###`). Going back up any number of levels is fine.
fn heading_level_jump(note: &Note, out: &mut Vec<Diagnostic>) {
    let mut previous: Option<HeadingLevel> = None;
    for (level, offset, _) in note.headings() {
        if let Some(previous) = previous {
            if level as usize > previous as usize + 1 {
                let message = format!(
                    "heading jumps from {} to {}",
                    hashes(previous),
                    hashes(level)
                );
                note.warn(out, offset, message);
            }
        }
        previous = Some(level);
    }
}

/// A heading whose text (trimmed, case-insensitive) repeats an earlier
/// heading's, at any level. Reported on the repeat.
fn duplicate_heading(note: &Note, out: &mut Vec<Diagnostic>) {
    let mut seen = std::collections::HashSet::new();
    for (_, offset, text) in note.headings() {
        let text = text.trim();
        if !text.is_empty() && !seen.insert(text.to_lowercase()) {
            note.warn(out, offset, format!("duplicate heading \"{text}\""));
        }
    }
}

/// A fenced code block that runs to the end of its container (the note, a
/// list item, a block quote) without a closing fence. Reported on the
/// opening fence.
fn unterminated_fence(note: &Note, out: &mut Vec<Diagnostic>) {
    for (event, range) in &note.events {
        if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) = event {
            if !fence_is_closed(&note.text[range.clone()]) {
                note.warn(out, range.start, "unterminated code fence".to_string());
            }
        }
    }
}

/// Whether a fenced block's source ends in a closing fence: a last line,
/// distinct from the opening one, made of at least as many of the opening
/// fence character. Container prefixes (indent, `>`) are ignored.
fn fence_is_closed(block: &str) -> bool {
    let strip = |line: &str| -> String {
        line.trim_start_matches(|c: char| c.is_whitespace() || c == '>')
            .trim_end()
            .to_string()
    };
    let mut lines = block.lines();
    let Some(opening) = lines.next().map(strip) else {
        return false;
    };
    let Some(closing) = lines.last().map(strip) else {
        return false;
    };
    let Some(fence) = opening.chars().next().filter(|c| *c == '`' || *c == '~') else {
        return false;
    };
    let width = opening.chars().take_while(|c| *c == fence).count();
    closing.chars().count() >= width && closing.chars().all(|c| c == fence)
}

/// A relative link whose target file does not exist, resolved against the
/// note's directory. Only targets [`link_probe_path`] accepts are checked.
fn missing_link_target(note: &Note, out: &mut Vec<Diagnostic>) {
    let Some(base) = note.path.parent() else {
        return;
    };
    for (event, range) in &note.events {
        let Event::Start(Tag::Link {
            link_type,
            dest_url,
            ..
        }) = event
        else {
            continue;
        };
        if matches!(link_type, LinkType::Autolink | LinkType::Email) {
            continue;
        }
        if let Some(target) = link_probe_path(base, dest_url) {
            if !target.exists() {
                note.warn(
                    out,
                    range.start,
                    format!("link target not found: {dest_url}"),
                );
            }
        }
    }
}

/// The file a link destination points at, or `None` for a destination that
/// must not be probed: empty or anchor-only (`#section`), with a URL scheme
/// (`https:`, `mailto:`), absolute (`/x`, `//host`, `~/x`), or with a `..`
/// component. `..` is refused outright because the lint pass does not know
/// the vault root and must never stat files outside it; a link to a
/// sibling directory is therefore not checked. A `#fragment` is stripped
/// and `%XX` escapes are decoded (an invalid escape means no probe).
fn link_probe_path(base: &Path, destination: &str) -> Option<PathBuf> {
    let target = destination.split('#').next().unwrap_or("");
    if target.is_empty() || has_scheme(target) {
        return None;
    }
    if target.starts_with(['/', '\\', '~']) {
        return None;
    }
    let decoded = percent_decode(target)?;
    let relative = Path::new(&decoded);
    let is_plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if relative.is_absolute() || !is_plain {
        return None;
    }
    Some(base.join(relative))
}

/// Whether `target` starts with a URL scheme (`https:`, `mailto:`,
/// `file:`): a letter, then letters, digits, `+`, `-` or `.`, then `:`.
fn has_scheme(target: &str) -> bool {
    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Decode `%XX` escapes. `None` for a malformed escape or non-UTF-8 result.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// A full (`[text][ref]`) or collapsed (`[text][]`) reference link whose
/// reference has no definition. Shortcut links (`[ref]`) are not reported:
/// any bracketed text parses as one, so they would flag ordinary prose.
fn undefined_reference(note: &Note, out: &mut Vec<Diagnostic>) {
    for (span, reference) in &note.undefined_references {
        note.warn(
            out,
            span.start,
            format!("undefined reference [{reference}]"),
        );
    }
}

/// A line ending in spaces or tabs, including a whitespace-only line.
/// Exactly two spaces after non-whitespace text is the markdown hard break
/// and is exempt, as in markdownlint's MD009.
fn trailing_whitespace(note: &Note, out: &mut Vec<Diagnostic>) {
    for (line, text) in note.text.lines().enumerate() {
        let content = text.trim_end_matches([' ', '\t']);
        let is_hard_break = !content.trim().is_empty() && &text[content.len()..] == "  ";
        if content.len() < text.len() && !is_hard_break {
            out.push(Diagnostic {
                line,
                severity: Severity::Warning,
                message: "trailing whitespace".to_string(),
            });
        }
    }
}

/// In a file named `TODO.md`, a line starting `- [` (after indentation)
/// that the notez todo parser would not read as a task. The accepted forms
/// mirror `notez-core`'s `todo` module: `- [ ] `, `- [/] `, `- [x] ` and
/// `- [X] `, each followed by the task text.
fn todo_task_line(note: &Note, out: &mut Vec<Diagnostic>) {
    if note.path.file_name().and_then(|name| name.to_str()) != Some("TODO.md") {
        return;
    }
    const TASK_PREFIXES: [&str; 4] = ["- [ ] ", "- [/] ", "- [x] ", "- [X] "];
    for (line, text) in note.text.lines().enumerate() {
        let trimmed = text.trim();
        if trimmed.starts_with("- [") && !TASK_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
            out.push(Diagnostic {
                line,
                severity: Severity::Warning,
                message: "malformed task line (expected - [ ], - [/] or - [x])".to_string(),
            });
        }
    }
}

/// A non-empty note without a level-one (`#` or `===`) heading. Reported
/// on line 0. A blank note gets no warning: there is nothing to title yet.
fn no_title_heading(note: &Note, out: &mut Vec<Diagnostic>) {
    if note.text.trim().is_empty() {
        return;
    }
    let has_title = note
        .headings()
        .iter()
        .any(|(level, _, _)| *level == HeadingLevel::H1);
    if !has_title {
        note.warn(out, 0, "no # heading".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Run one markdown check, by its table name, on `text` as `note.md`.
    fn check(name: &str, text: &str) -> Vec<Diagnostic> {
        check_at(name, Path::new("note.md"), text)
    }

    fn check_at(name: &str, path: &Path, text: &str) -> Vec<Diagnostic> {
        let (_, run) = MARKDOWN_CHECKS
            .iter()
            .find(|(check, _)| *check == name)
            .unwrap_or_else(|| panic!("no check named {name}"));
        let note = Note::parse(path, text);
        let mut out = Vec::new();
        run(&note, &mut out);
        out
    }

    fn lines_and_messages(diagnostics: &[Diagnostic]) -> Vec<(usize, &str)> {
        diagnostics
            .iter()
            .map(|d| (d.line, d.message.as_str()))
            .collect()
    }

    fn warning(line: usize, message: &str) -> Diagnostic {
        Diagnostic {
            line,
            severity: Severity::Warning,
            message: message.to_string(),
        }
    }

    // --- markdown checks, one positive and one negative each ---

    #[test]
    fn a_heading_level_jump_warns_on_the_deeper_heading() {
        let got = check("heading-level-jump", "# Title\n\ntext\n\n### Deep\n");
        assert_eq!(got, [warning(4, "heading jumps from # to ###")]);
    }

    #[test]
    fn heading_levels_one_step_down_or_back_up_do_not_warn() {
        let text = "# A\n## B\n### C\n# D\n## E\n";
        assert!(check("heading-level-jump", text).is_empty());
    }

    #[test]
    fn a_repeated_heading_warns_ignoring_case_and_padding() {
        let got = check("duplicate-heading", "# Notes\n\n## Setup\n\n## setup  \n");
        assert_eq!(got, [warning(4, "duplicate heading \"setup\"")]);
    }

    #[test]
    fn distinct_headings_do_not_warn() {
        assert!(check("duplicate-heading", "# Notes\n## Setup\n## Usage\n").is_empty());
    }

    #[test]
    fn an_unterminated_fence_warns_on_its_opening_line() {
        let got = check("unterminated-fence", "# T\n\n```rust\nfn main() {}\n");
        assert_eq!(got, [warning(2, "unterminated code fence")]);
        let got = check("unterminated-fence", "# T\n~~~~\nx\n~~~\n");
        assert_eq!(lines_and_messages(&got), [(1, "unterminated code fence")]);
        let got = check("unterminated-fence", "```");
        assert_eq!(lines_and_messages(&got), [(0, "unterminated code fence")]);
    }

    #[test]
    fn closed_fences_do_not_warn() {
        let text = "```rust\nfn main() {}\n```\n\n~~~~\nx\n~~~~~\n\n- item\n\n  ```\n  y\n  ```\n\n> ```\n> z\n> ```\n";
        assert!(check("unterminated-fence", text).is_empty());
    }

    #[test]
    fn an_undefined_reference_warns_and_a_defined_one_does_not() {
        let text = "# T\n\nSee [docs][manual] and [guide][].\n\nSee [ok][def].\n\n[def]: https://example.com\n";
        let got = check("undefined-reference", text);
        assert_eq!(
            lines_and_messages(&got),
            [
                (2, "undefined reference [manual]"),
                (2, "undefined reference [guide]")
            ]
        );
    }

    #[test]
    fn defined_references_and_bare_brackets_do_not_warn() {
        let text = "See [ok][def], [def][] and [just brackets].\n\n[def]: other.md\n";
        assert!(check("undefined-reference", text).is_empty());
    }

    #[test]
    fn trailing_whitespace_warns_per_line() {
        let got = check("trailing-whitespace", "# T\nclean\nspace \ntab\t\n   \nend");
        assert_eq!(
            lines_and_messages(&got),
            [
                (2, "trailing whitespace"),
                (3, "trailing whitespace"),
                (4, "trailing whitespace")
            ]
        );
    }

    #[test]
    fn lines_without_trailing_whitespace_do_not_warn() {
        let text = "# T\r\n  indented\r\n\r\nend\n";
        assert!(check("trailing-whitespace", text).is_empty());
    }

    #[test]
    fn a_two_space_hard_break_does_not_warn() {
        assert!(check("trailing-whitespace", "# T\ntext  \nnext\n").is_empty());
    }

    #[test]
    fn one_or_three_trailing_spaces_and_blank_two_space_lines_warn() {
        let got = check("trailing-whitespace", "# T\ntext   \ntext \n  \n");
        assert_eq!(
            lines_and_messages(&got),
            [
                (1, "trailing whitespace"),
                (2, "trailing whitespace"),
                (3, "trailing whitespace")
            ]
        );
    }

    #[test]
    fn a_malformed_task_line_in_todo_md_warns() {
        let text = "# TODO\n- [ ] ok\n- [] broken\n  - [y] nested\n- [x]\n";
        let got = check_at("todo-task-line", Path::new("vault/TODO.md"), text);
        let message = "malformed task line (expected - [ ], - [/] or - [x])";
        assert_eq!(
            lines_and_messages(&got),
            [(2, message), (3, message), (4, message)]
        );
    }

    #[test]
    fn valid_task_lines_and_other_files_do_not_warn() {
        let text = "# TODO\n- [ ] open\n- [/] half\n- [x] done\n  - [X] done\n- plain item\n";
        assert!(check_at("todo-task-line", Path::new("TODO.md"), text).is_empty());
        let broken = "- [] broken\n";
        assert!(check_at("todo-task-line", Path::new("notes.md"), broken).is_empty());
        assert!(check_at("todo-task-line", Path::new("todo.md"), broken).is_empty());
    }

    #[test]
    fn a_note_without_a_level_one_heading_warns_on_line_zero() {
        let got = check("no-title-heading", "intro\n\n## Section\n");
        assert_eq!(got, [warning(0, "no # heading")]);
    }

    #[test]
    fn a_titled_or_blank_note_does_not_warn() {
        assert!(check("no-title-heading", "intro\n\n# Title\n").is_empty());
        assert!(check("no-title-heading", "Title\n=====\n").is_empty());
        assert!(check("no-title-heading", "").is_empty());
        assert!(check("no-title-heading", " \n\n").is_empty());
    }

    // --- relative links ---

    #[test]
    fn a_missing_relative_link_target_warns_and_an_existing_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("exists.md"), "x").unwrap();
        std::fs::create_dir(dir.path().join("sub dir")).unwrap();
        std::fs::write(dir.path().join("sub dir/deep.md"), "x").unwrap();
        let note = dir.path().join("note.md");
        let text = "# T\n\n[a](exists.md)\n[b](missing.md)\n[c](exists.md#part)\n[d](./sub%20dir/deep.md)\n[e](<sub dir/deep.md>)\n[f](gone/x.md#y)\n";
        let got = check_at("missing-link-target", &note, text);
        assert_eq!(
            got,
            [
                warning(3, "link target not found: missing.md"),
                warning(7, "link target not found: gone/x.md#y"),
            ]
        );
    }

    #[test]
    fn absolute_urls_anchors_and_parent_escapes_are_never_probed() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("note.md");
        let unprobed = [
            "/definitely/not/here.md",
            "//host/share.md",
            "~/notes/x.md",
            "\\\\server\\x.md",
            "https://example.com/missing.md",
            "mailto:me@example.com",
            "file:///etc/passwd",
            "#section",
            "",
            "../outside.md",
            "sub/../../outside.md",
            "bad%zzescape.md",
        ];
        for destination in unprobed {
            assert_eq!(
                link_probe_path(dir.path(), destination),
                None,
                "{destination}"
            );
        }
        let text: String = unprobed
            .iter()
            .filter(|d| !d.is_empty())
            .map(|d| format!("[x](<{d}>)\n"))
            .collect();
        assert!(check_at("missing-link-target", &note, &text).is_empty());
        for destination in ["missing.md", "a/b.md", "./c.md#frag"] {
            let probe = link_probe_path(dir.path(), destination).unwrap();
            assert!(probe.starts_with(dir.path()), "{destination} -> {probe:?}");
        }
    }

    // --- lint() as a whole ---

    fn errors(text: &str, language: Language) -> Vec<Diagnostic> {
        let got = lint(Path::new("file"), text, Some(language));
        assert!(got.iter().all(|d| d.severity == Severity::Error), "{got:?}");
        got
    }

    #[test]
    fn rust_syntax_errors_and_missing_nodes_are_reported() {
        assert!(errors("fn main() {\n    let x = 1;\n}\n", Language::Rust).is_empty());
        let got = errors("fn main() {\n    let x = ;\n}\n", Language::Rust);
        assert_eq!(lines_and_messages(&got), [(1, "syntax error")]);
        let got = errors("fn main() {\n    let x = 1\n}\n", Language::Rust);
        assert_eq!(lines_and_messages(&got), [(1, "missing ;")]);
    }

    #[test]
    fn python_syntax_errors_are_reported() {
        assert!(errors("def f(x):\n    return x\n", Language::Python).is_empty());
        let got = errors(
            "def f(x):\n    return x\n\ndef g(:\n    pass\n",
            Language::Python,
        );
        assert!(!got.is_empty());
        assert!(got.iter().all(|d| d.line == 3), "{got:?}");
    }

    #[test]
    fn json_syntax_errors_are_reported() {
        assert!(errors("{\n  \"a\": 1,\n  \"b\": [true, null]\n}\n", Language::Json).is_empty());
        let got = errors("{\n  \"a\": 1,\n  \"b\": \n}\n", Language::Json);
        assert!(!got.is_empty());
        assert!(got.iter().all(|d| (2..=3).contains(&d.line)), "{got:?}");
    }

    #[test]
    fn every_language_lints_clean_text_without_panicking() {
        for language in Language::ALL {
            lint(Path::new("file"), "", Some(language));
            lint(Path::new("file"), "}}}{{{ ((( ``` \"", Some(language));
        }
    }

    #[test]
    fn markdown_gets_markdown_checks_only_never_tree_sitter_errors() {
        let text = "# T\n\n```rust\nfn main( {\n";
        let got = lint(Path::new("note.md"), text, Some(Language::Markdown));
        assert_eq!(got, [warning(2, "unterminated code fence")]);
    }

    #[test]
    fn no_language_means_no_diagnostics() {
        let text = "fn main( {\ntrailing \n";
        assert!(lint(Path::new("file.txt"), text, None).is_empty());
    }

    #[test]
    fn results_are_sorted_by_line_then_message() {
        let text = "intro \n\n### Deep \n";
        let got = lint(Path::new("note.md"), text, Some(Language::Markdown));
        assert_eq!(
            lines_and_messages(&got),
            [
                (0, "no # heading"),
                (0, "trailing whitespace"),
                (2, "trailing whitespace"),
            ]
        );
    }

    #[test]
    fn files_over_one_mebibyte_get_no_diagnostics() {
        let line = "trailing \n";
        let mut text = line.repeat(MAX_HIGHLIGHT_BYTES as usize / line.len());
        assert!(!lint(Path::new("note.md"), &text, Some(Language::Markdown)).is_empty());
        while text.len() as u64 <= MAX_HIGHLIGHT_BYTES {
            text.push_str(line);
        }
        assert!(lint(Path::new("note.md"), &text, Some(Language::Markdown)).is_empty());
        let rust = format!("fn main( {{\n{}", "// x\n".repeat(220_000));
        assert!(rust.len() as u64 > MAX_HIGHLIGHT_BYTES);
        assert!(lint(Path::new("main.rs"), &rust, Some(Language::Rust)).is_empty());
    }

    /// A guard against quadratic behaviour, not a benchmark: 500 KB of
    /// headings, prose, links, references and fences. The 5 s bound is
    /// loose so shared CI runners never flake.
    #[test]
    fn a_500_kb_note_lints_in_bounded_time() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("exists.md"), "x").unwrap();
        let note = dir.path().join("note.md");
        let mut text = String::from("# Title\n\n");
        let mut section = 0;
        while text.len() < 500 * 1024 {
            section += 1;
            text.push_str(&format!("## Section {section}\n\n"));
            text.push_str("Some *text* with a [link](exists.md), a [gone](gone.md), ");
            text.push_str("a [ref][missing] and `code` that goes on for a while. \n\n");
            text.push_str("```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n");
        }
        let started = Instant::now();
        let got = lint(&note, &text, Some(Language::Markdown));
        let elapsed = started.elapsed();
        eprintln!(
            "500 KB note: {} bytes, {} diagnostics, {elapsed:?}",
            text.len(),
            got.len()
        );
        assert!(got.len() >= 3 * section);
        assert!(elapsed.as_secs_f64() < 5.0, "linting took {elapsed:?}");
    }
}
