//! Diff calculation module for word-level diffing with whitespace normalization.

use similar::{ChangeTag, TextDiff};

use crate::models::Line;

/// Type of change for a word or line
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Unchanged,
    Added,
    Removed,
}

/// Represents a change in a single word
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordChange {
    pub text: String,
    pub change_type: ChangeType,
}

/// Type of change for a line
#[derive(Debug, Clone, PartialEq)]
pub enum LineChange {
    Unchanged,
    Added,
    Removed,
    Modified {
        words: Vec<WordChange>,
        old_leading_ws: String,
        new_leading_ws: String,
    },
}

/// A single diff line with optional working and HEAD content
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Working version: (line_number, content, change_type)
    pub working: Option<(usize, String, LineChange)>,
    /// HEAD version: (line_number, content, LineChange)
    pub head: Option<(usize, String, LineChange)>,
}

/// Result of diff calculation containing aligned line pairs
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub lines: Vec<DiffLine>,
}

/// Tokenize a line into words, splitting on whitespace and punctuation.
/// Punctuation characters become separate tokens.
///
/// Examples:
/// - "hello world" -> ["hello", "world"]
/// - "foo.bar" -> ["foo", ".", "bar"]
/// - "getUserName()" -> ["getUserName", "(", ")"]
pub fn tokenize_line(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None;

    for (i, c) in line.char_indices() {
        if c.is_whitespace() {
            // End current token if any
            if let Some(s) = start {
                tokens.push(&line[s..i]);
                start = None;
            }
        } else if c.is_alphanumeric() || c == '_' {
            // Continue or start a word token
            if start.is_none() {
                start = Some(i);
            }
        } else {
            // Punctuation: end current token and add punctuation as separate token
            if let Some(s) = start {
                tokens.push(&line[s..i]);
                start = None;
            }
            // Add punctuation as its own token
            // Handle multi-char operators by looking ahead
            let next_char = line[i..].chars().nth(1);
            let is_double_op = matches!(
                (c, next_char),
                ('-', Some('>'))
                    | ('=', Some('>'))
                    | ('=', Some('='))
                    | ('!', Some('='))
                    | ('<', Some('='))
                    | ('>', Some('='))
                    | ('+', Some('='))
                    | ('-', Some('='))
                    | ('*', Some('='))
                    | ('/', Some('='))
                    | ('&', Some('&'))
                    | ('|', Some('|'))
                    | ('<', Some('<'))
                    | ('>', Some('>'))
                    | (':', Some(':'))
            );

            if is_double_op {
                // Will be handled when we see the second char
                if !tokens.last().map(|t| t.ends_with(c)).unwrap_or(false) {
                    let end = i + c.len_utf8() + next_char.unwrap().len_utf8();
                    tokens.push(&line[i..end]);
                }
            } else {
                // Check if this is the second char of a double operator (skip it)
                let prev_token = tokens.last();
                let is_second_of_double = prev_token.map(|t| {
                    let bytes = t.as_bytes();
                    bytes.len() == 2 && line[i..].starts_with(&t[1..])
                }).unwrap_or(false);

                if !is_second_of_double {
                    tokens.push(&line[i..i + c.len_utf8()]);
                }
            }
        }
    }

    // Add final token if any
    if let Some(s) = start {
        tokens.push(&line[s..]);
    }

    tokens
}

/// Check if the difference between two lines is only whitespace (leading/trailing).
pub fn is_whitespace_only_change(old: &str, new: &str) -> bool {
    let old_trimmed = old.trim();
    let new_trimmed = new.trim();
    old_trimmed == new_trimmed
}

/// Check if two lines have the same tokens, just reordered.
/// This handles cases like import reordering: `{A, B, C}` vs `{B, C, A}`
fn is_same_tokens_reordered(a: &[&str], b: &[&str]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_sorted: Vec<&str> = a.to_vec();
    let mut b_sorted: Vec<&str> = b.to_vec();
    a_sorted.sort();
    b_sorted.sort();
    a_sorted == b_sorted
}

/// Calculate similarity between two lines based on word overlap.
/// Returns a value between 0.0 (completely different) and 1.0 (identical).
///
/// Also detects reordered tokens (same tokens, different order) which is
/// common with auto-sorted imports like `{A, B, C}` vs `{B, C, A}`.
pub fn line_similarity(old: &str, new: &str) -> f32 {
    let old_trimmed = old.trim();
    let new_trimmed = new.trim();

    // Both empty = identical
    if old_trimmed.is_empty() && new_trimmed.is_empty() {
        return 1.0;
    }

    // One empty, one not = completely different
    if old_trimmed.is_empty() || new_trimmed.is_empty() {
        return 0.0;
    }

    let old_words: Vec<&str> = tokenize_line(old_trimmed);
    let new_words: Vec<&str> = tokenize_line(new_trimmed);

    if old_words.is_empty() && new_words.is_empty() {
        return 1.0;
    }

    if old_words.is_empty() || new_words.is_empty() {
        return 0.0;
    }

    // Check if lines are identical
    if old_words == new_words {
        return 1.0;
    }

    // Check for reordered tokens (same tokens, different order)
    // Common with auto-sorted imports: `{A, B, C}` vs `{B, C, A}`
    if is_same_tokens_reordered(&old_words, &new_words) {
        return 0.95; // Nearly identical, just reordered
    }

    let diff = TextDiff::from_slices(&old_words, &new_words);
    let unchanged: usize = diff
        .iter_all_changes()
        .filter(|c| c.tag() == ChangeTag::Equal)
        .count();

    let max_len = old_words.len().max(new_words.len());
    unchanged as f32 / max_len as f32
}

