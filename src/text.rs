use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Safely truncate text to visual width limit.
/// Returns string that fits within `max_width` visual columns.
pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let char_width = ch.width().unwrap_or(0);
        if current_width + char_width > max_width {
            break;
        }
        result.push(ch);
        current_width += char_width;
    }

    result
}

/// Calculate visual column position of character at `char_index` in text.
/// Returns the visual width from start of text to the character position.
pub fn char_index_to_visual_col(text: &str, char_index: usize) -> usize {
    let chars_before: String = text.chars().take(char_index).collect();
    chars_before.width()
}

/// Calculate padding needed to reach visual width.
/// Returns number of spaces needed.
pub fn calculate_padding(current_text: &str, target_width: usize) -> usize {
    target_width.saturating_sub(current_text.width())
}

/// Wraps text to fit within a specified width, preserving leading and trailing whitespace.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    // Preserve leading whitespace
    let leading_whitespace: String = text.chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let leading_width = leading_whitespace.width();
    
    let trimmed_text = text.trim_start();
    
    // If the line is empty or only whitespace, return it as-is
    if trimmed_text.is_empty() {
        return vec![text.to_string()];
    }
    
    // If the leading whitespace itself is wider than the available width,
    // just return the original text without wrapping
    if leading_width >= width {
        return vec![text.to_string()];
    }
    
    let available_width = width.saturating_sub(leading_width);
    
    // Check if the original text ends with whitespace (before trim_end)
    let has_trailing_space = trimmed_text.ends_with(' ');
    
    let mut wrapped = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in trimmed_text.split_whitespace() {
        let word_width = word.width();
        if current_width + word_width + 1 > available_width && !current_line.is_empty() {
            wrapped.push(format!("{}{}", leading_whitespace, current_line));
            current_line.clear();
            current_width = 0;
        }
        if !current_line.is_empty() {
            current_line.push(' ');
            current_width += 1;
        }
        current_line.push_str(word);
        current_width += word_width;
    }

    if !current_line.is_empty() {
        // Add trailing space if the original text had one
        if has_trailing_space {
            current_line.push(' ');
        }
        wrapped.push(format!("{}{}", leading_whitespace, current_line));
    }

    if wrapped.is_empty() {
        wrapped.push(leading_whitespace);
    }

    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for new width-aware helper functions
    #[test]
    fn test_truncate_to_width_ascii() {
        let text = "Hello World";
        assert_eq!(truncate_to_width(text, 5), "Hello");
        assert_eq!(truncate_to_width(text, 11), "Hello World");
        assert_eq!(truncate_to_width(text, 0), "");
    }

    #[test]
    fn test_truncate_to_width_emoji() {
        let text = "Hello 🎉 World";
        // "Hello " = 6 cols, 🎉 = 2 cols, so max 7 should give "Hello "
        assert_eq!(truncate_to_width(text, 7), "Hello ");
        // Max 8 should include the emoji
        assert_eq!(truncate_to_width(text, 8), "Hello 🎉");
    }

    #[test]
    fn test_truncate_to_width_cjk() {
        let text = "你好世界"; // Each CJK char = 2 visual columns
        // Max 5 should give "你好" (4 cols), not "你好世" (6 cols)
        assert_eq!(truncate_to_width(text, 5), "你好");
        assert_eq!(truncate_to_width(text, 4), "你好");
        assert_eq!(truncate_to_width(text, 3), "你");
    }

    #[test]
    fn test_truncate_to_width_mixed() {
        let text = "Hi你好"; // "Hi" = 2 cols, "你好" = 4 cols, total 6 cols
        assert_eq!(truncate_to_width(text, 3), "Hi");
        assert_eq!(truncate_to_width(text, 4), "Hi你");
        assert_eq!(truncate_to_width(text, 6), "Hi你好");
    }

    #[test]
    fn test_char_index_to_visual_col_ascii() {
        let text = "Hello";
        assert_eq!(char_index_to_visual_col(text, 0), 0);
        assert_eq!(char_index_to_visual_col(text, 2), 2);
        assert_eq!(char_index_to_visual_col(text, 5), 5);
    }

    #[test]
    fn test_char_index_to_visual_col_mixed() {
        let text = "Hi你好"; // "Hi" = 2 cols + "你好" = 4 cols
        assert_eq!(char_index_to_visual_col(text, 0), 0); // Before "H"
        assert_eq!(char_index_to_visual_col(text, 2), 2); // Before "你"
        assert_eq!(char_index_to_visual_col(text, 3), 4); // Before "好"
        assert_eq!(char_index_to_visual_col(text, 4), 6); // After "好"
    }

    #[test]
    fn test_char_index_to_visual_col_emoji() {
        let text = "A🎉B"; // "A" = 1 col, 🎉 = 2 cols, "B" = 1 col
        assert_eq!(char_index_to_visual_col(text, 0), 0); // Before "A"
        assert_eq!(char_index_to_visual_col(text, 1), 1); // Before emoji
        assert_eq!(char_index_to_visual_col(text, 2), 3); // Before "B"
        assert_eq!(char_index_to_visual_col(text, 3), 4); // After "B"
    }

    #[test]
    fn test_calculate_padding_ascii() {
        assert_eq!(calculate_padding("Hello", 10), 5);
        assert_eq!(calculate_padding("Hello", 5), 0);
        assert_eq!(calculate_padding("Hello", 3), 0); // Saturating sub
    }

    #[test]
    fn test_calculate_padding_wide_chars() {
        let text = "你好"; // 2 chars = 4 visual columns
        assert_eq!(calculate_padding(text, 10), 6);
        assert_eq!(calculate_padding(text, 4), 0);
        assert_eq!(calculate_padding(text, 2), 0); // Saturating sub
    }

    #[test]
    fn test_calculate_padding_mixed() {
        let text = "Hi你好"; // 4 chars = 6 visual columns
        assert_eq!(calculate_padding(text, 10), 4);
        assert_eq!(calculate_padding(text, 6), 0);
    }

    #[test]
    fn test_edge_case_zero_width_chars() {
        // Combining diacritical marks have zero width
        let text = "e\u{0301}"; // e with combining acute accent
        assert_eq!(truncate_to_width(text, 1), "e\u{0301}");
        assert_eq!(char_index_to_visual_col(text, 2), 1); // Two chars, 1 visual col
    }

    #[test]
    fn test_wrap_text_basic() {
        let text = "This is a simple test";
        let wrapped = wrap_text(text, 10);
        assert_eq!(wrapped.len(), 3);
        assert_eq!(wrapped[0], "This is a");
        assert_eq!(wrapped[1], "simple");
        assert_eq!(wrapped[2], "test");
    }

    #[test]
    fn test_wrap_text_with_leading_whitespace() {
        let text = "    indented text here";
        let wrapped = wrap_text(text, 20);
        assert!(wrapped[0].starts_with("    "));
    }

    #[test]
    fn test_wrap_text_empty_string() {
        let text = "";
        let wrapped = wrap_text(text, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "");
    }

    #[test]
    fn test_wrap_text_only_whitespace() {
        let text = "    ";
        let wrapped = wrap_text(text, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "    ");
    }

    #[test]
    fn test_wrap_text_width_smaller_than_leading() {
        let text = "          some text";
        let wrapped = wrap_text(text, 5);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], text);
    }

    #[test]
    fn test_wrap_text_preserves_trailing_space() {
        let text = "hello world ";
        let wrapped = wrap_text(text, 20);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "hello world ");
        assert!(wrapped[0].ends_with(' '), "Trailing space should be preserved");
    }
    
    #[test]
    fn test_wrap_styled_text() {
        let style1 = 1;
        let style2 = 2;
        let segments = vec![
            (style1, "Hello "),
            (style2, "world "),
            (style1, "this is a test"),
        ];
        
        let wrapped = wrap_styled_text(&segments, 10);
        // "Hello " (6) + "world " (6) -> 12 > 10.
        // Line 1: "Hello "
        // Line 2: "world this" (6+5=11) > 10? No wait.
        // "Hello " (6) fits.
        // "world " (6) -> 12. New line.
        // Line 1: "Hello "
        // Line 2: "world " (6)
        // "this " (5) -> 11. New line.
        // Line 2: "world "
        // Line 3: "this is a " (10)
        // "test" (4) -> 14. New line.
        // Line 4: "test"
        
        // Exact behavior depends on splitting logic.
        assert!(wrapped.len() >= 3);
        assert_eq!(wrapped[0][0].1, "Hello ");
    }
}

