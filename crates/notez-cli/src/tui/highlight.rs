//! Syntax highlighting for the preview pane, built on tree-sitter.
//!
//! [`highlight_lines`] turns a source file or a fenced code block into
//! styled lines. Each grammar's bundled highlights query (and injections
//! query where the crate ships one) is compiled once per process, on first
//! use, and shared read-only across threads. A grammar whose query fails to
//! compile, or whose ABI the runtime rejects, is disabled: its text comes
//! back unstyled and [`language_available`] reports `false`, so the caller
//! can say so instead of the TUI panicking.
//!
//! Kotlin comes from `tree-sitter-kotlin-ng` (`tree-sitter-kotlin 0.3.8`
//! requires `tree-sitter <0.23`). That crate ships no queries, so its
//! highlights query lives here as [`KOTLIN_HIGHLIGHTS`].

use std::sync::OnceLock;

use ratatui::style::Style;
use ratatui::text::Span;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use super::theme;

/// Files larger than this many bytes are shown unhighlighted, so selecting
/// one stays fast: parsing is synchronous and proportional to the size.
pub const MAX_HIGHLIGHT_BYTES: u64 = 1024 * 1024;

/// A language with a bundled grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    Kotlin,
    Java,
    C,
    Toml,
    Json,
    Bash,
    Markdown,
}

impl Language {
    /// Every shipped language.
    pub const ALL: [Language; 9] = [
        Language::Rust,
        Language::Python,
        Language::Kotlin,
        Language::Java,
        Language::C,
        Language::Toml,
        Language::Json,
        Language::Bash,
        Language::Markdown,
    ];

    /// The language for a file extension (without the dot), case-insensitive.
    pub fn from_extension(extension: &str) -> Option<Language> {
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "kt" | "kts" => Some(Language::Kotlin),
            "java" => Some(Language::Java),
            "c" | "h" => Some(Language::C),
            "toml" => Some(Language::Toml),
            "json" => Some(Language::Json),
            "sh" | "bash" => Some(Language::Bash),
            "md" => Some(Language::Markdown),
            _ => None,
        }
    }

    /// The language for a fenced code block's info string, case-insensitive.
    /// Only the first word counts: attributes after a comma or whitespace
    /// (`rust,ignore`, `python title="x"`) are ignored.
    pub fn from_fence_tag(tag: &str) -> Option<Language> {
        let word = tag
            .trim()
            .split(|c: char| c == ',' || c.is_whitespace())
            .next()
            .unwrap_or("");
        match word.to_ascii_lowercase().as_str() {
            "rust" | "rs" => Some(Language::Rust),
            "python" | "py" => Some(Language::Python),
            "kotlin" | "kt" => Some(Language::Kotlin),
            "java" => Some(Language::Java),
            "c" => Some(Language::C),
            "toml" => Some(Language::Toml),
            "json" => Some(Language::Json),
            "bash" | "sh" | "shell" => Some(Language::Bash),
            "markdown" | "md" => Some(Language::Markdown),
            _ => None,
        }
    }

    /// The lowercase display name, for the footer (`rust`, `python`, ...).
    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::Kotlin => "kotlin",
            Language::Java => "java",
            Language::C => "c",
            Language::Toml => "toml",
            Language::Json => "json",
            Language::Bash => "bash",
            Language::Markdown => "markdown",
        }
    }

    fn slot(self) -> usize {
        match self {
            Language::Rust => 0,
            Language::Python => 1,
            Language::Java => 2,
            Language::C => 3,
            Language::Toml => 4,
            Language::Json => 5,
            Language::Bash => 6,
            Language::Markdown => 7,
            Language::Kotlin => 8,
        }
    }
}

/// The highlight categories the preview styles. A query capture maps to
/// the category named by one of its dot-separated segments
/// (`keyword.control` is [`Capture::Keyword`], `string.special.key` is
/// [`Capture::String`]); in the grammars shipped here that is always the
/// first segment. Captures matching none of them render plain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Constant,
    Variable,
    Operator,
    Punctuation,
    Attribute,
    Property,
}

