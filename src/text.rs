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
        // Should return as-is when leading whitespace is wider than available width
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
}
