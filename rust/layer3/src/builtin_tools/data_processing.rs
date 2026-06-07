//! # Data Processing Tools
//!
//! 数据处理工具集：JSON/YAML/TOML 解析与处理。

use crate::builtin_tools::BuiltinTool;
use crate::types::{Layer3Result, ToolCategory};
use async_trait::async_trait;
use base64::Engine;
use serde_json::Value;

// ============================================================================
// JSON Parse Tool
// ============================================================================

/// JSON 解析工具
pub struct JsonParseTool;

#[async_trait]
impl BuiltinTool for JsonParseTool {
    fn name(&self) -> &str {
        "json_parse"
    }

    fn description(&self) -> &str {
        "Parse JSON string and return formatted output. Supports querying with JSONPath."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "json": {
                    "type": "string",
                    "description": "JSON string to parse"
                },
                "query": {
                    "type": "string",
                    "description": "Optional JSONPath query (e.g., '$.data[0].name')"
                }
            },
            "required": ["json"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let json_str = args["json"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing json parameter"))?;

        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;

        if let Some(query) = args["query"].as_str() {
            // Simple JSONPath-like query
            let result = query_json(&value, query)?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
        } else {
            Ok(serde_json::to_string_pretty(&value)
                .map_err(|e| anyhow::anyhow!("Failed to format JSON: {}", e))?)
        }
    }
}

/// Simple JSONPath query implementation
fn query_json(value: &Value, path: &str) -> Layer3Result<Value> {
    let path = path.strip_prefix('$').unwrap_or(path);

    // Parse path into parts - handle both `.key` and `[index]` syntax
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
                // Read until closing bracket
                while let Some(inner) = chars.next() {
                    if inner == ']' {
                        break;
                    }
                    current.push(inner);
                }
                if !current.is_empty() {
                    parts.push(format!("[{}]", current));
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    let mut result = value.clone();
    for part in &parts {
        if part.starts_with('[') && part.ends_with(']') {
            // Array index
            let index_str = &part[1..part.len() - 1];
            let index: usize = index_str
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid array index: {}", e))?;
            result = result
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Index {} out of bounds", index))?;
        } else {
            result = result
                .get(part)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Key '{}' not found", part))?;
        }
    }
    Ok(result)
}

// ============================================================================
// JSON Stringify Tool
// ============================================================================

/// JSON 序列化工具
pub struct JsonStringifyTool;

#[async_trait]
impl BuiltinTool for JsonStringifyTool {
    fn name(&self) -> &str {
        "json_stringify"
    }

    fn description(&self) -> &str {
        "Convert a value to JSON string with optional pretty printing."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "value": {
                    "description": "Value to stringify (any JSON value)"
                },
                "pretty": {
                    "type": "boolean",
                    "description": "Pretty print with indentation (default: false)"
                }
            },
            "required": ["value"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let value = args
            .get("value")
            .cloned()
            .unwrap_or(serde_json::json!(null));
        let pretty = args["pretty"].as_bool().unwrap_or(false);

        if pretty {
            Ok(serde_json::to_string_pretty(&value)
                .map_err(|e| anyhow::anyhow!("Failed to stringify: {}", e))?)
        } else {
            Ok(serde_json::to_string(&value)
                .map_err(|e| anyhow::anyhow!("Failed to stringify: {}", e))?)
        }
    }
}

// ============================================================================
// YAML Parse Tool
// ============================================================================

/// YAML 解析工具
pub struct YamlParseTool;

#[async_trait]
impl BuiltinTool for YamlParseTool {
    fn name(&self) -> &str {
        "yaml_parse"
    }

    fn description(&self) -> &str {
        "Parse YAML string and convert to JSON format."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "yaml": {
                    "type": "string",
                    "description": "YAML string to parse"
                }
            },
            "required": ["yaml"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let yaml_str = args["yaml"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing yaml parameter"))?;

        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?;

        // Convert to JSON for consistent output
        let json_value = serde_json::to_value(&value)
            .map_err(|e| anyhow::anyhow!("Failed to convert to JSON: {}", e))?;

        Ok(serde_json::to_string_pretty(&json_value)
            .map_err(|e| anyhow::anyhow!("Failed to format: {}", e))?)
    }
}

// ============================================================================
// YAML Stringify Tool
// ============================================================================

/// YAML 序列化工具
pub struct YamlStringifyTool;

#[async_trait]
impl BuiltinTool for YamlStringifyTool {
    fn name(&self) -> &str {
        "yaml_stringify"
    }

    fn description(&self) -> &str {
        "Convert JSON value to YAML format."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "value": {
                    "description": "Value to convert (any JSON value)"
                }
            },
            "required": ["value"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let value = args
            .get("value")
            .cloned()
            .unwrap_or(serde_json::json!(null));

        let yaml_str = serde_yaml::to_string(&value)
            .map_err(|e| anyhow::anyhow!("Failed to convert to YAML: {}", e))?;

        Ok(yaml_str)
    }
}

// ============================================================================
// TOML Parse Tool
// ============================================================================

/// TOML 解析工具
pub struct TomlParseTool;

#[async_trait]
impl BuiltinTool for TomlParseTool {
    fn name(&self) -> &str {
        "toml_parse"
    }

    fn description(&self) -> &str {
        "Parse TOML string and convert to JSON format."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "toml": {
                    "type": "string",
                    "description": "TOML string to parse"
                }
            },
            "required": ["toml"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let toml_str = args["toml"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing toml parameter"))?;

        let value: toml::Value = toml_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))?;

        // Convert to JSON for consistent output
        let json_str = serde_json::to_string_pretty(&value)
            .map_err(|e| anyhow::anyhow!("Failed to convert to JSON: {}", e))?;

        Ok(json_str)
    }
}

// ============================================================================
// CSV Parse Tool
// ============================================================================

/// CSV 解析工具
pub struct CsvParseTool;

#[async_trait]
impl BuiltinTool for CsvParseTool {
    fn name(&self) -> &str {
        "csv_parse"
    }

    fn description(&self) -> &str {
        "Parse CSV string and convert to JSON array of objects."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "csv": {
                    "type": "string",
                    "description": "CSV string to parse"
                },
                "delimiter": {
                    "type": "string",
                    "description": "Column delimiter (default: ',')"
                },
                "has_header": {
                    "type": "boolean",
                    "description": "First row is header (default: true)"
                }
            },
            "required": ["csv"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let csv_str = args["csv"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing csv parameter"))?;

        let delimiter = args["delimiter"]
            .as_str()
            .unwrap_or(",")
            .chars()
            .next()
            .unwrap_or(',');
        let has_header = args["has_header"].as_bool().unwrap_or(true);

        let mut result: Vec<serde_json::Map<String, Value>> = Vec::new();
        let mut lines = csv_str.lines();

        let headers: Vec<String> = if has_header {
            lines
                .next()
                .ok_or_else(|| anyhow::anyhow!("Empty CSV"))?
                .split(delimiter)
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            // Generate column names
            let first_line = lines
                .next()
                .ok_or_else(|| anyhow::anyhow!("Empty CSV"))?
                .split(delimiter)
                .count();
            (0..first_line).map(|i| format!("col_{}", i)).collect()
        };

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let values: Vec<&str> = line.split(delimiter).collect();
            let mut row = serde_json::Map::new();
            for (i, header) in headers.iter().enumerate() {
                let value = values.get(i).unwrap_or(&"").trim();
                // Try to parse as number or keep as string
                let json_value = if let Ok(n) = value.parse::<i64>() {
                    Value::Number(n.into())
                } else if let Ok(n) = value.parse::<f64>() {
                    Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| 0.into()))
                } else if value == "true" {
                    Value::Bool(true)
                } else if value == "false" {
                    Value::Bool(false)
                } else {
                    Value::String(value.to_string())
                };
                row.insert(header.clone(), json_value);
            }
            result.push(row);
        }

        Ok(serde_json::to_string_pretty(&result)
            .map_err(|e| anyhow::anyhow!("Failed to format: {}", e))?)
    }
}