/// Result of word-level diff including preserved leading whitespace.
pub struct WordDiffResult {
    pub old_leading_ws: String,
    pub new_leading_ws: String,
    pub changes: Vec<WordChange>,
}

/// Compute word-level diff between two lines.
/// Preserves leading whitespace from both lines.
pub fn diff_words(old: &str, new: &str) -> WordDiffResult {
    // Preserve leading whitespace
    let old_trimmed = old.trim_start();
    let new_trimmed = new.trim_start();
    let old_leading_ws = old[..old.len() - old_trimmed.len()].to_string();
    let new_leading_ws = new[..new.len() - new_trimmed.len()].to_string();

    let old_words: Vec<&str> = tokenize_line(old_trimmed.trim_end());
    let new_words: Vec<&str> = tokenize_line(new_trimmed.trim_end());

    let diff = TextDiff::from_slices(&old_words, &new_words);
    let mut changes = Vec::new();

    for change in diff.iter_all_changes() {
        let change_type = match change.tag() {
            ChangeTag::Equal => ChangeType::Unchanged,
            ChangeTag::Insert => ChangeType::Added,
            ChangeTag::Delete => ChangeType::Removed,
        };

        changes.push(WordChange {
            text: change.value().to_string(),
            change_type,
        });
    }

    WordDiffResult {
        old_leading_ws,
        new_leading_ws,
        changes,
    }
}

/// Calculate diff between working lines and HEAD content.
/// Annotations are stripped from working content before comparison.
pub fn calculate_diff(working: &[Line], head_content: &str, comment_style: &str) -> DiffResult {
    // Prepare working content (strip annotations)
    let working_lines: Vec<String> = working
        .iter()
        .map(|line| strip_annotation(&line.content, comment_style))
        .collect();

    // Parse HEAD content
    let head_lines: Vec<&str> = if head_content.is_empty() {
        Vec::new()
    } else {
        head_content.lines().collect()
    };

    // Perform line-level diff
    let working_strs: Vec<&str> = working_lines.iter().map(|s| s.as_str()).collect();
    let diff = TextDiff::from_slices(&head_lines, &working_strs);

    let mut result_lines = Vec::new();
    let mut working_line_num = 0usize;
    let mut head_line_num = 0usize;

    // Collect changes for analysis
    let changes: Vec<_> = diff.iter_all_changes().collect();
    let mut i = 0;

    while i < changes.len() {
        let change = &changes[i];

        match change.tag() {
            ChangeTag::Equal => {
                // Unchanged line (but check for whitespace-only changes)
                working_line_num += 1;
                head_line_num += 1;
                result_lines.push(DiffLine {
                    working: Some((
                        working_line_num,
                        working[working_line_num - 1].content.clone(),
                        LineChange::Unchanged,
                    )),
                    head: Some((
                        head_line_num,
                        head_lines[head_line_num - 1].to_string(),
                        LineChange::Unchanged,
                    )),
                });
            }
            ChangeTag::Delete => {
                // Line removed from HEAD - check if next change is an Insert (modification)
                let next_is_insert = changes.get(i + 1).map(|c| c.tag() == ChangeTag::Insert).unwrap_or(false);

                if next_is_insert {
                    // Check similarity
                    let old_line = change.value();
                    let new_line = changes[i + 1].value();

                    if is_whitespace_only_change(old_line, new_line) {
                        // Whitespace-only change - treat as unchanged
                        working_line_num += 1;
                        head_line_num += 1;
                        result_lines.push(DiffLine {
                            working: Some((
                                working_line_num,
                                working[working_line_num - 1].content.clone(),
                                LineChange::Unchanged,
                            )),
                            head: Some((
                                head_line_num,
                                old_line.to_string(),
                                LineChange::Unchanged,
                            )),
                        });
                        i += 1; // Skip the Insert
                    } else if line_similarity(old_line, new_line) >= 0.5 {
                        // Modified line - compute word diff
                        let word_diff = diff_words(old_line, new_line);
                        working_line_num += 1;
                        head_line_num += 1;
                        result_lines.push(DiffLine {
                            working: Some((
                                working_line_num,
                                working[working_line_num - 1].content.clone(),
                                LineChange::Modified {
                                    words: word_diff.changes.clone(),
                                    old_leading_ws: word_diff.old_leading_ws.clone(),
                                    new_leading_ws: word_diff.new_leading_ws.clone(),
                                },
                            )),
                            head: Some((
                                head_line_num,
                                old_line.to_string(),
                                LineChange::Modified {
                                    words: word_diff.changes,
                                    old_leading_ws: word_diff.old_leading_ws,
                                    new_leading_ws: word_diff.new_leading_ws,
                                },
                            )),
                        });
                        i += 1; // Skip the Insert
                    } else {
                        // Too different - show as separate delete and insert
                        head_line_num += 1;
                        result_lines.push(DiffLine {
                            working: None,
                            head: Some((head_line_num, old_line.to_string(), LineChange::Removed)),
                        });
                    }
                } else {
                    // Pure deletion
                    head_line_num += 1;
                    result_lines.push(DiffLine {
                        working: None,
                        head: Some((head_line_num, change.value().to_string(), LineChange::Removed)),
                    });
                }
            }
            ChangeTag::Insert => {
                // Line added in working
                working_line_num += 1;
                result_lines.push(DiffLine {
                    working: Some((
                        working_line_num,
                        working[working_line_num - 1].content.clone(),
                        LineChange::Added,
                    )),
                    head: None,
                });
            }
        }

        i += 1;
    }

    DiffResult { lines: result_lines }
}

