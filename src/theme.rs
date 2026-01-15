use crossterm::style::Color;

#[derive(Clone, Copy, PartialEq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone, Copy)]
pub struct ColorScheme {
    pub bg: Color,
    pub _fg: Color,
    pub selected_bg: Color,
    pub annotated_bg: Color,
    pub annotated_selected_bg: Color,
    pub annotation_window_bg: Color,
    pub annotation_window_fg: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub line_number_fg: Color,
    // Diff mode colors
    pub diff_added_bg: Color,
    pub diff_removed_bg: Color,
    pub diff_added_word_bg: Color,
    pub diff_removed_word_bg: Color,
    // Diff cursor colors (when cursor is on a diff line)
    pub diff_added_selected_bg: Color,
    pub diff_removed_selected_bg: Color,
    // Diff indicator in status bar
    pub diff_indicator_bg: Color,
    pub diff_indicator_fg: Color,
    // File tree colors
    pub tree_bg: Color,
    pub tree_fg: Color,
    pub tree_folder_fg: Color,
    pub tree_selected_bg: Color,
    pub tree_selected_fg: Color,
    pub tree_current_file_bg: Color,
    pub tree_git_added_fg: Color,
    pub tree_git_removed_fg: Color,
    pub tree_separator_fg: Color,
    pub tree_header_fg: Color,
    pub tree_empty_fg: Color,
    pub error_fg: Color,
    pub error_bg: Color,
}