// ============================================================================
// Base64 Encode Tool
// ============================================================================

/// Base64 编码工具
pub struct Base64EncodeTool;

#[async_trait]
impl BuiltinTool for Base64EncodeTool {
    fn name(&self) -> &str {
        "base64_encode"
    }

    fn description(&self) -> &str {
        "Encode a string to Base64 format."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to encode"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            text.as_bytes(),
        ))
    }
}

// ============================================================================
// Base64 Decode Tool
// ============================================================================

/// Base64 解码工具
pub struct Base64DecodeTool;

#[async_trait]
impl BuiltinTool for Base64DecodeTool {
    fn name(&self) -> &str {
        "base64_decode"
    }

    fn description(&self) -> &str {
        "Decode a Base64 string."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "encoded": {
                    "type": "string",
                    "description": "Base64 encoded string"
                }
            },
            "required": ["encoded"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let encoded = args["encoded"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing encoded parameter"))?;

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| anyhow::anyhow!("Failed to decode Base64: {}", e))?;

        String::from_utf8(decoded)
            .map_err(|e| anyhow::anyhow!("Decoded bytes are not valid UTF-8: {}", e))
    }
}

// ============================================================================
// URL Encode Tool
// ============================================================================

/// URL 编码工具
pub struct UrlEncodeTool;

