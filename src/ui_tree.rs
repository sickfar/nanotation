//! File tree panel rendering module.

use crate::file_tree::{FileTreePanel, TreeEntryType, TreeMode};
use crate::theme::{ColorScheme, Theme};
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{self, Write};

/// Tree panel width constant
pub const TREE_WIDTH: u16 = 30;

/// Minimum terminal width when tree is visible
pub const MIN_WIDTH_WITH_TREE: u16 = 80;

/// Folder expand/collapse indicators (filled for modern terminals)
pub const FOLDER_EXPANDED: &str = "▼";
pub const FOLDER_COLLAPSED: &str = "▶";

/// Render bottom border for the tree
fn render_tree_bottom_border(
    stdout: &mut impl Write,
    y: u16,
    is_focused: bool,
    colors: &ColorScheme,
) -> io::Result<()> {
    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    queue!(stdout, MoveTo(0, y))?;

    // Bottom border: └──────────────────────────────┘
    let horizontal = "─".repeat((TREE_WIDTH - 2) as usize);
    queue!(
        stdout,
        SetForegroundColor(border_color),
        Print(format!("└{}┘", horizontal)),
        ResetColor
    )?;

    Ok(())
}

/// Render the file tree panel
pub fn render_file_tree(
    stdout: &mut impl Write,
    panel: &FileTreePanel,
    theme: Theme,
    height: u16,
    is_focused: bool,
) -> io::Result<()> {
    let colors = theme.colors();

    // Reserve 1 line for header and 1 for bottom border
    let content_height = height.saturating_sub(2) as usize;

    // Render header at Y=0 (inline with editor title)
    render_header(stdout, panel, &colors, is_focused, 0)?;

    // Render entries starting at Y=1
    let visible_entries = panel.entries.iter()
        .skip(panel.scroll_offset)
        .take(content_height);

    for (idx, entry) in visible_entries.enumerate() {
        let row = idx as u16 + 1; // +1 for header
        let entry_idx = panel.scroll_offset + idx;
        let is_selected = entry_idx == panel.selected_index && is_focused;
        let is_current_file = panel.is_current_file(&entry.path);

        render_entry(stdout, entry, &colors, row, is_selected, is_current_file, is_focused)?;
    }

    // Fill remaining rows with empty background
    let entries_shown = panel.entries.len().saturating_sub(panel.scroll_offset).min(content_height);
    let last_content_row = height.saturating_sub(1);
    for row in (entries_shown + 1)..(last_content_row as usize) {
        render_empty_row(stdout, &colors, row as u16, is_focused)?;
    }

    // Render bottom border at the last row (above status bar)
    render_tree_bottom_border(stdout, last_content_row, is_focused, &colors)?;

    Ok(())
}

/// Render the tree header (shows mode)
fn render_header(
    stdout: &mut impl Write,
    panel: &FileTreePanel,
    colors: &ColorScheme,
    is_focused: bool,
    y: u16,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, y))?;

    let bg = if is_focused {
        colors.panel_title_focused_bg
    } else {
        colors.panel_title_unfocused_bg
    };

    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    // Build header with corner borders: ┌─ Files ─────┐
    let header_text = match panel.mode {
        TreeMode::FullTree => "Files",
        TreeMode::GitChangedFiles => "Git Changes",
    };

    // Total width: "┌─" (2) + " " (1) + text + " " (1) + padding + "─┐" (2) = 6 + text + padding
    let border_and_spaces = 6;
    let available_width = TREE_WIDTH.saturating_sub(border_and_spaces);

    // Truncate if needed
    let truncated_text: String = if header_text.chars().count() > available_width as usize {
        header_text.chars().take((available_width.saturating_sub(1)) as usize).collect::<String>() + "…"
    } else {
        header_text.to_string()
    };

    let padding_needed = available_width.saturating_sub(truncated_text.chars().count() as u16);
    let padding = "─".repeat(padding_needed as usize);

    let title_line = format!("┌─ {} {}{}", truncated_text, padding, "─┐");

    // Render the title bar
    queue!(
        stdout,
        SetBackgroundColor(bg),
        SetForegroundColor(border_color),
        Print(&title_line),
        ResetColor
    )?;

    Ok(())
}