impl Capture {
    /// The fixed capture list, in the order handed to tree-sitter. A
    /// `Highlight(i)` from any configuration indexes this array.
    const ALL: [Capture; 12] = [
        Capture::Keyword,
        Capture::Function,
        Capture::Type,
        Capture::String,
        Capture::Number,
        Capture::Comment,
        Capture::Constant,
        Capture::Variable,
        Capture::Operator,
        Capture::Punctuation,
        Capture::Attribute,
        Capture::Property,
    ];

    fn name(self) -> &'static str {
        match self {
            Capture::Keyword => "keyword",
            Capture::Function => "function",
            Capture::Type => "type",
            Capture::String => "string",
            Capture::Number => "number",
            Capture::Comment => "comment",
            Capture::Constant => "constant",
            Capture::Variable => "variable",
            Capture::Operator => "operator",
            Capture::Punctuation => "punctuation",
            Capture::Attribute => "attribute",
            Capture::Property => "property",
        }
    }

    /// The theme style for this capture.
    pub fn style(self) -> Style {
        match self {
            Capture::Keyword => theme::syntax_keyword(),
            Capture::Function => theme::syntax_function(),
            Capture::Type => theme::syntax_type(),
            Capture::String => theme::syntax_string(),
            Capture::Number => theme::syntax_number(),
            Capture::Comment => theme::syntax_comment(),
            Capture::Constant => theme::syntax_constant(),
            Capture::Variable => theme::syntax_variable(),
            Capture::Operator => theme::syntax_operator(),
            Capture::Punctuation => theme::syntax_punctuation(),
            Capture::Attribute => theme::syntax_attribute(),
            Capture::Property => theme::syntax_property(),
        }
    }
}

/// Whether `language`'s grammar loaded and its queries compiled. When
/// `false`, [`highlight_lines`] returns that language's text unstyled.
pub fn language_available(language: Language) -> bool {
    grammar(language.slot()).is_some()
}

/// Parse `source` with `language`'s grammar into a syntax tree, for
/// callers that inspect the tree itself (the preview's lint pass walks it
/// for `ERROR` and `MISSING` nodes). Only the language's own grammar runs:
/// injections (markdown inline, fenced code, Rust macro bodies) are not
/// parsed. `None` when the grammar is unavailable (see
/// [`language_available`]) or the parser gives up.
pub fn parse(source: &str, language: Language) -> Option<tree_sitter::Tree> {
    let config = grammar(language.slot())?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&config.language).ok()?;
    parser.parse(source, None)
}

/// Highlight `source` as `language`, one entry per source line (as
/// `str::lines` splits them: a trailing newline adds no line, `\r\n` loses
/// its `\r`). The spans of a line concatenate to exactly that line's text;
/// text no capture applies to has the default (unset) style, so a caller
/// can patch it with a base style. An empty line is an empty `Vec`.
///
/// Parsing is synchronous and proportional to the source size. If the
/// grammar is unavailable or highlighting fails, the lines come back
/// unstyled rather than panicking.
pub fn highlight_lines(source: &str, language: Language) -> Vec<Vec<Span<'static>>> {
    highlight_segments(source, grammar(language.slot()))
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|(text, capture)| match capture {
                    Some(capture) => Span::styled(text, capture.style()),
                    None => Span::raw(text),
                })
                .collect()
        })
        .collect()
}

/// A line as `(text, capture)` segments; adjacent segments differ in capture.
type Segments = Vec<(String, Option<Capture>)>;

/// Slot of the markdown inline grammar, which only exists as an injection
/// target inside markdown and has no [`Language`] of its own.
const MARKDOWN_INLINE_SLOT: usize = 9;