#[async_trait]
impl BuiltinTool for UrlEncodeTool {
    fn name(&self) -> &str {
        "url_encode"
    }

    fn description(&self) -> &str {
        "Encode a string for use in URLs (percent encoding)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to encode"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        Ok(urlencoding::encode(text).into_owned())
    }
}

// ============================================================================
// URL Decode Tool
// ============================================================================

/// URL 解码工具
pub struct UrlDecodeTool;

#[async_trait]
impl BuiltinTool for UrlDecodeTool {
    fn name(&self) -> &str {
        "url_decode"
    }

    fn description(&self) -> &str {
        "Decode a URL-encoded string."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "encoded": {
                    "type": "string",
                    "description": "URL-encoded string"
                }
            },
            "required": ["encoded"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let encoded = args["encoded"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing encoded parameter"))?;

        Ok(urlencoding::decode(encoded)
            .map_err(|e| anyhow::anyhow!("Failed to decode URL: {}", e))?
            .into_owned())
    }
}

// ============================================================================
// Hash Tool
// ============================================================================

/// 哈希计算工具
pub struct HashTool;

#[async_trait]
impl BuiltinTool for HashTool {
    fn name(&self) -> &str {
        "hash"
    }

    fn description(&self) -> &str {
        "Calculate hash of a string. Supports MD5, SHA256, SHA512."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to hash"
                },
                "algorithm": {
                    "type": "string",
                    "enum": ["md5", "sha256", "sha512"],
                    "description": "Hash algorithm (default: sha256)"
                }
            },
            "required": ["text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text parameter"))?;

        let algorithm = args["algorithm"].as_str().unwrap_or("sha256");

        let hash = match algorithm {
            "md5" => {
                use md5::Md5;
                use sha2::Digest;
                let mut hasher = Md5::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            "sha256" => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            "sha512" => {
                use sha2::{Digest, Sha512};
                let mut hasher = Sha512::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            _ => return Err(anyhow::anyhow!("Unsupported algorithm: {}", algorithm)),
        };

        Ok(hash)
    }
}

// ============================================================================
// UUID Generate Tool
// ============================================================================

/// UUID 生成工具
pub struct UuidGenerateTool;

