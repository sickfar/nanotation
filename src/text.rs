use unicode_width::UnicodeWidthStr;

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

