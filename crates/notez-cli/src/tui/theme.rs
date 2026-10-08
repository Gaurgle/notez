//! Catppuccin Mocha palette and named styles.
//!
//! Carried over from notez-cli verbatim so the TUI looks identical. RGB
//! triplets only; do not introduce 256-color approximations here.

use notez_core::core::Scope;
use ratatui::style::{Color, Modifier, Style};

pub const RED: Color = Color::Rgb(243, 139, 168);
pub const PEACH: Color = Color::Rgb(250, 179, 135);
pub const GREEN: Color = Color::Rgb(166, 227, 161);
pub const YELLOW: Color = Color::Rgb(249, 226, 175);
pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
pub const MAUVE: Color = Color::Rgb(203, 166, 247);
pub const TEAL: Color = Color::Rgb(148, 226, 213);
pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
pub const OVERLAY: Color = Color::Rgb(108, 112, 134);
pub const SURFACE: Color = Color::Rgb(69, 71, 90);
pub const SURFACE0: Color = Color::Rgb(49, 50, 68);
pub const BASE: Color = Color::Rgb(30, 30, 46);
pub const TEXT: Color = Color::Rgb(205, 214, 244);
pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);

pub fn header() -> Style {
    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default().bg(SURFACE0)
}

pub fn normal() -> Style {
    Style::default().fg(TEXT)
}

pub fn dimmed() -> Style {
    Style::default().fg(OVERLAY)
}

pub fn dir_name() -> Style {
    Style::default().fg(SAPPHIRE)
}

pub fn file_name() -> Style {
    Style::default().fg(TEXT)
}

pub fn count() -> Style {
    Style::default().fg(OVERLAY)
}

pub fn border() -> Style {
    Style::default().fg(SURFACE)
}

/// The border of the focused pane in a split view.
pub fn border_focused() -> Style {
    Style::default().fg(LAVENDER)
}

/// The grip on the border between two panes: furniture at rest, lit like a
/// focused border while it is being dragged.
pub fn grip(dragging: bool) -> Style {
    if dragging { border_focused() } else { border() }
}

/// A pane's number in its title (the key that focuses it): lavender and
/// bold on the focused pane, otherwise in the readable overlay grey, since
/// an unfocused pane is exactly when its number is needed.
pub fn pane_number(focused: bool) -> Style {
    if focused {
        Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(OVERLAY)
    }
}

pub fn checked() -> Style {
    Style::default()
        .fg(OVERLAY)
        .add_modifier(Modifier::CROSSED_OUT)
}

pub fn unchecked() -> Style {
    Style::default().fg(SAPPHIRE)
}

pub fn command_line() -> Style {
    Style::default().fg(MAUVE)
}

/// A rendered markdown heading. Levels 1 and 2 get their own colour,
/// deeper levels are bold only. The colours stay clear of the scope badge
/// colours in [`scope_color`].
pub fn heading(level: u8) -> Style {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    match level {
        1 => bold.fg(MAUVE),
        2 => bold.fg(SAPPHIRE),
        _ => bold,
    }
}

/// Inline code and fenced or indented code blocks in rendered markdown.
pub fn code() -> Style {
    Style::default().fg(PEACH)
}

/// Block quote text and its `▎` bar in rendered markdown.
pub fn quote() -> Style {
    Style::default().fg(SUBTEXT).add_modifier(Modifier::ITALIC)
}

/// The `(url)` after a link's text, and image placeholders, in rendered
/// markdown.
pub fn link_url() -> Style {
    Style::default().fg(OVERLAY)
}

/// A horizontal rule in rendered markdown.
pub fn rule() -> Style {
    Style::default().fg(SURFACE)
}

// Syntax highlighting, one style per capture in `tui/highlight.rs`. They
// reuse the palette and stay clear of the scope badge colours in
// [`scope_color`] (lavender, teal, flamingo, green), so a highlighted token
// never reads as a scope.

/// Keywords (`fn`, `def`, `if`, `return`).
pub fn syntax_keyword() -> Style {
    Style::default().fg(MAUVE)
}

/// Function and method names, macros.
pub fn syntax_function() -> Style {
    Style::default().fg(SAPPHIRE).add_modifier(Modifier::BOLD)
}

/// Type names, built-in types.
pub fn syntax_type() -> Style {
    Style::default().fg(YELLOW)
}

/// String literals, escapes, JSON keys.
pub fn syntax_string() -> Style {
    Style::default().fg(PEACH)
}

/// Numeric literals.
pub fn syntax_number() -> Style {
    Style::default().fg(RED)
}