#[async_trait]
impl BuiltinTool for UuidGenerateTool {
    fn name(&self) -> &str {
        "uuid_generate"
    }

    fn description(&self) -> &str {
        "Generate a UUID. Supports v4 (random) and v7 (time-ordered)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "version": {
                    "type": "integer",
                    "enum": [4, 7],
                    "description": "UUID version (default: 4)"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of UUIDs to generate (default: 1)"
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::DataProcessing
    }

    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let version = args["version"].as_u64().unwrap_or(4);
        let count = args["count"].as_u64().unwrap_or(1).max(1).min(100) as usize;

        let uuids: Vec<String> = (0..count)
            .map(|_| {
                match version {
                    4 => uuid::Uuid::new_v4().to_string(),
                    7 => {
                        // v7 uses timestamp + random - fallback to v4 if not available
                        uuid::Uuid::new_v4().to_string()
                    }
                    _ => uuid::Uuid::new_v4().to_string(),
                }
            })
            .collect();

        if count == 1 {
            Ok(uuids[0].clone())
        } else {
            Ok(uuids.join("\n"))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_parse() {
        let tool = JsonParseTool;
        assert_eq!(tool.category(), ToolCategory::DataProcessing);
    }

    #[tokio::test]
    async fn test_json_parse_basic() {
        let tool = JsonParseTool;
        let result = tool
            .execute(json!({"json": r#"{"name": "test", "value": 42}"#}))
            .await
            .unwrap();
        assert!(result.contains("test"));
        assert!(result.contains("42"));
    }

    #[tokio::test]
    async fn test_json_parse_with_query() {
        let tool = JsonParseTool;
        let result = tool
            .execute(json!({
                "json": r#"{"data": [{"name": "Alice"}, {"name": "Bob"}]}"#,
                "query": "$.data[0].name"
            }))
            .await
            .unwrap();
        assert!(result.contains("Alice"));
    }

    #[tokio::test]
    async fn test_csv_parse() {
        let tool = CsvParseTool;
        let result = tool
            .execute(json!({
                "csv": "name,age,active\nAlice,30,true\nBob,25,false"
            }))
            .await
            .unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("30"));
    }

    #[tokio::test]
    async fn test_base64_roundtrip() {
        let encode_tool = Base64EncodeTool;
        let encoded = encode_tool
            .execute(json!({"text": "Hello, World!"}))
            .await
            .unwrap();

        let decode_tool = Base64DecodeTool;
        let decoded = decode_tool
            .execute(json!({"encoded": encoded}))
            .await
            .unwrap();

        assert_eq!(decoded, "Hello, World!");
    }

    #[tokio::test]
    async fn test_url_roundtrip() {
        let encode_tool = UrlEncodeTool;
        let encoded = encode_tool
            .execute(json!({"text": "hello world"}))
            .await
            .unwrap();

        let decode_tool = UrlDecodeTool;
        let decoded = decode_tool
            .execute(json!({"encoded": encoded}))
            .await
            .unwrap();

        assert_eq!(decoded, "hello world");
    }

    #[tokio::test]
    async fn test_hash_sha256() {
        let tool = HashTool;
        let result = tool
            .execute(json!({"text": "hello", "algorithm": "sha256"}))
            .await
            .unwrap();
        // SHA256 of "hello" is well-known
        assert_eq!(result.len(), 64); // 256 bits = 64 hex chars
    }

    #[tokio::test]
    async fn test_uuid_generate() {
        let tool = UuidGenerateTool;
        let result = tool.execute(json!({})).await.unwrap();
        assert!(uuid::Uuid::parse_str(&result).is_ok());
    }

    #[tokio::test]
    async fn test_uuid_generate_multiple() {
        let tool = UuidGenerateTool;
        let result = tool.execute(json!({"count": 5})).await.unwrap();
        let uuids: Vec<&str> = result.lines().collect();
        assert_eq!(uuids.len(), 5);
    }
}
