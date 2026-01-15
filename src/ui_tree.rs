//! File tree panel rendering module.

use crate::file_tree::{FileTreePanel, TreeEntryType, TreeMode};
use crate::theme::{ColorScheme, Theme};
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Print, SetBackgroundColor, SetForegroundColor},
};
use std::io::{self, Write};

/// Tree panel width constant
pub const TREE_WIDTH: u16 = 30;

/// Minimum terminal width when tree is visible
pub const MIN_WIDTH_WITH_TREE: u16 = 80;

/// Separator character for tree/editor border
pub const TREE_SEPARATOR: char = '│';

/// Folder expand/collapse indicators (filled for modern terminals)
pub const FOLDER_EXPANDED: &str = "▼";
pub const FOLDER_COLLAPSED: &str = "▶";

/// Render the file tree panel
pub fn render_file_tree(
    stdout: &mut impl Write,
    panel: &FileTreePanel,
    theme: Theme,
    height: u16,
    is_focused: bool,
) -> io::Result<()> {
    let colors = theme.colors();
    let content_height = height.saturating_sub(1) as usize; // Reserve 1 line for header

    // Render header
    render_header(stdout, panel, &colors, is_focused)?;

    // Render entries
    let visible_entries = panel.entries.iter()
        .skip(panel.scroll_offset)
        .take(content_height);

    for (idx, entry) in visible_entries.enumerate() {
        let row = idx as u16 + 1; // +1 for header
        let entry_idx = panel.scroll_offset + idx;
        let is_selected = entry_idx == panel.selected_index && is_focused;
        let is_current_file = panel.is_current_file(&entry.path);

        render_entry(stdout, entry, &colors, row, is_selected, is_current_file)?;
    }

    // Fill remaining rows with empty background
    let entries_shown = panel.entries.len().saturating_sub(panel.scroll_offset).min(content_height);
    for row in (entries_shown + 1)..=(content_height) {
        render_empty_row(stdout, &colors, row as u16)?;
    }

    Ok(())
}

/// Render the tree header (shows mode)
fn render_header(
    stdout: &mut impl Write,
    panel: &FileTreePanel,
    colors: &ColorScheme,
    is_focused: bool,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0))?;

    let bg = if is_focused {
        colors.tree_selected_bg
    } else {
        colors.tree_bg
    };

    queue!(
        stdout,
        SetBackgroundColor(bg),
        SetForegroundColor(colors.tree_header_fg)
    )?;

    let header = match panel.mode {
        TreeMode::FullTree => " Files",
        TreeMode::GitChangedFiles => " Git Changes",
    };

    let header_display = format!("{:<width$}", header, width = TREE_WIDTH as usize);
    queue!(stdout, Print(&header_display[..TREE_WIDTH as usize]))?;

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

    queue!(stdout, SetBackgroundColor(bg))?;

    // Build the display string
    let indent = "  ".repeat(entry.depth);
    let max_name_width = TREE_WIDTH as usize - indent.len() - 2; // -2 for icon/space

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
            let padding_width = (TREE_WIDTH as usize).saturating_sub(name_width + stat_str.len());
            let padding = " ".repeat(padding_width);

            queue!(stdout, Print(&name_part))?;
            queue!(stdout, Print(&padding))?;

            // Render git stats with colors
            render_git_stats(stdout, colors, status.added_lines, status.removed_lines)?;
            return Ok(());
        }
    }

    // Pad to fill width
    let display_width = name_display.chars().count();
    let padding = TREE_WIDTH as usize - display_width.min(TREE_WIDTH as usize);
    let padded = format!("{}{}", name_display, " ".repeat(padding));

    queue!(stdout, Print(&padded[..TREE_WIDTH as usize]))?;

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
fn render_empty_row(stdout: &mut impl Write, colors: &ColorScheme, row: u16) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, row),
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(colors.tree_fg),
        Print(" ".repeat(TREE_WIDTH as usize))
    )?;
    Ok(())
}

/// Render the separator between tree and editor
pub fn render_separator(stdout: &mut impl Write, theme: Theme, height: u16) -> io::Result<()> {
    let colors = theme.colors();

    queue!(
        stdout,
        SetBackgroundColor(colors.tree_bg),
        SetForegroundColor(colors.tree_separator_fg)
    )?;

    for row in 0..height {
        queue!(
            stdout,
            MoveTo(TREE_WIDTH, row),
            Print(TREE_SEPARATOR)
        )?;
    }

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
        assert_eq!(TREE_SEPARATOR, '│');
    }
}