/// Highlights for `tree-sitter-kotlin-ng`, which ships no queries. Written
/// against its node types for the fixed capture list. Patterns earlier in
/// the query win over later ones for the same node, so the specific
/// identifier patterns precede the general type pattern.
const KOTLIN_HIGHLIGHTS: &str = r#"
[(line_comment) (block_comment) (shebang)] @comment

[(string_literal) (multiline_string_literal) (character_literal)] @string
(escape_sequence) @string
(interpolation ["$" "${" "}"] @punctuation)

[(number_literal) (float_literal)] @number

((identifier) @constant
  (#any-of? @constant "true" "false" "null"))

(annotation "@" @attribute)
(annotation (user_type (identifier) @attribute))
(annotation (constructor_invocation (user_type (identifier) @attribute)))

(function_declaration name: (identifier) @function)
(call_expression (identifier) @function)
(call_expression (navigation_expression (identifier) @function .))

(class_declaration name: (identifier) @type)
(object_declaration name: (identifier) @type)
(user_type (identifier) @type)

(navigation_expression (identifier) @property .)

[
  "abstract" "actual" "annotation" "as" "as?" "by" "catch" "class"
  "companion" "const" "constructor" "crossinline" "data" "do" "else"
  "enum" "expect" "external" "final" "finally" "for" "fun" "get" "if"
  "import" "in" "!in" "infix" "init" "inline" "inner" "interface"
  "internal" "is" "!is" "lateinit" "noinline" "object" "open" "operator"
  "out" "override" "package" "private" "protected" "public" "return"
  "return@" "sealed" "set" "super" "super@" "suspend" "tailrec" "this"
  "this@" "throw" "try" "typealias" "val" "value" "var" "vararg" "when"
  "where" "while"
] @keyword
(reification_modifier) @keyword

[
  "!" "!!" "!=" "!==" "%" "%=" "&&" "*" "*=" "+" "++" "+=" "-" "--" "-="
  "->" ".." "..<" "/" "/=" "<" "<=" "=" "==" "===" ">" ">=" "?:" "||"
] @operator

["(" ")" "[" "]" "{" "}" "," "." "?." ";" ":" "::"] @punctuation
"#;

/// Injections for the markdown block grammar, in place of the bundled
/// `tree_sitter_md::INJECTION_QUERY_BLOCK`. The block grammar exposes the
/// punctuation inside a code fence (`(`, `"`, `;` ...) as anonymous
/// children of `code_fence_content`, and without `injection.include-children`
/// tree-sitter-highlight cuts those bytes out of the injected range, so the
/// fenced code parses as fragments. The `html` and `yaml` injections are left
/// out because no grammar for them ships.
const MARKDOWN_BLOCK_INJECTIONS: &str = r#"
(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content
  (#set! injection.include-children))

((plus_metadata) @injection.content
  (#set! injection.language "toml"))

((inline) @injection.content
  (#set! injection.language "markdown_inline")
  (#set! injection.include-children))
"#;

/// One lazily built configuration per grammar slot. `None` inside the
/// `OnceLock` records a grammar that failed to load, so the failure is not
/// retried on every call.
static GRAMMARS: [OnceLock<Option<HighlightConfiguration>>; 10] = [const { OnceLock::new() }; 10];

fn grammar(slot: usize) -> Option<&'static HighlightConfiguration> {
    GRAMMARS[slot].get_or_init(|| build_slot(slot)).as_ref()
}

fn build_slot(slot: usize) -> Option<HighlightConfiguration> {
    let (language, name, highlights, injections) = match slot {
        0 => (
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
        ),
        1 => (
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
        ),
        2 => (
            tree_sitter_java::LANGUAGE.into(),
            "java",
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
        ),
        3 => (
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
        ),
        4 => (
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
        ),
        5 => (
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
        ),
        6 => (
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
        ),
        7 => (
            tree_sitter_md::LANGUAGE.into(),
            "markdown",
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            MARKDOWN_BLOCK_INJECTIONS,
        ),
        8 => (
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            "kotlin",
            KOTLIN_HIGHLIGHTS,
            "",
        ),
        MARKDOWN_INLINE_SLOT => (
            tree_sitter_md::INLINE_LANGUAGE.into(),
            "markdown_inline",
            tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
            tree_sitter_md::INJECTION_QUERY_INLINE,
        ),
        _ => return None,
    };
    build_config(language, name, highlights, injections)
}

/// Load a grammar and compile its queries, or `None` if the runtime rejects
/// the grammar's ABI or a query fails to compile. None of the shipped
/// grammars has a locals query.
fn build_config(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
) -> Option<HighlightConfiguration> {
    if tree_sitter::Parser::new().set_language(&language).is_err() {
        return None;
    }
    let mut config =
        HighlightConfiguration::new(language, name, highlights, injections, "").ok()?;
    let recognized: Vec<&str> = Capture::ALL.iter().map(|capture| capture.name()).collect();
    config.configure(&recognized);
    Some(config)
}

/// Resolve an injection's language name: `markdown_inline` from the
/// markdown block grammar, a fence's info string, or `rust` for Rust macro
/// bodies. Unknown languages (`yaml`, `html`) stay unhighlighted.
fn injected_grammar(name: &str) -> Option<&'static HighlightConfiguration> {
    if name == "markdown_inline" {
        return grammar(MARKDOWN_INLINE_SLOT);
    }
    Language::from_fence_tag(name).and_then(|language| grammar(language.slot()))
}

fn highlight_segments(source: &str, config: Option<&HighlightConfiguration>) -> Vec<Segments> {
    config
        .and_then(|config| try_highlight(source, config))
        .unwrap_or_else(|| plain_segments(source))
}

fn try_highlight(source: &str, config: &HighlightConfiguration) -> Option<Vec<Segments>> {
    let bytes = source.as_bytes();
    let mut highlighter = Highlighter::new();
    // The closure lets the injected configurations' `'static` lifetime
    // shorten to `config`'s; passing `injected_grammar` itself would demand
    // a `'static` `config` and does not compile.
    #[allow(clippy::redundant_closure)]
    let events = highlighter
        .highlight(config, bytes, None, None, |name| injected_grammar(name))
        .ok()?;
    let mut lines = LineBuilder::default();
    let mut stack: Vec<Option<Capture>> = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(highlight) => {
                stack.push(Capture::ALL.get(highlight.0).copied());
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let text = String::from_utf8_lossy(bytes.get(start..end)?);
                lines.push(&text, stack.last().copied().flatten());
            }
        }
    }
    Some(lines.finish())
}

fn plain_segments(source: &str) -> Vec<Segments> {
    source
        .lines()
        .map(|line| {
            if line.is_empty() {
                Vec::new()
            } else {
                vec![(line.to_string(), None)]
            }
        })
        .collect()
}

/// Splits a stream of captured source fragments into lines, matching
/// `str::lines`.
#[derive(Default)]
struct LineBuilder {
    lines: Vec<Segments>,
    current: Segments,
}

impl LineBuilder {
    fn push(&mut self, text: &str, capture: Option<Capture>) {
        let mut parts = text.split('\n');
        if let Some(first) = parts.next() {
            self.append(first, capture);
        }
        for part in parts {
            self.end_line();
            self.append(part, capture);
        }
    }

    fn append(&mut self, text: &str, capture: Option<Capture>) {
        if text.is_empty() {
            return;
        }
        match self.current.last_mut() {
            Some((last, last_capture)) if *last_capture == capture => last.push_str(text),
            _ => self.current.push((text.to_string(), capture)),
        }
    }

    fn end_line(&mut self) {
        let mut line = std::mem::take(&mut self.current);
        if let Some((last, _)) = line.last_mut() {
            if last.ends_with('\r') {
                last.pop();
                if last.is_empty() {
                    line.pop();
                }
            }
        }
        self.lines.push(line);
    }

    fn finish(mut self) -> Vec<Segments> {
        if !self.current.is_empty() {
            self.lines.push(self.current);
        }
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn segments(source: &str, language: Language) -> Vec<Segments> {
        highlight_segments(source, grammar(language.slot()))
    }

    fn captures(lines: &[Segments]) -> Vec<Capture> {
        lines
            .iter()
            .flatten()
            .filter_map(|(_, capture)| *capture)
            .collect()
    }

    fn line_texts(lines: &[Vec<Span<'static>>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.iter().map(|span| span.content.as_ref()).collect())
            .collect()
    }

    /// Highlights `source`, checks the lines reproduce the source text, and
    /// that every capture in `expected` occurs.
    fn assert_highlights(language: Language, source: &str, expected: &[Capture]) {
        let lines = highlight_lines(source, language);
        assert_eq!(
            lines.len(),
            source.lines().count(),
            "line count for {language:?}"
        );
        assert_eq!(line_texts(&lines), source.lines().collect::<Vec<_>>());
        let found = captures(&segments(source, language));
        for capture in expected {
            assert!(
                found.contains(capture),
                "{language:?}: no {capture:?} in {found:?}"
            );
        }
    }

    #[test]
    fn rust_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::Rust,
            "fn main() {\n    let s = \"hi\"; // note\n}\n",
            &[
                Capture::Keyword,
                Capture::String,
                Capture::Comment,
                Capture::Function,
            ],
        );
    }

    #[test]
    fn python_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::Python,
            "def f():\n    return \"x\" + 1\n",
            &[Capture::Keyword, Capture::String, Capture::Number],
        );
    }

    #[test]
    fn kotlin_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::Kotlin,
            "@Suppress(\"x\")\nfun greet(name: String): Int {\n    val s = \"hi $name\" // note\n    println(s)\n    return 1\n}\n",
            &[
                Capture::Keyword,
                Capture::String,
                Capture::Comment,
                Capture::Function,
                Capture::Type,
                Capture::Number,
                Capture::Attribute,
            ],
        );
        let lines = segments("val ok = true\n", Language::Kotlin);
        assert!(
            lines[0].contains(&("val".to_string(), Some(Capture::Keyword))),
            "{lines:?}"
        );
        assert!(
            lines[0].contains(&("true".to_string(), Some(Capture::Constant))),
            "{lines:?}"
        );
    }

    #[test]
    fn java_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::Java,
            "class A {\n    String s = \"x\";\n}\n",
            &[Capture::Keyword, Capture::String, Capture::Type],
        );
    }

    #[test]
    fn c_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::C,
            "int main(void) {\n    return \"x\"[0];\n}\n",
            &[Capture::Keyword, Capture::String, Capture::Number],
        );
    }

    #[test]
    fn toml_snippet_yields_property_and_string() {
        assert_highlights(
            Language::Toml,
            "[package]\nname = \"x\"\nversion = 1\n",
            &[Capture::Property, Capture::String, Capture::Number],
        );
    }

    #[test]
    fn json_snippet_yields_number_and_string() {
        assert_highlights(
            Language::Json,
            "{\n  \"a\": 1,\n  \"b\": \"x\"\n}\n",
            &[Capture::Number, Capture::String],
        );
    }

    #[test]
    fn bash_snippet_yields_keyword_and_string() {
        assert_highlights(
            Language::Bash,
            "if [ -n \"$x\" ]; then\n  echo \"hi\"\nfi\n",
            &[Capture::Keyword, Capture::String],
        );
    }

    /// The markdown grammars capture no keywords or strings of their own;
    /// a fenced block's code is highlighted through the injection.
    #[test]
    fn markdown_snippet_highlights_fenced_code_through_injection() {
        assert_highlights(
            Language::Markdown,
            "# Title\n\nSome *text*.\n\n```rust\nlet s = \"x\";\n```\n",
            &[Capture::Punctuation, Capture::Keyword, Capture::String],
        );
    }

    #[test]
    fn markdown_fenced_code_keeps_its_punctuation_and_inline_markup_is_injected() {
        let source = "Some *text*.\n\n```python\ndef f(): return \"x\"\n```\n";
        let lines = segments(source, Language::Markdown);
        assert_eq!(lines[0][1], ("*".to_string(), Some(Capture::Punctuation)));
        let code = &lines[3];
        assert!(
            code.contains(&("def".to_string(), Some(Capture::Keyword))),
            "{code:?}"
        );
        assert!(
            code.contains(&("return".to_string(), Some(Capture::Keyword))),
            "{code:?}"
        );
        assert!(
            code.contains(&("\"x\"".to_string(), Some(Capture::String))),
            "{code:?}"
        );
    }

    #[test]
    fn markdown_in_quotes_and_lists_keeps_every_line() {
        assert_highlights(
            Language::Markdown,
            "> a *b*\n> c `d`\n\n- item\n  ```rust\n  let x = \"y\";\n  ```\n+++\n",
            &[Capture::Punctuation, Capture::Keyword],
        );
    }

    #[test]
    fn from_extension_maps_the_shipped_extensions() {
        let table = [
            ("rs", Language::Rust),
            ("py", Language::Python),
            ("kt", Language::Kotlin),
            ("kts", Language::Kotlin),
            ("KT", Language::Kotlin),
            ("java", Language::Java),
            ("c", Language::C),
            ("h", Language::C),
            ("toml", Language::Toml),
            ("json", Language::Json),
            ("sh", Language::Bash),
            ("bash", Language::Bash),
            ("md", Language::Markdown),
            ("RS", Language::Rust),
        ];
        for (extension, language) in table {
            assert_eq!(
                Language::from_extension(extension),
                Some(language),
                "{extension}"
            );
        }
        for unknown in ["", "txt", "kotlin", "yaml", ".rs"] {
            assert_eq!(Language::from_extension(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn from_fence_tag_maps_tags_and_ignores_attributes() {
        let table = [
            ("rust", Language::Rust),
            ("rs", Language::Rust),
            ("RS", Language::Rust),
            ("Rust", Language::Rust),
            ("rust,ignore", Language::Rust),
            ("rust no_run", Language::Rust),
            ("  rust  ", Language::Rust),
            ("python", Language::Python),
            ("py", Language::Python),
            ("kotlin", Language::Kotlin),
            ("kt", Language::Kotlin),
            ("Kotlin", Language::Kotlin),
            ("java", Language::Java),
            ("c", Language::C),
            ("toml", Language::Toml),
            ("json", Language::Json),
            ("bash", Language::Bash),
            ("sh", Language::Bash),
            ("shell", Language::Bash),
            ("markdown", Language::Markdown),
            ("md", Language::Markdown),
        ];
        for (tag, language) in table {
            assert_eq!(Language::from_fence_tag(tag), Some(language), "{tag}");
        }
        for unknown in ["", ",rust", "kts", "text", "rustx", "h"] {
            assert_eq!(Language::from_fence_tag(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn names_are_lowercase_and_map_back_through_the_fence_tag() {
        for language in Language::ALL {
            let name = language.name();
            assert_eq!(name, name.to_ascii_lowercase());
            assert_eq!(Language::from_fence_tag(name), Some(language));
        }
    }

    #[test]
    fn language_available_for_every_shipped_language() {
        for language in Language::ALL {
            assert!(language_available(language), "{language:?} failed to load");
        }
        assert!(
            grammar(MARKDOWN_INLINE_SLOT).is_some(),
            "markdown inline failed to load"
        );
    }

    #[test]
    fn configurations_are_built_once() {
        let first = grammar(Language::Rust.slot()).unwrap() as *const _;
        highlight_lines("fn a() {}", Language::Rust);
        let second = grammar(Language::Rust.slot()).unwrap() as *const _;
        assert_eq!(first, second);
    }

    #[test]
    fn a_query_that_fails_to_compile_disables_the_grammar_without_panicking() {
        let broken = build_config(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            "(no_such_node) @keyword",
            "",
        );
        assert!(broken.is_none());
        let lines = highlight_segments("fn a() {}\nlet b = 1;\n", broken.as_ref());
        assert_eq!(
            lines,
            vec![
                vec![("fn a() {}".to_string(), None)],
                vec![("let b = 1;".to_string(), None)],
            ]
        );
    }

    #[test]
    fn invalid_syntax_does_not_panic_and_keeps_every_line() {
        let source = "fn ( {{ \"unterminated\n}}} let = = ;\n@@@ <<< ]]]\n";
        for language in Language::ALL {
            let lines = highlight_lines(source, language);
            assert_eq!(lines.len(), 3, "{language:?}");
            assert_eq!(
                line_texts(&lines),
                source.lines().collect::<Vec<_>>(),
                "{language:?}"
            );
        }
    }

    #[test]
    fn empty_source_yields_no_lines() {
        for language in Language::ALL {
            assert!(highlight_lines("", language).is_empty(), "{language:?}");
        }
    }

    #[test]
    fn line_count_follows_str_lines() {
        for source in ["\n", "a", "a\n", "a\n\n", "\n\nb", "a\nb"] {
            let lines = highlight_lines(source, Language::Rust);
            assert_eq!(lines.len(), source.lines().count(), "{source:?}");
            assert_eq!(
                line_texts(&lines),
                source.lines().collect::<Vec<_>>(),
                "{source:?}"
            );
        }
    }

    #[test]
    fn crlf_line_endings_drop_the_carriage_return() {
        let source = "fn main() {\r\n    let s = \"x\";\r\n\r\n}\r\n";
        let lines = highlight_lines(source, Language::Rust);
        assert_eq!(
            line_texts(&lines),
            vec!["fn main() {", "    let s = \"x\";", "", "}"]
        );
        assert!(lines[2].is_empty());
        assert!(captures(&segments(source, Language::Rust)).contains(&Capture::Keyword));
    }

    #[test]
    fn unicode_text_is_preserved() {
        let source = "let s = \"héllo ✓\"; // ünïcode\n";
        let lines = highlight_lines(source, Language::Rust);
        assert_eq!(line_texts(&lines), vec!["let s = \"héllo ✓\"; // ünïcode"]);
    }

    #[test]
    fn uncaptured_text_is_unstyled_and_captured_text_uses_the_theme() {
        let lines = highlight_lines("let x = \"s\";", Language::Rust);
        let spans = &lines[0];
        let keyword = spans.iter().find(|span| span.content == "let").unwrap();
        assert_eq!(keyword.style, theme::syntax_keyword());
        let string = spans.iter().find(|span| span.content == "\"s\"").unwrap();
        assert_eq!(string.style, theme::syntax_string());
        let plain = spans.iter().find(|span| span.content == " x = ").unwrap();
        assert_eq!(plain.style, Style::default());
    }

    #[test]
    fn a_200_kb_rust_file_highlights_in_bounded_time() {
        let block = "/// Doc comment.\n#[derive(Debug)]\npub struct Item { id: u32, name: String }\n\nfn build(n: u32) -> Vec<Item> {\n    (0..n).map(|id| Item { id, name: format!(\"item {id}\") }).collect()\n}\n\n";
        let mut source = String::new();
        while source.len() < 200 * 1024 {
            source.push_str(block);
        }
        let started = Instant::now();
        let lines = highlight_lines(&source, Language::Rust);
        let elapsed = started.elapsed();
        eprintln!(
            "200 KB rust: {} bytes, {} lines, {elapsed:?}",
            source.len(),
            lines.len()
        );
        assert_eq!(lines.len(), source.lines().count());
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
    }
}
