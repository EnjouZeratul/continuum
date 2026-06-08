//! # Text Processing Tools
//!
//! 文本处理工具集：统计、转换、分割等。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;

// ============================================================================
// Count Lines Tool
// ============================================================================

/// 行数统计工具
pub struct CountLinesTool;

#[async_trait]
impl BuiltinTool for CountLinesTool {
    fn name(&self) -> &str {
        "count_lines"
    }

    fn description(&self) -> &str {
        "Count lines, words, and characters in text."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to analyze"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let lines = text.lines().count();
        let words = text.split_whitespace().count();
        let chars = text.chars().count();
        let bytes = text.len();

        Ok(format!(
            "Lines: {}\nWords: {}\nCharacters: {}\nBytes: {}",
            lines, words, chars, bytes
        ))
    }
}

// ============================================================================
// Word Frequency Tool
// ============================================================================

/// 词频统计工具
pub struct WordFrequencyTool;

#[async_trait]
impl BuiltinTool for WordFrequencyTool {
    fn name(&self) -> &str {
        "word_frequency"
    }

    fn description(&self) -> &str {
        "Count word frequency in text."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to analyze"
                },
                "top": {
                    "type": "integer",
                    "description": "Number of top words to return (default: 10)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case sensitive counting (default: false)"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let top = args["top"].as_u64().unwrap_or(10) as usize;
        let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(false);

        use std::collections::HashMap;
        let mut freq: HashMap<String, usize> = HashMap::new();

        for word in text.split_whitespace() {
            let word = if case_sensitive {
                word.to_string()
            } else {
                word.to_lowercase()
            };
            // Remove punctuation
            let word: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
            if !word.is_empty() {
                *freq.entry(word).or_insert(0) += 1;
            }
        }

        let mut freq_vec: Vec<_> = freq.into_iter().collect();
        freq_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

        let result: Vec<String> = freq_vec
            .iter()
            .take(top)
            .map(|(word, count)| format!("{}: {}", word, count))
            .collect();

        Ok(result.join("\n"))
    }
}

// ============================================================================
// Text Transform Tool
// ============================================================================

/// 文本转换工具
pub struct TextTransformTool;

#[async_trait]
impl BuiltinTool for TextTransformTool {
    fn name(&self) -> &str {
        "text_transform"
    }

