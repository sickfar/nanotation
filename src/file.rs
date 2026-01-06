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
    while i < raw_lines.len() {
        let line = raw_lines[i];
        if line.trim().starts_with(&annotation_marker) {
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
                    annotation: None,
                });
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
}