impl Theme {
    pub fn colors(&self) -> ColorScheme {
        match self {
            Theme::Dark => ColorScheme {
                bg: Color::Black,
                _fg: Color::White,
                selected_bg: Color::Rgb { r: 40, g: 40, b: 40 },
                annotated_bg: Color::Rgb { r: 40, g: 60, b: 80 },
                annotated_selected_bg: Color::Rgb {
                    r: 60,
                    g: 90,
                    b: 120,
                },
                annotation_window_bg: Color::Rgb {
                    r: 50,
                    g: 70,
                    b: 90,
                },
                annotation_window_fg: Color::Yellow,
                status_bg: Color::DarkGrey,
                status_fg: Color::White,
                line_number_fg: Color::Rgb {
                    r: 120,
                    g: 120,
                    b: 120,
                },
                // Diff colors - pale green/red backgrounds
                diff_added_bg: Color::Rgb {
                    r: 30,
                    g: 50,
                    b: 30,
                },
                diff_removed_bg: Color::Rgb {
                    r: 50,
                    g: 30,
                    b: 30,
                },
                // Word-level highlights - brighter
                diff_added_word_bg: Color::Rgb {
                    r: 50,
                    g: 100,
                    b: 50,
                },
                diff_removed_word_bg: Color::Rgb {
                    r: 100,
                    g: 50,
                    b: 50,
                },
                // Cursor on diff line - blend added/removed with selection
                diff_added_selected_bg: Color::Rgb {
                    r: 50,
                    g: 70,
                    b: 50,
                },
                diff_removed_selected_bg: Color::Rgb {
                    r: 70,
                    g: 50,
                    b: 50,
                },
                // Orange indicator for diff availability
                diff_indicator_bg: Color::Rgb {
                    r: 200,
                    g: 120,
                    b: 50,
                },
                diff_indicator_fg: Color::Black,
                // File tree colors
                tree_bg: Color::Black,
                tree_fg: Color::White,
                tree_folder_fg: Color::Rgb {
                    r: 100,
                    g: 180,
                    b: 255,
                },
                tree_selected_bg: Color::Rgb {
                    r: 50,
                    g: 50,
                    b: 80,
                },
                tree_selected_fg: Color::White,
                tree_current_file_bg: Color::Rgb {
                    r: 40,
                    g: 60,
                    b: 40,
                },
                tree_git_added_fg: Color::Rgb {
                    r: 100,
                    g: 200,
                    b: 100,
                },
                tree_git_removed_fg: Color::Rgb {
                    r: 200,
                    g: 100,
                    b: 100,
                },
                tree_separator_fg: Color::Rgb {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                tree_header_fg: Color::Rgb {
                    r: 180,
                    g: 180,
                    b: 100,
                },
                tree_empty_fg: Color::DarkGrey,
                error_fg: Color::White,
                error_bg: Color::Rgb {
                    r: 150,
                    g: 50,
                    b: 50,
                },
            },
            Theme::Light => ColorScheme {
                bg: Color::White,
                _fg: Color::Black,
                selected_bg: Color::Rgb {
                    r: 220,
                    g: 220,
                    b: 220,
                },
                annotated_bg: Color::Rgb {
                    r: 200,
                    g: 220,
                    b: 240,
                },
                annotated_selected_bg: Color::Rgb {
                    r: 170,
                    g: 200,
                    b: 230,
                },
                annotation_window_bg: Color::Rgb {
                    r: 180,
                    g: 210,
                    b: 240,
                },
                annotation_window_fg: Color::Rgb {
                    r: 50,
                    g: 50,
                    b: 150,
                },
                status_bg: Color::Rgb {
                    r: 100,
                    g: 100,
                    b: 100,
                },
                status_fg: Color::White,
                line_number_fg: Color::Rgb {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                // Diff colors - pale green/red backgrounds
                diff_added_bg: Color::Rgb {
                    r: 220,
                    g: 255,
                    b: 220,
                },
                diff_removed_bg: Color::Rgb {
                    r: 255,
                    g: 220,
                    b: 220,
                },
                // Word-level highlights - more saturated
                diff_added_word_bg: Color::Rgb {
                    r: 180,
                    g: 255,
                    b: 180,
                },
                diff_removed_word_bg: Color::Rgb {
                    r: 255,
                    g: 180,
                    b: 180,
                },
                // Cursor on diff line - blend added/removed with selection
                diff_added_selected_bg: Color::Rgb {
                    r: 180,
                    g: 220,
                    b: 180,
                },
                diff_removed_selected_bg: Color::Rgb {
                    r: 220,
                    g: 180,
                    b: 180,
                },
                // Orange indicator for diff availability
                diff_indicator_bg: Color::Rgb {
                    r: 230,
                    g: 140,
                    b: 60,
                },
                diff_indicator_fg: Color::Black,
                // File tree colors
                tree_bg: Color::White,
                tree_fg: Color::Black,
                tree_folder_fg: Color::Rgb {
                    r: 30,
                    g: 100,
                    b: 200,
                },
                tree_selected_bg: Color::Rgb {
                    r: 200,
                    g: 210,
                    b: 230,
                },
                tree_selected_fg: Color::Black,
                tree_current_file_bg: Color::Rgb {
                    r: 210,
                    g: 230,
                    b: 210,
                },
                tree_git_added_fg: Color::Rgb {
                    r: 50,
                    g: 150,
                    b: 50,
                },
                tree_git_removed_fg: Color::Rgb {
                    r: 180,
                    g: 50,
                    b: 50,
                },
                tree_separator_fg: Color::Rgb {
                    r: 180,
                    g: 180,
                    b: 180,
                },
                tree_header_fg: Color::Rgb {
                    r: 120,
                    g: 120,
                    b: 50,
                },
                tree_empty_fg: Color::Grey,
                error_fg: Color::White,
                error_bg: Color::Rgb {
                    r: 200,
                    g: 80,
                    b: 80,
                },
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
        assert_eq!(colors._fg, Color::White);
        assert_eq!(colors.selected_bg, Color::Rgb { r: 40, g: 40, b: 40 });
        assert_eq!(colors.status_bg, Color::DarkGrey);
    }

    #[test]
    fn test_light_theme_colors() {
        let theme = Theme::Light;
        let colors = theme.colors();
        assert_eq!(colors.bg, Color::White);
        assert_eq!(colors._fg, Color::Black);
    }

    #[test]
    fn test_dark_theme_diff_colors() {
        let colors = Theme::Dark.colors();
        // Diff colors should be different from background
        assert_ne!(colors.diff_added_bg, colors.bg);
        assert_ne!(colors.diff_removed_bg, colors.bg);
    }

    #[test]
    fn test_light_theme_diff_colors() {
        let colors = Theme::Light.colors();
        // Diff colors should be different from background
        assert_ne!(colors.diff_added_bg, colors.bg);
        assert_ne!(colors.diff_removed_bg, colors.bg);
    }

    #[test]
    fn test_word_highlight_distinct() {
        let colors = Theme::Dark.colors();
        // Word highlights should be visually distinct from line highlights
        assert_ne!(colors.diff_added_word_bg, colors.diff_added_bg);
        assert_ne!(colors.diff_removed_word_bg, colors.diff_removed_bg);
    }

    #[test]
    fn test_diff_cursor_colors_dark_theme() {
        let colors = Theme::Dark.colors();
        // Cursor on diff lines should be different from regular diff backgrounds
        assert_ne!(colors.diff_added_selected_bg, colors.diff_added_bg);
        assert_ne!(colors.diff_removed_selected_bg, colors.diff_removed_bg);
        // Cursor colors should also differ from regular selection
        assert_ne!(colors.diff_added_selected_bg, colors.selected_bg);
        assert_ne!(colors.diff_removed_selected_bg, colors.selected_bg);
    }

    #[test]
    fn test_diff_cursor_colors_light_theme() {
        let colors = Theme::Light.colors();
        // Cursor on diff lines should be different from regular diff backgrounds
        assert_ne!(colors.diff_added_selected_bg, colors.diff_added_bg);
        assert_ne!(colors.diff_removed_selected_bg, colors.diff_removed_bg);
        // Cursor colors should also differ from regular selection
        assert_ne!(colors.diff_added_selected_bg, colors.selected_bg);
        assert_ne!(colors.diff_removed_selected_bg, colors.selected_bg);
    }

    #[test]
    fn test_dark_theme_tree_colors() {
        let colors = Theme::Dark.colors();
        // Tree colors should be different from each other
        assert_ne!(colors.tree_folder_fg, colors.tree_fg);
        assert_ne!(colors.tree_selected_bg, colors.tree_bg);
        assert_ne!(colors.tree_current_file_bg, colors.tree_bg);
        assert_ne!(colors.tree_git_added_fg, colors.tree_git_removed_fg);
    }

    #[test]
    fn test_light_theme_tree_colors() {
        let colors = Theme::Light.colors();
        // Tree colors should be different from each other
        assert_ne!(colors.tree_folder_fg, colors.tree_fg);
        assert_ne!(colors.tree_selected_bg, colors.tree_bg);
        assert_ne!(colors.tree_current_file_bg, colors.tree_bg);
        assert_ne!(colors.tree_git_added_fg, colors.tree_git_removed_fg);
    }

    #[test]
    fn test_error_colors_distinct() {
        let dark = Theme::Dark.colors();
        let light = Theme::Light.colors();
        // Error colors should be visible on their respective backgrounds
        assert_ne!(dark.error_bg, dark.bg);
        assert_ne!(light.error_bg, light.bg);
    }
}