/// Comments, including doc comments.
pub fn syntax_comment() -> Style {
    Style::default().fg(OVERLAY).add_modifier(Modifier::ITALIC)
}

/// Constants and built-in constants (`true`, `None`, `NULL`).
pub fn syntax_constant() -> Style {
    Style::default().fg(RED)
}

/// Variables and parameters.
pub fn syntax_variable() -> Style {
    Style::default().fg(TEXT)
}

/// Operators.
pub fn syntax_operator() -> Style {
    Style::default().fg(SUBTEXT)
}

/// Brackets, delimiters and other punctuation.
pub fn syntax_punctuation() -> Style {
    Style::default().fg(OVERLAY)
}

/// Attributes and annotations (`#[derive]`, `@Override`).
pub fn syntax_attribute() -> Style {
    Style::default().fg(YELLOW).add_modifier(Modifier::ITALIC)
}

/// Fields, properties and TOML keys.
pub fn syntax_property() -> Style {
    Style::default().fg(SAPPHIRE)
}

/// Per-tag colors used by the todoz tag system. Five entries: important,
/// prio, longterm, idea, blocked.
pub const FLAG_COLORS: [Color; 5] = [
    Color::Rgb(243, 139, 168), // red, important
    Color::Rgb(250, 179, 135), // peach, prio
    Color::Rgb(249, 226, 175), // yellow, longterm
    Color::Rgb(116, 199, 236), // sapphire, idea
    Color::Rgb(203, 166, 247), // mauve, blocked
];

/// The colour of a scope's badge and scope word in the tree browser. Kept
/// apart from the tag colours in [`FLAG_COLORS`], so a badge never reads as
/// a tag dot.
pub fn scope_color(scope: Scope) -> Color {
    match scope {
        Scope::Personal => LAVENDER,
        Scope::Public => TEAL,
        Scope::Local => FLAMINGO,
        Scope::Global => GREEN,
    }
}

/// The badge on the todo board's rows in the tree browser (the `_todos`
/// store and every row under it, and every `TODO.md`), in place of the
/// scope icon: Nerd Font `nf-fa-tasks` (Font Awesome list-check), U+F0AE,
/// one column wide like the scope icons in `Scope::icon`.
pub const ICON_TODO: &str = "\u{f0ae}";

/// Dim a color by dividing each channel by 3, used for inactive tag dots.
/// Terminal DIM modifier is too inconsistent across emulators to rely on.
pub fn dim_color(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(r / 3, g / 3, b / 3),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_color_divides_rgb_channels() {
        let c = Color::Rgb(243, 139, 168);
        let dimmed = dim_color(c);
        assert_eq!(dimmed, Color::Rgb(81, 46, 56));
    }

    #[test]
    fn dim_color_passes_through_non_rgb() {
        let dimmed = dim_color(Color::Red);
        assert_eq!(dimmed, Color::Red);
    }

    #[test]
    fn five_flag_colors_defined() {
        assert_eq!(FLAG_COLORS.len(), 5);
    }

    #[test]
    fn each_scope_has_its_own_colour_apart_from_the_tag_colours() {
        let colors: Vec<Color> = [Scope::Personal, Scope::Public, Scope::Local, Scope::Global]
            .into_iter()
            .map(scope_color)
            .collect();
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b, "two scopes share {a:?}");
            }
            assert!(!FLAG_COLORS.contains(a), "{a:?} is a tag colour");
            assert_ne!(Some(*a), selected().bg, "{a:?} is the selection background");
            assert_ne!(Some(*a), dimmed().fg, "{a:?} is the dimmed colour");
        }
    }

    #[test]
    fn syntax_styles_avoid_scope_colours_and_keep_key_pairs_apart() {
        let syntax = [
            syntax_keyword(),
            syntax_function(),
            syntax_type(),
            syntax_string(),
            syntax_number(),
            syntax_comment(),
            syntax_constant(),
            syntax_variable(),
            syntax_operator(),
            syntax_punctuation(),
            syntax_attribute(),
            syntax_property(),
        ];
        let scopes = [Scope::Personal, Scope::Public, Scope::Local, Scope::Global];
        for style in syntax {
            for scope in scopes {
                assert_ne!(
                    style.fg,
                    Some(scope_color(scope)),
                    "{style:?} uses a scope colour"
                );
            }
        }
        assert_ne!(syntax_string().fg, syntax_number().fg);
        assert_ne!(syntax_keyword().fg, syntax_type().fg);
        let comment = syntax_comment();
        assert!(
            comment.fg == dimmed().fg || comment.add_modifier.contains(Modifier::ITALIC),
            "comments are dim or italic"
        );
    }
}
