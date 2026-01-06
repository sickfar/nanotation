use crossterm::style::Color;

#[derive(Clone, Copy, PartialEq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone, Copy)]
pub struct ColorScheme {
    pub bg: Color,
    pub fg: Color,
    pub selected_bg: Color,
    pub annotated_bg: Color,
    pub annotated_selected_bg: Color,
    pub annotation_window_bg: Color,
    pub annotation_window_fg: Color,
    pub status_bg: Color,
    pub status_fg: Color,
}

impl Theme {
    pub fn colors(&self) -> ColorScheme {
        match self {
            Theme::Dark => ColorScheme {
                bg: Color::Black,
                fg: Color::White,
                selected_bg: Color::DarkGrey,
                annotated_bg: Color::Rgb { r: 40, g: 60, b: 80 },
                annotated_selected_bg: Color::Rgb { r: 60, g: 90, b: 120 },
                annotation_window_bg: Color::Rgb { r: 50, g: 70, b: 90 },
                annotation_window_fg: Color::Yellow,
                status_bg: Color::DarkGrey,
                status_fg: Color::White,
            },
            Theme::Light => ColorScheme {
                bg: Color::White,
                fg: Color::Black,
                selected_bg: Color::Rgb { r: 220, g: 220, b: 220 },
                annotated_bg: Color::Rgb { r: 200, g: 220, b: 240 },
                annotated_selected_bg: Color::Rgb { r: 170, g: 200, b: 230 },
                annotation_window_bg: Color::Rgb { r: 180, g: 210, b: 240 },
                annotation_window_fg: Color::Rgb { r: 50, g: 50, b: 150 },
                status_bg: Color::Rgb { r: 100, g: 100, b: 100 },
                status_fg: Color::White,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_colors() {
        let theme = Theme::Dark;
        let colors = theme.colors();
        assert_eq!(colors.bg, Color::Black);
        assert_eq!(colors.fg, Color::White);
        assert_eq!(colors.status_bg, Color::DarkGrey);
    }

    #[test]
    fn test_light_theme_colors() {
        let theme = Theme::Light;
        let colors = theme.colors();
        assert_eq!(colors.bg, Color::White);
        assert_eq!(colors.fg, Color::Black);
    }
}