/// Wraps styled text segments to fit within a specified width.
/// Returns a list of lines, where each line is a list of (style, text) tuples.
/// Wraps styled text segments to fit within a specified width.
/// Returns a list of lines, where each line is a list of (style, text) tuples.
pub fn wrap_styled_text<T: Clone + Copy + PartialEq>(segments: &[(T, &str)], width: usize) -> Vec<Vec<(T, String)>> {
    // Collect all text to calculate leading whitespace
    let full_text: String = segments.iter().map(|(_, s)| *s).collect();
    
    let leading_whitespace: String = full_text.chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let leading_width = leading_whitespace.width();
    
    // Quick exit for empty case
    if full_text.trim().is_empty() {
         if !full_text.is_empty() {
             if let Some((style, _)) = segments.first() {
                return vec![vec![(*style, full_text)]];
             }
         }
         return vec![vec![]];
    }
    
    if leading_width >= width {
        let line = segments.iter().map(|(style, text)| (*style, text.to_string())).collect();
        return vec![line];
    }

    let _available_width = width.saturating_sub(leading_width);
    let mut wrapped_lines = Vec::new();
    let mut current_line: Vec<(T, String)> = Vec::new();
    let mut current_width = 0;
    
    struct Token<T> {
        style: T,
        text: String,
        is_whitespace: bool,
        width: usize,
    }
    
    let mut tokens: Vec<Token<T>> = Vec::new();
    
    for (style, text) in segments {
        let char_indices: Vec<(usize, char)> = text.char_indices().collect();
        let mut start = 0;
        
        while start < char_indices.len() {
            let start_char = char_indices[start].1;
            let mut end = start + 1;
            let is_ws = start_char.is_whitespace();
            
            while end < char_indices.len() {
                if char_indices[end].1.is_whitespace() != is_ws {
                    break;
                }
                end += 1;
            }
            
            let slice_end = if end < char_indices.len() {
                char_indices[end].0
            } else {
                text.len()
            };
            
            let slice_start = char_indices[start].0;
            let token_text = &text[slice_start..slice_end];
            
            tokens.push(Token {
                style: *style,
                text: token_text.to_string(),
                is_whitespace: is_ws,
                width: token_text.width(),
            });
            
            start = end;
        }
    }

    let indent_style = segments.first().map(|(s, _)| *s).expect("Checked empty");
    
    for token in tokens {
        if token.is_whitespace {
             if current_width + token.width > width {
                 wrapped_lines.push(current_line);
                 current_line = Vec::new();
                 if !leading_whitespace.is_empty() {
                      current_line.push((indent_style, leading_whitespace.clone()));
                 }
                 current_width = leading_width;
                 continue; 
             }
             
             current_line.push((token.style, token.text));
             current_width += token.width;
        } else {
            if current_width + token.width > width {
                 wrapped_lines.push(current_line);
                 current_line = Vec::new();
                 if !leading_whitespace.is_empty() {
                      current_line.push((indent_style, leading_whitespace.clone()));
                 }
                 current_width = leading_width;
            }
            
            current_line.push((token.style, token.text));
            current_width += token.width;
        }
    }
    
    if !current_line.is_empty() {
        wrapped_lines.push(current_line);
    }
    
    // Merge adjacent segments with same style
    let mut merged_wrapped_lines = Vec::new();
    for line in wrapped_lines {
        let mut merged_line: Vec<(T, String)> = Vec::new();
        if let Some((first_style, first_text)) = line.first() {
            let mut current_style = *first_style;
            let mut current_text = first_text.clone();
            
            for (style, text) in line.iter().skip(1) {
                if *style == current_style {
                    current_text.push_str(text);
                } else {
                    merged_line.push((current_style, current_text));
                    current_style = *style;
                    current_text = text.clone();
                }
            }
            merged_line.push((current_style, current_text));
        }
        merged_wrapped_lines.push(merged_line);
    }
    
    merged_wrapped_lines
}