/// Strip annotation from a line content.
fn strip_annotation(content: &str, comment_style: &str) -> String {
    if comment_style.is_empty() {
        // For markdown, annotations are standalone [ANNOTATION]
        if let Some(pos) = content.find("[ANNOTATION]") {
            return content[..pos].trim_end().to_string();
        }
        return content.to_string();
    }

    // Find comment with annotation marker
    let annotation_marker = format!("{} [ANNOTATION]", comment_style);
    if let Some(pos) = content.find(&annotation_marker) {
        return content[..pos].trim_end().to_string();
    }

    content.to_string()
}

#[cfg(test)]
mod tokenize_tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_words() {
        assert_eq!(tokenize_line("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_punctuation() {
        assert_eq!(tokenize_line("foo.bar"), vec!["foo", ".", "bar"]);
    }

    #[test]
    fn test_tokenize_function_call() {
        assert_eq!(
            tokenize_line("getUserName()"),
            vec!["getUserName", "(", ")"]
        );
    }

    #[test]
    fn test_tokenize_complex() {
        assert_eq!(
            tokenize_line("arr[0].value"),
            vec!["arr", "[", "0", "]", ".", "value"]
        );
    }

    #[test]
    fn test_tokenize_operators() {
        assert_eq!(tokenize_line("a -> b"), vec!["a", "->", "b"]);
        assert_eq!(tokenize_line("x += 1"), vec!["x", "+=", "1"]);
        assert_eq!(tokenize_line("a == b"), vec!["a", "==", "b"]);
    }

    #[test]
    fn test_tokenize_empty() {
        assert_eq!(tokenize_line(""), Vec::<&str>::new());
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        assert_eq!(tokenize_line("   "), Vec::<&str>::new());
    }

    #[test]
    fn test_tokenize_tabs_spaces() {
        assert_eq!(tokenize_line("a\t  b"), vec!["a", "b"]);
    }

    #[test]
    fn test_tokenize_preserves_quotes() {
        assert_eq!(tokenize_line("\"hello\""), vec!["\"", "hello", "\""]);
    }

    #[test]
    fn test_tokenize_camelcase() {
        assert_eq!(tokenize_line("getUserName"), vec!["getUserName"]);
    }

    #[test]
    fn test_tokenize_snake_case() {
        assert_eq!(tokenize_line("get_user_name"), vec!["get_user_name"]);
    }

    #[test]
    fn test_tokenize_numbers() {
        assert_eq!(tokenize_line("x = 123"), vec!["x", "=", "123"]);
    }
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;

    #[test]
    fn test_whitespace_only_leading_spaces() {
        assert!(is_whitespace_only_change("    foo", "        foo"));
    }

    #[test]
    fn test_whitespace_only_tab_to_spaces() {
        assert!(is_whitespace_only_change("\tfoo", "    foo"));
    }

    #[test]
    fn test_whitespace_only_trailing() {
        assert!(is_whitespace_only_change("foo   ", "foo"));
    }

    #[test]
    fn test_whitespace_content_changed() {
        assert!(!is_whitespace_only_change("foo", "bar"));
    }

    #[test]
    fn test_whitespace_internal_change() {
        // Internal whitespace changes are detected by trim comparison
        // "a b" and "a  b" both trim to "a b" and "a  b" which are different
        // Actually trim() only removes leading/trailing, not internal
        assert!(!is_whitespace_only_change("a b", "a  b"));
    }

    #[test]
    fn test_whitespace_empty_vs_spaces() {
        assert!(is_whitespace_only_change("", "   "));
    }
}

#[cfg(test)]
mod similarity_tests {
    use super::*;