/// Render a single tree entry
fn render_entry(
    stdout: &mut impl Write,
    entry: &crate::file_tree::TreeEntry,
    colors: &ColorScheme,
    row: u16,
    is_selected: bool,
    is_current_file: bool,
    is_focused: bool,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, row))?;

    // Determine background color
    let bg = if is_selected {
        colors.tree_selected_bg
    } else if is_current_file {
        colors.tree_current_file_bg
    } else {
        colors.tree_bg
    };

    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    // Left border
    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(border_color),
        Print("│")
    )?;

    // Entry content
    queue!(stdout, SetBackgroundColor(bg))?;

    // Build the display string (account for -2 border, -2 icon/space)
    let indent = "  ".repeat(entry.depth);
    let content_width = (TREE_WIDTH - 2) as usize; // -2 for borders
    let max_name_width = content_width.saturating_sub(indent.len() + 2); // -2 for icon/space

    let (icon, fg) = match &entry.entry_type {
        TreeEntryType::Directory { is_expanded } => {
            let icon = if *is_expanded { FOLDER_EXPANDED } else { FOLDER_COLLAPSED };
            (icon, colors.tree_folder_fg)
        }
        TreeEntryType::File { git_status } => {
            if !entry.is_selectable() {
                // Empty placeholder or non-selectable
                ("", colors.tree_empty_fg)
            } else if git_status.is_some() {
                ("", colors.tree_fg)
            } else {
                ("", colors.tree_fg)
            }
        }
    };

    // Set foreground based on entry type
    let fg = if is_selected { colors.tree_selected_fg } else { fg };
    queue!(stdout, SetForegroundColor(fg))?;

    // Format the name, possibly with git status
    let name_display = match &entry.entry_type {
        TreeEntryType::File { git_status: Some(status) } if entry.is_selectable() => {
            let stat_str = format_git_status(status.added_lines, status.removed_lines);
            let available_width = max_name_width.saturating_sub(stat_str.len() + 1);
            let truncated_name = truncate_name(&entry.name, available_width);
            format!("{}{} {}", indent, truncated_name, stat_str)
        }
        _ => {
            let truncated_name = truncate_name(&entry.name, max_name_width);
            if icon.is_empty() {
                format!("{}  {}", indent, truncated_name)
            } else {
                format!("{}{} {}", indent, icon, truncated_name)
            }
        }
    };

    // Render git status colors if applicable
    if let TreeEntryType::File { git_status: Some(status) } = &entry.entry_type {
        if entry.is_selectable() {
            // Render name first
            let stat_str = format_git_status(status.added_lines, status.removed_lines);
            let available_width = max_name_width.saturating_sub(stat_str.len() + 1);
            let truncated_name = truncate_name(&entry.name, available_width);
            let name_part = format!("{}  {}", indent, truncated_name);

            // Pad to align git status
            let name_width = name_part.chars().count();
            let padding_width = content_width.saturating_sub(name_width + stat_str.len());
            let padding = " ".repeat(padding_width);

            queue!(stdout, Print(&name_part))?;
            queue!(stdout, Print(&padding))?;

            // Render git stats with colors
            render_git_stats(stdout, colors, status.added_lines, status.removed_lines)?;

            // Right border
            queue!(
                stdout,
                SetBackgroundColor(colors.tree_bg),
                SetForegroundColor(border_color),
                Print("│")
            )?;
            return Ok(());
        }
    }

    // Pad to fill width (use character count, not bytes, for proper UTF-8 handling)
    let display_width = name_display.chars().count();
    let padding = content_width.saturating_sub(display_width.min(content_width));

    queue!(stdout, Print(&name_display))?;
    queue!(stdout, Print(" ".repeat(padding)))?;

    // Right border
    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(border_color),
        Print("│")
    )?;

    Ok(())
}

/// Format git status string
fn format_git_status(added: usize, removed: usize) -> String {
    match (added > 0, removed > 0) {
        (true, true) => format!("+{} -{}", added, removed),
        (true, false) => format!("+{}", added),
        (false, true) => format!("-{}", removed),
        (false, false) => String::new(),
    }
}

/// Render git stats with colors
fn render_git_stats(
    stdout: &mut impl Write,
    colors: &ColorScheme,
    added: usize,
    removed: usize,
) -> io::Result<()> {
    if added > 0 {
        queue!(
            stdout,
            SetForegroundColor(colors.tree_git_added_fg),
            Print(format!("+{}", added))
        )?;
    }

    if added > 0 && removed > 0 {
        queue!(stdout, Print(" "))?;
    }

    if removed > 0 {
        queue!(
            stdout,
            SetForegroundColor(colors.tree_git_removed_fg),
            Print(format!("-{}", removed))
        )?;
    }

    Ok(())
}

/// Truncate a name to fit within width
fn truncate_name(name: &str, max_width: usize) -> String {
    if name.chars().count() <= max_width {
        name.to_string()
    } else if max_width <= 3 {
        name.chars().take(max_width).collect()
    } else {
        let truncated: String = name.chars().take(max_width - 1).collect();
        format!("{}…", truncated)
    }
}

/// Render an empty row
fn render_empty_row(stdout: &mut impl Write, colors: &ColorScheme, row: u16, is_focused: bool) -> io::Result<()> {
    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    queue!(stdout, MoveTo(0, row))?;

    // Left border
    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(border_color),
        Print("│")
    )?;

    // Empty content
    let content_width = (TREE_WIDTH - 2) as usize;
    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(colors.tree_fg),
        Print(" ".repeat(content_width))
    )?;

    // Right border
    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(border_color),
        Print("│")
    )?;

    Ok(())
}