    fn description(&self) -> &str {
        "Transform text case: uppercase, lowercase, title, snake_case, camelCase, etc."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to transform"
                },
                "transform": {
                    "type": "string",
                    "enum": ["uppercase", "lowercase", "title", "capitalize", "snake_case", "camelCase", "PascalCase", "kebab-case", "reverse"],
                    "description": "Transform to apply"
                }
            },
            "required": ["text", "transform"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let transform = args["transform"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing transform parameter"))?;

        let result = match transform {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "title" => {
                let mut result = String::new();
                let mut capitalize_next = true;
                for c in text.chars() {
                    if c.is_whitespace() {
                        capitalize_next = true;
                        result.push(c);
                    } else if capitalize_next {
                        result.push(c.to_uppercase().next().unwrap_or(c));
                        capitalize_next = false;
                    } else {
                        result.push(c);
                    }
                }
                result
            }
            "capitalize" => {
                let mut chars = text.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
            "snake_case" => {
                let mut result = String::new();
                for (i, c) in text.chars().enumerate() {
                    if c.is_uppercase() {
                        if i > 0 {
                            result.push('_');
                        }
                        result.push(c.to_lowercase().next().unwrap_or(c));
                    } else if c == ' ' || c == '-' {
                        result.push('_');
                    } else {
                        result.push(c);
                    }
                }
                result.to_lowercase()
            }
            "camelCase" => {
                let mut result = String::new();
                let mut capitalize_next = false;
                for c in text.chars() {
                    if c == '_' || c == ' ' || c == '-' {
                        capitalize_next = true;
                    } else if capitalize_next {
                        result.push(c.to_uppercase().next().unwrap_or(c));
                        capitalize_next = false;
                    } else {
                        result.push(c);
                    }
                }
                result
            }
            "PascalCase" => {
                let camel = {
                    let mut result = String::new();
                    let mut capitalize_next = true;
                    for c in text.chars() {
                        if c == '_' || c == ' ' || c == '-' {
                            capitalize_next = true;
                        } else if capitalize_next {
                            result.push(c.to_uppercase().next().unwrap_or(c));
                            capitalize_next = false;
                        } else {
                            result.push(c);
                        }
                    }
                    result
                };
                camel
            }
            "kebab-case" => {
                let mut result = String::new();
                for (i, c) in text.chars().enumerate() {
                    if c.is_uppercase() {
                        if i > 0 {
                            result.push('-');
                        }
                        result.push(c.to_lowercase().next().unwrap_or(c));
                    } else if c == ' ' || c == '_' {
                        result.push('-');
                    } else {
                        result.push(c);
                    }
                }
                result.to_lowercase()
            }
            "reverse" => text.chars().rev().collect(),
            _ => return Err(anyhow::anyhow!("Unknown transform: {}", transform)),
        };

        Ok(result)
    }
}

// ============================================================================
// Text Split Tool
// ============================================================================

/// 文本分割工具
pub struct TextSplitTool;

#[async_trait]
impl BuiltinTool for TextSplitTool {
    fn name(&self) -> &str {
        "text_split"
    }

    fn description(&self) -> &str {
        "Split text by delimiter, line count, or character count."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to split"
                },
                "method": {
                    "type": "string",
                    "enum": ["delimiter", "lines", "chars"],
                    "description": "Split method"
                },
                "delimiter": {
                    "type": "string",
                    "description": "Delimiter for 'delimiter' method (default: whitespace)"
                },
                "count": {
                    "type": "integer",
                    "description": "Chunk count/size for 'lines' or 'chars' method"
                }
            },
            "required": ["text", "method"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let method = args["method"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing method parameter"))?;

        let result = match method {
            "delimiter" => {
                let delimiter = args["delimiter"].as_str().unwrap_or(" ");
                let parts: Vec<&str> = text.split(delimiter).collect();
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, part)| format!("{}: {}", i + 1, part))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            "lines" => {
                let count = args["count"].as_u64().unwrap_or(10) as usize;
                let lines: Vec<&str> = text.lines().collect();
                let chunk_size = lines.len().div_ceil(count);
                lines
                    .chunks(chunk_size.max(1))
                    .enumerate()
                    .map(|(i, chunk)| format!("--- Chunk {} ---\n{}", i + 1, chunk.join("\n")))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
            "chars" => {
                let count = args["count"].as_u64().unwrap_or(100) as usize;
                text.as_bytes()
                    .chunks(count.max(1))
                    .enumerate()
                    .map(|(i, chunk)| {
                        format!(
                            "--- Chunk {} ({} chars) ---\n{}",
                            i + 1,
                            chunk.len(),
                            String::from_utf8_lossy(chunk)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
            _ => return Err(anyhow::anyhow!("Unknown method: {}", method)),
        };

        Ok(result)
    }
}

// ============================================================================
// Regex Match Tool
// ============================================================================

/// 正则匹配工具
pub struct RegexMatchTool;

#[async_trait]
impl BuiltinTool for RegexMatchTool {
    fn name(&self) -> &str {
        "regex_match"
    }

    fn description(&self) -> &str {
        "Match a regex pattern against text and return matches."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to search"
                },
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern"
                },
                "group": {
                    "type": "integer",
                    "description": "Capture group to return (default: 0 for full match)"
                }
            },
            "required": ["text", "pattern"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing pattern parameter"))?;

        let group = args["group"].as_u64().unwrap_or(0) as usize;

        let re = regex::Regex::new(pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;

        let matches: Vec<String> = re
            .captures_iter(text)
            .filter_map(|cap| cap.get(group).map(|m| m.as_str().to_string()))
            .collect();

        if matches.is_empty() {
            Ok("No matches found".to_string())
        } else {
            Ok(format!(
                "Found {} matches:\n{}",
                matches.len(),
                matches.join("\n")
            ))
        }
    }
}

// ============================================================================
// Text Diff Tool
// ============================================================================

/// 文本差异工具
pub struct TextDiffTool;

#[async_trait]
impl BuiltinTool for TextDiffTool {
    fn name(&self) -> &str {
        "text_diff"
    }

    fn description(&self) -> &str {
        "Compare two texts and show differences."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text1": {
                    "type": "string",
                    "description": "First text"
                },
                "text2": {
                    "type": "string",
                    "description": "Second text"
                }
            },
            "required": ["text1", "text2"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text1 = args["text1"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text1 parameter"))?;

        let text2 = args["text2"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text2 parameter"))?;

        let lines1: Vec<&str> = text1.lines().collect();
        let lines2: Vec<&str> = text2.lines().collect();

        // Simple line-by-line diff
        let mut result = Vec::new();
        let max_lines = lines1.len().max(lines2.len());

        for i in 0..max_lines {
            let line1 = lines1.get(i).copied().unwrap_or("");
            let line2 = lines2.get(i).copied().unwrap_or("");

            if line1 != line2 {
                if i < lines1.len() {
                    result.push(format!("- {}: {}", i + 1, line1));
                }
                if i < lines2.len() {
                    result.push(format!("+ {}: {}", i + 1, line2));
                }
            }
        }

        if result.is_empty() {
            Ok("No differences found".to_string())
        } else {
            Ok(format!("Differences:\n{}", result.join("\n")))
        }
    }
}

// ============================================================================
// Sort Lines Tool
// ============================================================================

/// 行排序工具
pub struct SortLinesTool;

#[async_trait]
impl BuiltinTool for SortLinesTool {
    fn name(&self) -> &str {
        "sort_lines"
    }

    fn description(&self) -> &str {
        "Sort lines of text alphabetically or numerically."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to sort"
                },
                "reverse": {
                    "type": "boolean",
                    "description": "Sort in reverse order (default: false)"
                },
                "numeric": {
                    "type": "boolean",
                    "description": "Sort numerically (default: false)"
                },
                "unique": {
                    "type": "boolean",
                    "description": "Remove duplicate lines (default: false)"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TextProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let reverse = args["reverse"].as_bool().unwrap_or(false);
        let numeric = args["numeric"].as_bool().unwrap_or(false);
        let unique = args["unique"].as_bool().unwrap_or(false);

        let mut lines: Vec<&str> = text.lines().collect();

        if numeric {
            lines.sort_by(|a, b| {
                let a_num = a.trim().parse::<f64>().unwrap_or(f64::NEG_INFINITY);
                let b_num = b.trim().parse::<f64>().unwrap_or(f64::NEG_INFINITY);
                a_num
                    .partial_cmp(&b_num)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            lines.sort();
        }

        if reverse {
            lines.reverse();
        }

        if unique {
            lines.dedup();
        }

        Ok(lines.join("\n"))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_count_lines() {
        let tool = CountLinesTool;
        let result = tool
            .execute(json!({"text": "Hello\nWorld\nTest"}))
            .await
            .unwrap();
        assert!(result.contains("Lines: 3"));
        assert!(result.contains("Words: 3"));
    }

    #[tokio::test]
    async fn test_word_frequency() {
        let tool = WordFrequencyTool;
        let result = tool
            .execute(json!({"text": "hello world hello test hello", "top": 2}))
            .await
            .unwrap();
        assert!(result.contains("hello: 3"));
    }

    #[tokio::test]
    async fn test_text_transform_uppercase() {
        let tool = TextTransformTool;
        let result = tool
            .execute(json!({"text": "hello", "transform": "uppercase"}))
            .await
            .unwrap();
        assert_eq!(result, "HELLO");
    }

    #[tokio::test]
    async fn test_text_transform_snake_case() {
        let tool = TextTransformTool;
        let result = tool
            .execute(json!({"text": "HelloWorld", "transform": "snake_case"}))
            .await
            .unwrap();
        assert_eq!(result, "hello_world");
    }

    #[tokio::test]
    async fn test_regex_match() {
        let tool = RegexMatchTool;
        let result = tool
            .execute(json!({"text": "hello 123 world 456", "pattern": r"\d+"}))
            .await
            .unwrap();
        assert!(result.contains("123"));
        assert!(result.contains("456"));
    }

    #[tokio::test]
    async fn test_sort_lines() {
        let tool = SortLinesTool;
        let result = tool
            .execute(json!({"text": "zebra\napple\nbanana", "unique": false}))
            .await
            .unwrap();
        assert!(result.starts_with("apple"));
        assert!(result.ends_with("zebra"));
    }
}
