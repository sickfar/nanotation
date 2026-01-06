use crate::models::Line;
use std::fs;
use std::io;

/// Detects the appropriate comment style based on file extension.
pub fn detect_comment_style(path: &str) -> String {
    let ext = path.split('.').last().unwrap_or("");
    match ext {
        "rs" | "go" | "java" | "kt" | "js" | "ts" | "c" | "cpp" | "h" | "cs" | "php" | "scala" | "dart" | "swift" => "//",
        "py" | "sh" | "rb" | "yaml" | "yml" | "toml" | "pl" | "r" | "dockerfile" => "#",
        "sql" | "lua" | "hs" | "ada" => "--",
        "md" => "",
        _ => {
            if path.ends_with("Dockerfile") {
                "#"
            } else {
                "//"
            }
        },
    }.to_string()
}

/// Parses file content into lines with optional annotations.
pub fn parse_file(content: &str, comment: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let raw_lines: Vec<&str> = content.lines().collect();
    let annotation_marker = if comment.is_empty() {
        "[ANNOTATION]".to_string()
    } else {
        format!("{} [ANNOTATION]", comment)
    };

    let mut i = 0;
    let mut in_code_block = false;
    let is_markdown = comment.is_empty();

    while i < raw_lines.len() {
        let line = raw_lines[i];
        
        // Handle code block toggling for markdown
        if is_markdown && line.trim().starts_with("```") {
            in_code_block = !in_code_block;
        }

        if !in_code_block && line.trim().starts_with(&annotation_marker) {
            let annotation_text = line.trim()
                .strip_prefix(&annotation_marker)
                .unwrap_or("")
                .trim()
                .to_string();
            
            if i + 1 < raw_lines.len() {
                lines.push(Line {
                    content: raw_lines[i + 1].to_string(),
                    annotation: Some(annotation_text),
                });
                i += 2;
            } else {
                lines.push(Line {
                    content: line.to_string(),
                    annotation: None, // Or keep it as content? Logic below used None for end of file annotation mismatch
                });
                // Actually, looking at original logic:
                // If it's the last line and looks like an annotation, it consumes it but with no attached content?
                // Original: 
                // lines.push(Line { content: line.to_string(), annotation: None }); 
                // i += 1;
                // Since this block is "if it STARTS with annotation marker", 
                // The original logic for the `else` (last line) was: treat it as a normal line because it has no following line to attach to.
                // But wait, the original logic for `else` was finding the marker, then pushing `line` as content.
                // Meaning it WASN'T treated as an annotation if it was the last line?
                // Let's stick to the original behavior for that edge case, but wrapped in `!in_code_block`.
                i += 1;
            }
        } else {
            lines.push(Line {
                content: line.to_string(),
                annotation: None,
            });
            i += 1;
        }
    }

    if lines.is_empty() {
        lines.push(Line { content: String::new(), annotation: None });
    }

    lines
}