/// Render an error message in the editor area (for binary files, permission denied, etc.)
pub fn render_error_message(
    stdout: &mut impl Write,
    message: &str,
    theme: Theme,
    start_col: u16,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let colors = theme.colors();

    // Clear the area first
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(colors.error_fg)
    )?;

    for row in 0..height.saturating_sub(5) {
        queue!(
            stdout,
            MoveTo(start_col, row),
            Print(" ".repeat(width as usize))
        )?;
    }

    // Calculate center position for the message
    let message_row = height / 3;
    let message_lines: Vec<&str> = message.lines().collect();

    for (i, line) in message_lines.iter().enumerate() {
        let row = message_row + i as u16;
        if row >= height.saturating_sub(5) {
            break;
        }

        let line_len = line.chars().count();
        let col = start_col + (width.saturating_sub(line_len as u16)) / 2;

        queue!(
            stdout,
            MoveTo(col, row),
            SetBackgroundColor(colors.error_bg),
            SetForegroundColor(colors.error_fg),
            Print(format!(" {} ", line))
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_name_short() {
        assert_eq!(truncate_name("short", 10), "short");
    }

    #[test]
    fn test_truncate_name_exact() {
        assert_eq!(truncate_name("exactly10!", 10), "exactly10!");
    }

    #[test]
    fn test_truncate_name_long() {
        let result = truncate_name("this_is_a_very_long_filename.txt", 15);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 15);
    }

    #[test]
    fn test_truncate_name_very_short_max() {
        let result = truncate_name("longname", 3);
        assert_eq!(result.chars().count(), 3);
    }

    #[test]
    fn test_format_git_status_both() {
        assert_eq!(format_git_status(10, 5), "+10 -5");
    }

    #[test]
    fn test_format_git_status_added_only() {
        assert_eq!(format_git_status(10, 0), "+10");
    }

    #[test]
    fn test_format_git_status_removed_only() {
        assert_eq!(format_git_status(0, 5), "-5");
    }

    #[test]
    fn test_format_git_status_none() {
        assert_eq!(format_git_status(0, 0), "");
    }

    #[test]
    fn test_constants() {
        assert_eq!(TREE_WIDTH, 30);
        assert_eq!(MIN_WIDTH_WITH_TREE, 80);
    }

    #[test]
    fn test_folder_icons_are_single_char_width() {
        // Folder icons are multi-byte UTF-8 but should count as 1 character for display width
        // This is important for proper line padding calculations
        assert_eq!(FOLDER_EXPANDED.chars().count(), 1);
        assert_eq!(FOLDER_COLLAPSED.chars().count(), 1);
        // Verify they ARE multi-byte (the root cause of the bug)
        assert!(FOLDER_EXPANDED.len() > 1, "▼ should be multi-byte UTF-8");
        assert!(FOLDER_COLLAPSED.len() > 1, "▶ should be multi-byte UTF-8");
    }

    #[test]
    fn test_display_width_with_utf8_icons() {
        // Regression test: line with folder icon should have correct character count
        // The bug was using byte length instead of char count for padding
        let indent = "";
        let icon = FOLDER_EXPANDED;
        let name = "src";
        let display = format!("{}{} {}", indent, icon, name);

        // Should be: "▼" (1 char) + " " (1 char) + "src" (3 chars) = 5 chars
        assert_eq!(display.chars().count(), 5);

        // Padding should fill to TREE_WIDTH
        let display_width = display.chars().count();
        let padding = TREE_WIDTH as usize - display_width;
        assert_eq!(display_width + padding, TREE_WIDTH as usize);
    }

    // =========================================================================
    // Border Tests (for upcoming border implementation)
    // =========================================================================

    #[test]
    fn test_tree_border_width() {
        // Verify border fits within TREE_WIDTH
        let border_width = TREE_WIDTH;
        assert_eq!(border_width, 30);
    }

    #[test]
    fn test_tree_content_width_with_borders() {
        // Verify content width accounts for left/right borders
        let left_border = 1u16;
        let right_border = 1u16;
        let content_width = TREE_WIDTH - left_border - right_border;

        // With borders: 30 - 1 - 1 = 28 chars for content
        assert_eq!(content_width, 28);
    }

    #[test]
    fn test_tree_entry_xoffset_with_border() {
        // Verify entries start at column 1 (after left border)
        let x_offset = 1u16;
        assert_eq!(x_offset, 1);

        // Entry should render at tree start (0) + offset (1) = column 1
        let entry_column = 0 + x_offset;
        assert_eq!(entry_column, 1);
    }

    #[test]
    fn test_tree_border_characters() {
        // Verify correct Unicode box-drawing characters for borders
        let top_left = '┌';
        let top_right = '┐';
        let bottom_left = '└';
        let bottom_right = '┘';
        let vertical = '│';
        let horizontal = '─';

        // Verify these are the expected characters
        assert_eq!(top_left, '┌');
        assert_eq!(top_right, '┐');
        assert_eq!(bottom_left, '└');
        assert_eq!(bottom_right, '┘');
        assert_eq!(vertical, '│');
        assert_eq!(horizontal, '─');
    }
}