    #[test]
    fn test_similarity_identical() {
        assert!((line_similarity("foo bar", "foo bar") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_completely_different() {
        assert!((line_similarity("aaa bbb", "xxx yyy") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_half() {
        let sim = line_similarity("foo bar", "foo baz");
        // foo matches (1), bar->baz is delete+insert
        // With TextDiff, "foo" is equal, "bar" is removed, "baz" is added
        // unchanged = 1, max_len = 2, so sim = 0.5
        assert!((sim - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_similarity_above_threshold() {
        let sim = line_similarity("a b c d", "a b c e");
        // a, b, c match (3), d->e is change
        // unchanged = 3, max_len = 4, sim = 0.75
        assert!(sim >= 0.5);
    }

    #[test]
    fn test_similarity_below_threshold() {
        let sim = line_similarity("a b", "x y z");
        // Nothing matches directly
        assert!(sim < 0.5);
    }

    #[test]
    fn test_similarity_empty() {
        assert!((line_similarity("", "") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_one_empty() {
        assert!((line_similarity("foo", "") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_reordered_imports() {
        // Same tokens, just reordered - common with auto-sorted imports
        let sim = line_similarity(
            "use crate::models::{Action, Line, Mode};",
            "use crate::models::{Line, Mode, Action};",
        );
        // Should be very high similarity (same tokens, different order)
        assert!(
            sim >= 0.9,
            "Reordered imports should be nearly identical: {}",
            sim
        );
    }

    #[test]
    fn test_similarity_reordered_simple() {
        // Simple reordering case
        let sim = line_similarity("a, b, c", "c, b, a");
        assert!(sim >= 0.9, "Reordered items should be similar: {}", sim);
    }

    #[test]
    fn test_similarity_not_reordered_different_tokens() {
        // Different tokens - should not match reorder check
        let sim = line_similarity("a, b, c", "x, y, z");
        assert!(sim < 0.5, "Different tokens should not be similar: {}", sim);
    }
}

#[cfg(test)]
mod word_diff_tests {
    use super::*;

    #[test]
    fn test_diff_words_single_change() {
        let result = diff_words("foo bar", "foo baz");
        assert_eq!(result.changes.len(), 3);
        assert_eq!(
            result.changes[0],
            WordChange {
                text: "foo".into(),
                change_type: ChangeType::Unchanged
            }
        );
        assert_eq!(
            result.changes[1],
            WordChange {
                text: "bar".into(),
                change_type: ChangeType::Removed
            }
        );
        assert_eq!(
            result.changes[2],
            WordChange {
                text: "baz".into(),
                change_type: ChangeType::Added
            }
        );
    }

    #[test]
    fn test_diff_words_addition() {
        let result = diff_words("a b", "a b c");
        assert!(result.changes
            .iter()
            .any(|c| c.text == "c" && c.change_type == ChangeType::Added));
    }

    #[test]
    fn test_diff_words_removal() {
        let result = diff_words("a b c", "a c");
        assert!(result.changes
            .iter()
            .any(|c| c.text == "b" && c.change_type == ChangeType::Removed));
    }

    #[test]
    fn test_diff_words_identical() {
        let result = diff_words("foo bar", "foo bar");
        assert!(result.changes
            .iter()
            .all(|c| c.change_type == ChangeType::Unchanged));
    }

    #[test]
    fn test_diff_words_all_different() {
        let result = diff_words("a b", "x y");
        assert!(!result.changes
            .iter()
            .any(|c| c.change_type == ChangeType::Unchanged));
    }

    #[test]
    fn test_diff_words_preserves_leading_whitespace() {
        let result = diff_words("    indented", "        more_indented");
        assert_eq!(result.old_leading_ws, "    ");
        assert_eq!(result.new_leading_ws, "        ");
    }

    #[test]
    fn test_diff_words_preserves_tab_indentation() {
        let result = diff_words("\t\tcode", "\tcode");
        assert_eq!(result.old_leading_ws, "\t\t");
        assert_eq!(result.new_leading_ws, "\t");
    }

    #[test]
    fn test_diff_words_no_leading_whitespace() {
        let result = diff_words("no indent", "also no indent");
        assert_eq!(result.old_leading_ws, "");
        assert_eq!(result.new_leading_ws, "");
    }
}

#[cfg(test)]
mod alignment_tests {
    use super::*;

    fn line(content: &str) -> Line {
        Line {
            content: content.to_string(),
            annotation: None,
        }
    }

    fn line_with_annotation(content: &str, annotation: &str) -> Line {
        Line {
            content: content.to_string(),
            annotation: Some(annotation.to_string()),
        }
    }

    #[test]
    fn test_calculate_diff_no_changes() {
        let working = vec![line("line1"), line("line2")];
        let head = "line1\nline2";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 2);
        assert!(result
            .lines
            .iter()
            .all(|l| matches!(l.working.as_ref().unwrap().2, LineChange::Unchanged)));
    }

    #[test]
    fn test_calculate_diff_line_added() {
        let working = vec![line("line1"), line("line2"), line("line3")];
        let head = "line1\nline2";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 3);
        assert!(result.lines[2].head.is_none());
        assert!(matches!(
            result.lines[2].working.as_ref().unwrap().2,
            LineChange::Added
        ));
    }

    #[test]
    fn test_calculate_diff_line_removed() {
        let working = vec![line("line1")];
        let head = "line1\nline2";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 2);
        assert!(result.lines[1].working.is_none());
        assert!(matches!(
            result.lines[1].head.as_ref().unwrap().2,
            LineChange::Removed
        ));
    }

    #[test]
    fn test_calculate_diff_line_modified() {
        let working = vec![line("foo bar baz")];
        let head = "foo BAR baz";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 1);
        assert!(matches!(
            result.lines[0].working.as_ref().unwrap().2,
            LineChange::Modified { .. }
        ));
    }

    #[test]
    fn test_calculate_diff_whitespace_ignored() {
        let working = vec![line("    foo")];
        let head = "\tfoo";
        let result = calculate_diff(&working, head, "//");
        assert!(matches!(
            result.lines[0].working.as_ref().unwrap().2,
            LineChange::Unchanged
        ));
    }

    #[test]
    fn test_calculate_diff_annotation_stripped() {
        let working = vec![line_with_annotation("code here", "fix this")];
        let head = "code here";
        let result = calculate_diff(&working, head, "//");
        assert!(matches!(
            result.lines[0].working.as_ref().unwrap().2,
            LineChange::Unchanged
        ));
    }

    #[test]
    fn test_calculate_diff_multiple_hunks() {
        let working = vec![
            line("unchanged"),
            line("modified line here"),
            line("unchanged2"),
            line("new line"),
        ];
        let head = "unchanged\nold line was here\nunchanged2";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 4);
    }

    #[test]
    fn test_calculate_diff_empty_working() {
        let working: Vec<Line> = vec![];
        let head = "line1\nline2";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 2);
        assert!(result.lines.iter().all(|l| l.working.is_none()));
    }

    #[test]
    fn test_calculate_diff_empty_head() {
        let working = vec![line("line1")];
        let head = "";
        let result = calculate_diff(&working, head, "//");
        assert_eq!(result.lines.len(), 1);
        assert!(result.lines[0].head.is_none());
    }
}

// ============================================================================
// Scroll Synchronization Helpers
// ============================================================================

/// Maps a working copy line number (0-indexed) to its diff line index.
/// Returns None if the cursor_line doesn't have a corresponding diff line.
pub fn cursor_to_diff_index(diff_result: &DiffResult, cursor_line: usize) -> Option<usize> {
    let target_line_num = cursor_line + 1; // DiffLine uses 1-indexed line numbers

    for (idx, diff_line) in diff_result.lines.iter().enumerate() {
        if let Some((line_num, _, _)) = &diff_line.working {
            if *line_num == target_line_num {
                return Some(idx);
            }
        }
    }
    None
}


/// Adjusts scroll_offset to ensure the cursor's diff line is visible.
/// Returns the new scroll_offset.
///
/// This is a pure function that doesn't require terminal access, making it testable.
pub fn adjust_diff_scroll(
    cursor_line: usize,
    current_scroll: usize,
    visible_height: usize,
    diff_result: &DiffResult,
) -> usize {
    if let Some(diff_idx) = cursor_to_diff_index(diff_result, cursor_line) {
        if diff_idx < current_scroll {
            // Cursor above visible area - scroll up
            diff_idx
        } else if diff_idx >= current_scroll + visible_height {
            // Cursor below visible area - scroll down
            diff_idx.saturating_sub(visible_height) + 1
        } else {
            // Cursor already visible
            current_scroll
        }
    } else {
        // Cursor line not found in diff (shouldn't happen normally)
        current_scroll
    }
}

#[cfg(test)]
mod strip_annotation_tests {
    use super::strip_annotation;

    #[test]
    fn test_strip_annotation_rust() {
        let result = strip_annotation("code // [ANNOTATION] fix this", "//");
        assert_eq!(result, "code");
    }

    #[test]
    fn test_strip_annotation_python() {
        let result = strip_annotation("code # [ANNOTATION] fix this", "#");
        assert_eq!(result, "code");
    }

    #[test]
    fn test_strip_annotation_none() {
        let result = strip_annotation("code", "//");
        assert_eq!(result, "code");
    }

    #[test]
    fn test_strip_annotation_preserves_code() {
        let result = strip_annotation("let x = 5; // [ANNOTATION] review", "//");
        assert_eq!(result, "let x = 5;");
    }

    #[test]
    fn test_strip_annotation_markdown() {
        let result = strip_annotation("some text [ANNOTATION] fix", "");
        assert_eq!(result, "some text");
    }
}

#[cfg(test)]
mod scroll_sync_tests {
    use super::*;
    use crate::models::Line;

    /// Helper to create a simple diff result for testing
    fn create_test_diff(working_lines: &[&str], head_lines: &[&str]) -> DiffResult {
        let working: Vec<Line> = working_lines.iter().map(|s| Line {
            content: s.to_string(),
            annotation: None,
        }).collect();
        calculate_diff(&working, &head_lines.join("\n"), "//")
    }

    // =========================================================================
    // Test-only utility functions for scroll verification
    // =========================================================================

    /// Checks if a diff line index is visible given current scroll state.
    fn is_diff_line_visible(
        diff_line_idx: usize,
        scroll_offset: usize,
        visible_height: usize,
    ) -> bool {
        diff_line_idx >= scroll_offset && diff_line_idx < scroll_offset + visible_height
    }

    /// Returns the range of diff line indices that are visible (start, end exclusive).
    fn get_visible_diff_range(
        scroll_offset: usize,
        visible_height: usize,
        total_diff_lines: usize,
    ) -> (usize, usize) {
        let start = scroll_offset.min(total_diff_lines);
        let end = (scroll_offset + visible_height).min(total_diff_lines);
        (start, end)
    }

    /// Returns info about what both panes show at the current scroll position.
    struct VisiblePaneInfo {
        left_lines: Vec<Option<usize>>,  // Working line numbers (1-indexed), None for blanks
        right_lines: Vec<Option<usize>>, // HEAD line numbers (1-indexed), None for blanks
    }

    /// Get information about what lines are visible in both panes.
    fn get_visible_pane_info(
        diff_result: &DiffResult,
        scroll_offset: usize,
        visible_height: usize,
    ) -> VisiblePaneInfo {
        let (start, end) = get_visible_diff_range(scroll_offset, visible_height, diff_result.lines.len());

        let mut left_lines = Vec::with_capacity(end - start);
        let mut right_lines = Vec::with_capacity(end - start);

        for idx in start..end {
            let diff_line = &diff_result.lines[idx];
            left_lines.push(diff_line.working.as_ref().map(|(n, _, _)| *n));
            right_lines.push(diff_line.head.as_ref().map(|(n, _, _)| *n));
        }

        VisiblePaneInfo { left_lines, right_lines }
    }

    // =========================================================================
    // cursor_to_diff_index tests
    // =========================================================================

    #[test]
    fn test_cursor_to_diff_index_simple() {
        // Simple case: 3 unchanged lines
        let diff = create_test_diff(&["a", "b", "c"], &["a", "b", "c"]);

        assert_eq!(cursor_to_diff_index(&diff, 0), Some(0)); // line "a"
        assert_eq!(cursor_to_diff_index(&diff, 1), Some(1)); // line "b"
        assert_eq!(cursor_to_diff_index(&diff, 2), Some(2)); // line "c"
    }

    #[test]
    fn test_cursor_to_diff_index_with_added_line() {
        // Working has more lines than HEAD
        let diff = create_test_diff(&["a", "new", "b"], &["a", "b"]);

        // Working lines should map correctly
        assert_eq!(cursor_to_diff_index(&diff, 0), Some(0)); // "a"
        assert_eq!(cursor_to_diff_index(&diff, 1), Some(1)); // "new" (added)
        assert_eq!(cursor_to_diff_index(&diff, 2), Some(2)); // "b"
    }

    #[test]
    fn test_cursor_to_diff_index_with_removed_line() {
        // HEAD has more lines than working (line was removed)
        let diff = create_test_diff(&["a", "c"], &["a", "b", "c"]);

        // Working lines map, skipping the removed line's diff entry
        assert_eq!(cursor_to_diff_index(&diff, 0), Some(0)); // "a"
        assert_eq!(cursor_to_diff_index(&diff, 1), Some(2)); // "c" (diff_idx 2, as "b" is at idx 1)
    }

    #[test]
    fn test_cursor_to_diff_index_out_of_range() {
        let diff = create_test_diff(&["a", "b"], &["a", "b"]);

        // Cursor beyond working lines
        assert_eq!(cursor_to_diff_index(&diff, 99), None);
    }

    #[test]
    fn test_cursor_to_diff_index_empty_diff() {
        let diff = create_test_diff(&[], &[]);

        assert_eq!(cursor_to_diff_index(&diff, 0), None);
    }

    // =========================================================================
    // is_diff_line_visible tests
    // =========================================================================

    #[test]
    fn test_is_diff_line_visible_in_range() {
        // scroll_offset = 5, visible_height = 10, so visible range is 5..15
        assert!(is_diff_line_visible(5, 5, 10));  // first visible
        assert!(is_diff_line_visible(10, 5, 10)); // middle
        assert!(is_diff_line_visible(14, 5, 10)); // last visible
    }

    #[test]
    fn test_is_diff_line_visible_out_of_range() {
        // scroll_offset = 5, visible_height = 10, so visible range is 5..15
        assert!(!is_diff_line_visible(4, 5, 10));  // just above
        assert!(!is_diff_line_visible(15, 5, 10)); // just below (15 is exclusive)
        assert!(!is_diff_line_visible(0, 5, 10));  // way above
        assert!(!is_diff_line_visible(100, 5, 10)); // way below
    }

    #[test]
    fn test_is_diff_line_visible_at_start() {
        // scroll_offset = 0, visible_height = 5
        assert!(is_diff_line_visible(0, 0, 5));
        assert!(is_diff_line_visible(4, 0, 5));
        assert!(!is_diff_line_visible(5, 0, 5));
    }

    // =========================================================================
    // get_visible_diff_range tests
    // =========================================================================

    #[test]
    fn test_get_visible_diff_range_normal() {
        // 100 total lines, scroll at 20, height 10
        let (start, end) = get_visible_diff_range(20, 10, 100);
        assert_eq!(start, 20);
        assert_eq!(end, 30);
    }

    #[test]
    fn test_get_visible_diff_range_at_start() {
        let (start, end) = get_visible_diff_range(0, 10, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 10);
    }

    #[test]
    fn test_get_visible_diff_range_at_end() {
        // Scroll near end - should clamp to total
        let (start, end) = get_visible_diff_range(95, 10, 100);
        assert_eq!(start, 95);
        assert_eq!(end, 100); // Clamped
    }

    #[test]
    fn test_get_visible_diff_range_beyond_end() {
        // Scroll past end
        let (start, end) = get_visible_diff_range(150, 10, 100);
        assert_eq!(start, 100); // Clamped
        assert_eq!(end, 100);   // Clamped
    }

    #[test]
    fn test_get_visible_diff_range_small_file() {
        // File smaller than visible height
        let (start, end) = get_visible_diff_range(0, 10, 5);
        assert_eq!(start, 0);
        assert_eq!(end, 5); // Only 5 lines available
    }

    // =========================================================================
    // adjust_diff_scroll tests
    // =========================================================================

    #[test]
    fn test_adjust_scroll_cursor_visible() {
        let diff = create_test_diff(&["a", "b", "c", "d", "e"], &["a", "b", "c", "d", "e"]);

        // cursor_line=2, scroll=0, height=5 -> cursor is visible, no change
        let new_scroll = adjust_diff_scroll(2, 0, 5, &diff);
        assert_eq!(new_scroll, 0);
    }

    #[test]
    fn test_adjust_scroll_cursor_above() {
        // Create a larger diff for scrolling tests
        let lines: Vec<&str> = (0..20).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        // cursor_line=2, scroll=10, height=5 -> cursor above visible (10..15)
        let new_scroll = adjust_diff_scroll(2, 10, 5, &diff);
        assert_eq!(new_scroll, 2); // Scroll up to show cursor at top
    }

    #[test]
    fn test_adjust_scroll_cursor_below() {
        let lines: Vec<&str> = (0..20).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        // cursor_line=15, scroll=0, height=5 -> cursor below visible (0..5)
        let new_scroll = adjust_diff_scroll(15, 0, 5, &diff);
        assert_eq!(new_scroll, 11); // Scroll down to show cursor
    }

    #[test]
    fn test_adjust_scroll_with_removed_lines() {
        // HEAD has extra lines that were removed in working
        // Working: a, c, d (3 lines)
        // HEAD: a, b, c, d (4 lines)
        // Diff will have 4 entries, with "b" only on right side
        let diff = create_test_diff(&["a", "c", "d"], &["a", "b", "c", "d"]);

        // cursor_line=2 (pointing to "d" in working) should map to diff_idx 3
        let diff_idx = cursor_to_diff_index(&diff, 2);
        assert_eq!(diff_idx, Some(3));

        // With scroll=0, height=3, visible range is 0..3, cursor at 3 is not visible
        let new_scroll = adjust_diff_scroll(2, 0, 3, &diff);
        assert_eq!(new_scroll, 1); // Scroll to make diff_idx 3 visible at bottom
    }

    // =========================================================================
    // get_visible_pane_info tests - verify both panes show same diff indices
    // =========================================================================

    #[test]
    fn test_pane_info_synchronized_unchanged() {
        let diff = create_test_diff(&["a", "b", "c"], &["a", "b", "c"]);

        let info = get_visible_pane_info(&diff, 0, 3);

        // Both panes should show same line numbers for unchanged content
        assert_eq!(info.left_lines, vec![Some(1), Some(2), Some(3)]);
        assert_eq!(info.right_lines, vec![Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn test_pane_info_synchronized_with_added() {
        // Working: a, new, b | HEAD: a, b
        let diff = create_test_diff(&["a", "new", "b"], &["a", "b"]);

        let info = get_visible_pane_info(&diff, 0, 10);

        // Left pane has all working lines
        assert!(info.left_lines.contains(&Some(1))); // "a"
        assert!(info.left_lines.contains(&Some(2))); // "new"
        assert!(info.left_lines.contains(&Some(3))); // "b"

        // Right pane has HEAD lines, with None for added line position
        assert!(info.right_lines.contains(&Some(1))); // "a"
        assert!(info.right_lines.contains(&Some(2))); // "b"
        // There should be a None in right_lines where "new" appears in left
    }

    #[test]
    fn test_pane_info_synchronized_with_removed() {
        // Working: a, c | HEAD: a, b, c
        let diff = create_test_diff(&["a", "c"], &["a", "b", "c"]);

        let info = get_visible_pane_info(&diff, 0, 10);

        // Left pane has working lines, with None for removed line position
        assert!(info.left_lines.contains(&Some(1))); // "a"
        assert!(info.left_lines.contains(&Some(2))); // "c"

        // Right pane has all HEAD lines
        assert!(info.right_lines.contains(&Some(1))); // "a"
        assert!(info.right_lines.contains(&Some(2))); // "b"
        assert!(info.right_lines.contains(&Some(3))); // "c"

        // There should be a None in left_lines where "b" appears in right
        assert!(info.left_lines.iter().any(|l| l.is_none()));
    }

    #[test]
    fn test_pane_info_scrolled() {
        let lines: Vec<&str> = (0..10).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        // Scroll to middle, view 3 lines
        let info = get_visible_pane_info(&diff, 5, 3);

        // Should show lines 6, 7, 8 (1-indexed)
        assert_eq!(info.left_lines, vec![Some(6), Some(7), Some(8)]);
        assert_eq!(info.right_lines, vec![Some(6), Some(7), Some(8)]);
    }

    #[test]
    fn test_pane_info_both_panes_same_length() {
        // Regardless of content, both panes should always have same length
        let diff = create_test_diff(&["a", "new1", "new2", "b"], &["a", "old", "b"]);

        let info = get_visible_pane_info(&diff, 0, 20);

        assert_eq!(
            info.left_lines.len(),
            info.right_lines.len(),
            "Both panes must show same number of rows (scroll sync)"
        );
    }

    // =========================================================================
    // Integration-style scroll tests
    // =========================================================================

    #[test]
    fn test_scroll_sequence_down() {
        // Simulate scrolling down through a file
        let lines: Vec<&str> = (0..20).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        let visible_height = 5;
        let mut scroll = 0;

        // Move cursor from 0 to 10, checking scroll adjusts
        for cursor in 0..=10 {
            scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

            // Verify cursor is now visible
            let diff_idx = cursor_to_diff_index(&diff, cursor).unwrap();
            assert!(
                is_diff_line_visible(diff_idx, scroll, visible_height),
                "Cursor {} (diff_idx {}) should be visible with scroll {}",
                cursor, diff_idx, scroll
            );
        }
    }

    #[test]
    fn test_scroll_sequence_up() {
        // Simulate scrolling up through a file
        let lines: Vec<&str> = (0..20).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        let visible_height = 5;
        let mut scroll = 15; // Start scrolled down

        // Move cursor from 19 down to 5
        for cursor in (5..=19).rev() {
            scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

            // Verify cursor is now visible
            let diff_idx = cursor_to_diff_index(&diff, cursor).unwrap();
            assert!(
                is_diff_line_visible(diff_idx, scroll, visible_height),
                "Cursor {} (diff_idx {}) should be visible with scroll {}",
                cursor, diff_idx, scroll
            );
        }
    }

    #[test]
    fn test_page_navigation() {
        let lines: Vec<&str> = (0..50).map(|_| "line").collect();
        let diff = create_test_diff(&lines, &lines);

        let visible_height = 10;
        let mut scroll = 0;
        let mut cursor = 0;

        // Page down
        cursor = (cursor + visible_height).min(49);
        scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

        assert!(is_diff_line_visible(
            cursor_to_diff_index(&diff, cursor).unwrap(),
            scroll,
            visible_height
        ));

        // Page down again
        cursor = (cursor + visible_height).min(49);
        scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

        assert!(is_diff_line_visible(
            cursor_to_diff_index(&diff, cursor).unwrap(),
            scroll,
            visible_height
        ));

        // Page up
        cursor = cursor.saturating_sub(visible_height);
        scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

        assert!(is_diff_line_visible(
            cursor_to_diff_index(&diff, cursor).unwrap(),
            scroll,
            visible_height
        ));
    }

    #[test]
    fn test_scroll_sync_with_asymmetric_diff() {
        // Complex case: Working has some additions, HEAD has some that were removed
        // Working: a, new1, b, c, new2, d
        // HEAD: a, b, old, c, d
        let diff = create_test_diff(
            &["a", "new1", "b", "c", "new2", "d"],
            &["a", "b", "old", "c", "d"],
        );

        let visible_height = 3;

        // Navigate through all working lines, verify each stays visible
        let mut scroll = 0;
        for cursor in 0..6 {
            scroll = adjust_diff_scroll(cursor, scroll, visible_height, &diff);

            if let Some(diff_idx) = cursor_to_diff_index(&diff, cursor) {
                assert!(
                    is_diff_line_visible(diff_idx, scroll, visible_height),
                    "Cursor {} should be visible after scroll adjustment",
                    cursor
                );

                // Verify both panes show the same diff line range
                let info = get_visible_pane_info(&diff, scroll, visible_height);
                assert_eq!(
                    info.left_lines.len(),
                    info.right_lines.len(),
                    "Panes must stay synchronized at cursor {}", cursor
                );
            }
        }
    }
}