/// Saves lines with annotations to a file.
pub fn save_file(path: &str, lines: &[Line], lang_comment: &str) -> io::Result<()> {
    let mut output = String::new();
    let annotation_marker = if lang_comment.is_empty() {
        "[ANNOTATION]".to_string()
    } else {
        format!("{} [ANNOTATION]", lang_comment)
    };

    for line in lines {
        if let Some(ref annotation) = line.annotation {
            output.push_str(&format!("{} {}\n", annotation_marker, annotation));
        }
        output.push_str(&line.content);
        output.push('\n');
    }

    fs::write(path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_comment_style_rust() {
        assert_eq!(detect_comment_style("main.rs"), "//");
    }

    #[test]
    fn test_detect_comment_style_python() {
        assert_eq!(detect_comment_style("script.py"), "#");
    }

    #[test]
    fn test_detect_comment_style_markdown() {
        assert_eq!(detect_comment_style("README.md"), "");
    }

    #[test]
    fn test_detect_comment_style_unknown() {
        assert_eq!(detect_comment_style("file.xyz"), "//");
    }

    #[test]
    fn test_detect_comment_style_sql() {
        assert_eq!(detect_comment_style("query.sql"), "--");
    }

    #[test]
    fn test_detect_comment_style_docker() {
        assert_eq!(detect_comment_style("Dockerfile"), "#");
        assert_eq!(detect_comment_style("dev.dockerfile"), "#");
    }

    #[test]
    fn test_detect_comment_style_config() {
        assert_eq!(detect_comment_style("config.yaml"), "#");
        assert_eq!(detect_comment_style("Cargo.toml"), "#");
    }

    #[test]
    fn test_detect_comment_style_swift() {
        assert_eq!(detect_comment_style("App.swift"), "//");
    }

    #[test]
    fn test_parse_file_without_annotations() {
        let content = "fn main() {\n    println!(\"Hello\");\n}";
        let lines = parse_file(content, "//");
        assert_eq!(lines.len(), 3);
        assert!(lines[0].annotation.is_none());
        assert_eq!(lines[0].content, "fn main() {");
    }

    #[test]
    fn test_parse_file_with_annotations() {
        let content = "// [ANNOTATION] This is a comment\nfn main() {\n    println!(\"Hello\");\n}";
        let lines = parse_file(content, "//");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].annotation, Some("This is a comment".to_string()));
        assert_eq!(lines[0].content, "fn main() {");
    }

    #[test]
    fn test_parse_file_empty() {
        let content = "";
        let lines = parse_file(content, "//");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "");
    }

    #[test]
    fn test_parse_save_roundtrip() {
        let temp_path = "/tmp/test_nanotation_roundtrip.txt";
        let original_content = "// [ANNOTATION] Test annotation\nlet x = 5;\nlet y = 10;";
        
        // Write original content
        fs::write(temp_path, original_content).unwrap();
        
        // Parse
        let content = fs::read_to_string(temp_path).unwrap();
        let lines = parse_file(&content, "//");
        
        // Save
        save_file(temp_path, &lines, "//").unwrap();
        
        // Read back
        let saved_content = fs::read_to_string(temp_path).unwrap();
        let lines2 = parse_file(&saved_content, "//");
        
        // Verify
        assert_eq!(lines.len(), lines2.len());
        assert_eq!(lines[0].annotation, lines2[0].annotation);
        assert_eq!(lines[0].content, lines2[0].content);
        
        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_parse_markdown_code_block() {
        let content = "Normal text\n```\n[ANNOTATION] This should be ignored\n```\nTarget line";
        let lines = parse_file(content, "");
        
        // Line 0: Normal text
        assert_eq!(lines[0].content, "Normal text");
        assert_eq!(lines[0].annotation, None);
        
        // Line 1: ```
        assert_eq!(lines[1].content, "```");
        assert_eq!(lines[1].annotation, None);
        
        // Line 2: [ANNOTATION] ... 
        // Should be treated as content because it's in a code block
        assert_eq!(lines[2].content, "[ANNOTATION] This should be ignored");
        assert_eq!(lines[2].annotation, None);
        
        // Line 3: ```
        assert_eq!(lines[3].content, "```");
        
        // Line 4: Target line
        assert_eq!(lines[4].content, "Target line");
    }

    #[test]
    fn test_parse_markdown_mixed() {
        let content = "```\n[ANNOTATION] ignore me\n```\n[ANNOTATION] valid\nTarget";
        let lines = parse_file(content, "");
        
        // Lines 0-2: code block
        assert_eq!(lines[1].content, "[ANNOTATION] ignore me");
        assert_eq!(lines[1].annotation, None);
        
        // Line 3 (was 4 in raw): Target with annotation
        // "```" is line index 2.
        // "[ANNOTATION] valid" is line index 3 (raw), but consumed.
        // "Target" is line index 4 (raw).
        // So lines vector:
        // 0: ```
        // 1: [ANNOTATION] ignore me
        // 2: ```
        // 3: Target (with annotation)
        
        assert_eq!(lines[3].content, "Target");
        assert_eq!(lines[3].annotation, Some("valid".to_string()));
    }
}
