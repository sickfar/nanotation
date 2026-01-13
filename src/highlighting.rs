use syntect::parsing::SyntaxSet;
use syntect::highlighting::{Theme, ThemeSettings, ThemeItem, Color, Style, FontStyle, ScopeSelectors};
use crossterm::style::{Color as CrosstermColor};

pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
}

impl SyntaxHighlighter {
    pub fn new(dark_mode: bool) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme = if dark_mode {
            create_zenbones_dark()
        } else {
            create_zenbones_light()
        };

        SyntaxHighlighter {
            syntax_set,
            theme,
        }
    }

    pub fn highlight<'a>(&self, line: &'a str, extension: &str) -> Vec<(Style, &'a str)> {
        let syntax = self.syntax_set.find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let mut h = syntect::easy::HighlightLines::new(syntax, &self.theme);
        h.highlight_line(line, &self.syntax_set).unwrap_or_else(|_| vec![(Style::default(), line)])
    }
}

fn create_zenbones_dark() -> Theme {
    // Alabaster Dark (Approximation based on strict adherence)
    // Using official palette derived from Alabaster Light where possible, adapted for Dark BG.
    let bg = Color { r: 14, g: 20, b: 21, a: 255 };      // #0e1415
    let fg = Color { r: 206, g: 206, b: 206, a: 255 };   // #cecece

    // Strict Alabaster Palette (from Alabaster.sublime-color-scheme)
    // The Light theme uses these values. We will use them here too as "Strict" adherence implies usage of these hexes.
    let comment = Color { r: 170, g: 55, b: 49, a: 255 };   // #AA3731 (Red)
    let string = Color { r: 68, g: 140, b: 39, a: 255 };    // #448C27 (Green)
    let constant = Color { r: 122, g: 62, b: 157, a: 255 }; // #7A3E9D (Magenta)
    let definition = Color { r: 120, g: 160, b: 255, a: 255 };// Lighter blue for better readability on dark/grey backgrounds
    let _regex = Color { r: 122, g: 62, b: 157, a: 255 };    // #7A3E9D (Magenta)

    build_alabaster_theme("Alabaster Dark", bg, fg, comment, string, constant, definition, _regex)
}

fn create_zenbones_light() -> Theme {
    // Alabaster Light (Exact from official repo)
    let bg = Color { r: 247, g: 247, b: 247, a: 255 };   // #F7F7F7
    let fg = Color { r: 0, g: 0, b: 0, a: 255 };         // #000000
    let comment = Color { r: 170, g: 55, b: 49, a: 255 };   // #AA3731 (Red)
    let string = Color { r: 68, g: 140, b: 39, a: 255 };    // #448C27 (Green)
    let constant = Color { r: 122, g: 62, b: 157, a: 255 }; // #7A3E9D (Magenta)
    let definition = Color { r: 50, g: 92, b: 192, a: 255 };// #325CC0 (Blue)
    let regex = Color { r: 122, g: 62, b: 157, a: 255 };    // #7A3E9D (Magenta)

    build_alabaster_theme("Alabaster Light", bg, fg, comment, string, constant, definition, regex)
}

fn build_alabaster_theme(
    name: &str, 
    bg: Color, 
    fg: Color, 
    comment: Color, 
    string: Color, 
    constant: Color,
    definition: Color,
    _regex: Color
) -> Theme {
    let mut theme = Theme {
        name: Some(name.to_string()),
        author: Some("Alabaster Port".to_string()),
        settings: ThemeSettings::default(),
        scopes: Vec::new(),
    };

    theme.settings.background = Some(bg);
    theme.settings.foreground = Some(fg);
    theme.settings.caret = Some(fg);
    theme.settings.selection = Some(Color { r: 61, g: 64, b: 66, a: 255 }); 

    // Comments
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("comment").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(comment),
            background: None,
            font_style: Some(FontStyle::default()), // Alabaster doesn't use italics much
        },
    });

    // Strings
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("string").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(string),
            background: None,
            font_style: Some(FontStyle::default()), 
        },
    });

    // Constants (Numbers, Booleans)
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("constant.numeric, constant.language, constant.character").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(constant),
            background: None,
            font_style: Some(FontStyle::default()),
        },
    });

    // Definitions (Functions, Classes)
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("entity.name, entity.name.function, entity.name.type").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(definition),
            background: None,
            font_style: Some(FontStyle::default()),
        },
    });
    
    // Keywords - Alabaster explicitly does NOT highlight standard keywords.
    // They are usually "least important".
    // We will keep them FG (default) or make them plain.
    // However, syntect might default them to something else if we load defaults? NO we create theme.
    // So by omission, they become default FG. 
    // BUT we need to clear any inherited styles? No, we are building fresh.
    
    // Punctuation / Brackets
    // Alabaster Light uses Grey #777. Dark uses Grey #777 probably (or similar).
    let grey = Color { r: 119, g: 119, b: 119, a: 255 };
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("punctuation").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(grey),
            background: None,
            font_style: Some(FontStyle::default()),
        },
    });

    // Markdown Specifics to be nice
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("markup.heading").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(fg),
            background: None,
            font_style: Some(FontStyle::BOLD),
        },
    });
    
    theme.scopes.push(ThemeItem {
        scope: ScopeSelectors::from_str("markup.bold").unwrap(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(fg),
            background: None,
            font_style: Some(FontStyle::BOLD),
        },
    });

    theme
}

use std::str::FromStr;

// Helper implementation to convert syntect style to something usable by crossterm
// We can't implement From/Into because types are foreign.
// We will just expose a conversion function.

pub fn to_crossterm_color(c: Color) -> CrosstermColor {
    CrosstermColor::Rgb { r: c.r, g: c.g, b: c.b }
}

