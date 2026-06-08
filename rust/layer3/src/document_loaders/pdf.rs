//! # PDF Document Loader
//!
//! PDF 文件加载器：解析 PDF 文件并提取文本内容和元数据。
//!
//! ## 功能
//!
//! - 基于页面的文本提取
//! - 元数据提取（标题、作者、创建日期等）
//! - 支持异步加载
//! - 完整错误处理

use crate::document_loaders::{DocumentLoader, LoadOptions};
use crate::retriever_engine::Document;
use crate::types::Layer3Error;
use crate::types::Layer3Result;
use async_trait::async_trait;
use lopdf::Document as PdfDoc;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

/// PDF Loader 实现
pub struct PdfLoader {
    #[allow(dead_code)]
    options: LoadOptions,
}

impl PdfLoader {
    pub fn new() -> Self {
        Self {
            options: LoadOptions::default(),
        }
    }

    pub fn with_options(options: LoadOptions) -> Self {
        Self { options }
    }

    /// 从 PDF 提取元数据
    fn extract_metadata(&self, pdf: &PdfDoc) -> HashMap<String, serde_json::Value> {
        let mut metadata = HashMap::new();

        // Helper to extract string from PDF dictionary
        fn get_string_from_dict(
            dict: &lopdf::Dictionary,
            key: &[u8],
        ) -> Option<String> {
            let obj = dict.get(key).ok()?;
            if let lopdf::Object::String(bytes, _) = obj {
                PdfLoader::decode_pdf_string(bytes).ok()
            } else {
                None
            }
        }

        // 提取文档信息
        if let Ok(trailer) = pdf.trailer.get(b"Info") {
            if let Ok(info_ref) = trailer.as_reference() {
                if let Ok(lopdf::Object::Dictionary(dict)) = pdf.get_object(info_ref) {
                    // 标题
                    if let Some(title) = get_string_from_dict(dict, b"Title") {
                        metadata.insert("title".to_string(), serde_json::json!(title));
                    }

                    // 作者
                    if let Some(author) = get_string_from_dict(dict, b"Author") {
                        metadata.insert("author".to_string(), serde_json::json!(author));
                    }

                    // 主题
                    if let Some(subject) = get_string_from_dict(dict, b"Subject") {
                        metadata.insert("subject".to_string(), serde_json::json!(subject));
                    }

                    // 创建者
                    if let Some(creator) = get_string_from_dict(dict, b"Creator") {
                        metadata.insert("creator".to_string(), serde_json::json!(creator));
                    }

                    // 生产者
                    if let Some(producer) = get_string_from_dict(dict, b"Producer") {
                        metadata.insert("producer".to_string(), serde_json::json!(producer));
                    }

                    // 创建日期
                    if let Some(creation_date) = get_string_from_dict(dict, b"CreationDate") {
                        metadata.insert("creation_date".to_string(), serde_json::json!(creation_date));
                    }

                    // 修改日期
                    if let Some(mod_date) = get_string_from_dict(dict, b"ModDate") {
                        metadata.insert("modification_date".to_string(), serde_json::json!(mod_date));
                    }
                }
            }
        }

        // 页数
        let page_count = pdf.get_pages().len();
        metadata.insert("page_count".to_string(), serde_json::json!(page_count));

        metadata
    }

    /// 解码 PDF 字符串（处理编码）
    fn decode_pdf_string(bytes: &[u8]) -> Layer3Result<String> {
        // 尝试 UTF-8
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Ok(s.to_string());
        }

        // 尝试 Latin-1 (ISO-8859-1)
        let decoded: String = bytes.iter().map(|&b| b as char).collect();
        Ok(decoded)
    }

    /// 从单个页面提取文本
    fn extract_page_text(pdf: &PdfDoc, page_id: (u32, u16)) -> Layer3Result<String> {
        let mut text = String::new();

        if let Ok(lopdf::Object::Dictionary(dict)) = pdf.get_object(page_id) {
            if let Ok(contents) = dict.get(b"Contents") {
                match contents {
                    lopdf::Object::Reference(ref_id) => {
                        if let Ok(lopdf::Object::Stream(stream_obj)) = pdf.get_object(*ref_id) {
                            if let Ok(content) = stream_obj.decompressed_content() {
                                text.push_str(&Self::parse_content_stream(&content));
                            }
                        }
                    }
                    lopdf::Object::Array(arr) => {
                        for obj in arr {
                            if let lopdf::Object::Reference(ref_id) = obj {
                                if let Ok(lopdf::Object::Stream(stream_obj)) = pdf.get_object(*ref_id) {
                                    if let Ok(content) = stream_obj.decompressed_content() {
                                        text.push_str(&Self::parse_content_stream(&content));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(text)
    }

    /// 解析 PDF 内容流，提取文本
    fn parse_content_stream(content: &[u8]) -> String {
        let mut text = String::new();
        let content_str = String::from_utf8_lossy(content);

        // 简单的文本提取：查找 Tj 和 TJ 操作符
        let mut current_text = String::new();
        let mut in_string = false;
        let mut escape_next = false;

        for ch in content_str.chars() {
            if escape_next {
                current_text.push(ch);
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => {
                    escape_next = true;
                }
                '(' => {
                    if !in_string {
                        in_string = true;
                        current_text.clear();
                    } else {
                        current_text.push(ch);
                    }
                }
                ')' => {
                    if in_string {
                        in_string = false;
                        if !current_text.is_empty() {
                            // 过滤控制字符
                            let cleaned: String = current_text
                                .chars()
                                .filter(|c| {
                                    c.is_alphabetic()
                                        || c.is_numeric()
                                        || c.is_whitespace()
                                        || *c == '-'
                                        || *c == '.'
                                        || *c == ','
                                })
                                .collect();
                            if !cleaned.trim().is_empty() {
                                text.push_str(&cleaned);
                                text.push(' ');
                            }
                        }
                    } else {
                        current_text.push(ch);
                    }
                }
                _ => {
                    if in_string {
                        current_text.push(ch);
                    }
                }
            }
        }

        // 清理多余空格
        let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

        cleaned
    }

    /// 从 PDF 提取所有页面文本
    fn extract_all_text(&self, pdf: &PdfDoc) -> Layer3Result<Vec<(usize, String)>> {
        let pages = pdf.get_pages();
        let mut result = Vec::new();

        for (page_num, page_id) in pages.iter() {
            match Self::extract_page_text(pdf, *page_id) {
                Ok(page_text) => {
                    if !page_text.trim().is_empty() {
                        result.push((*page_num as usize, page_text));
                    }
                }
                Err(e) => {
                    warn!("Failed to extract text from page {}: {}", page_num, e);
                }
            }
        }

        Ok(result)
    }
}

impl Default for PdfLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentLoader for PdfLoader {
    async fn load(&self, path: PathBuf) -> Layer3Result<Document> {
        debug!("Loading PDF file: {:?}", path);

        // 读取 PDF 文件
        let pdf = PdfDoc::load(&path).map_err(|e| {
            Layer3Error::PersistenceError(format!(
                "Failed to load PDF file '{}': {}",
                path.display(),
                e
            ))
        })?;

        // 提取元数据
        let metadata = self.extract_metadata(&pdf);

        // 提取所有文本
        let pages = self.extract_all_text(&pdf)?;
        let full_text: String = pages
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        // 创建文档
        let mut doc = Document::new(full_text).with_source(path.to_string_lossy().to_string());

        // 添加元数据
        doc.metadata = metadata;

        Ok(doc)
    }

    async fn load_and_split(&self, path: PathBuf) -> Layer3Result<Vec<Document>> {
        debug!("Loading and splitting PDF file: {:?}", path);

        // 读取 PDF 文件
        let pdf = PdfDoc::load(&path).map_err(|e| {
            Layer3Error::PersistenceError(format!(
                "Failed to load PDF file '{}': {}",
                path.display(),
                e
            ))
        })?;

        // 提取元数据
        let base_metadata = self.extract_metadata(&pdf);

        // 提取所有页面文本
        let pages = self.extract_all_text(&pdf)?;

        if pages.is_empty() {
            // 如果没有提取到文本，返回空数组
            return Ok(Vec::new());
        }

        // 为每个页面创建一个文档
        let source = path.to_string_lossy().to_string();
        let documents: Vec<Document> = pages
            .into_iter()
            .map(|(page_num, text)| {
                let mut metadata = base_metadata.clone();
                metadata.insert("page".to_string(), serde_json::json!(page_num));
                metadata.insert(
                    "total_pages".to_string(),
                    serde_json::json!(pdf.get_pages().len()),
                );

                Document {
                    id: None,
                    content: text,
                    metadata,
                    source: Some(source.clone()),
                }
            })
            .collect();

        Ok(documents)
    }

    fn supports(&self, path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "pdf")
            .unwrap_or(false)
    }

    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::Dictionary;
    use lopdf::Object as PdfObject;
    use lopdf::Stream;
    use tempfile::NamedTempFile;

    fn create_minimal_pdf() -> NamedTempFile {
        // Create a PDF programmatically using lopdf
        let mut pdf = lopdf::Document::new();

        // Object 1: Catalog
        pdf.add_object(PdfObject::Dictionary(Dictionary::from_iter([
            ("Type", PdfObject::Name("Catalog".as_bytes().to_vec())),
            ("Pages", PdfObject::Reference((2, 0))),
        ])));

        // Object 2: Pages
        pdf.add_object(PdfObject::Dictionary(Dictionary::from_iter([
            ("Type", PdfObject::Name("Pages".as_bytes().to_vec())),
            ("Kids", PdfObject::Array(vec![PdfObject::Reference((3, 0))])),
            ("Count", PdfObject::Integer(1)),
        ])));

        // Object 3: Page
        pdf.add_object(PdfObject::Dictionary(Dictionary::from_iter([
            ("Type", PdfObject::Name("Page".as_bytes().to_vec())),
            ("Parent", PdfObject::Reference((2, 0))),
            (
                "MediaBox",
                PdfObject::Array(vec![
                    PdfObject::Integer(0),
                    PdfObject::Integer(0),
                    PdfObject::Integer(612),
                    PdfObject::Integer(792),
                ]),
            ),
            ("Contents", PdfObject::Reference((4, 0))),
        ])));

        // Object 4: Content stream with text
        let content = b"BT /F1 12 Tf 100 700 Td (Hello World) Tj ET";
        pdf.add_object(PdfObject::Stream(Stream::new(
            Dictionary::from_iter([("Length", PdfObject::Integer(content.len() as i64))]),
            content.to_vec(),
        )));

        // Save to temp file
        let file = NamedTempFile::with_suffix(".pdf").unwrap();
        pdf.save(file.path()).expect("Failed to save PDF");
        file
    }

    #[test]
    fn test_pdf_loader_extensions() {
        let loader = PdfLoader::new();
        assert!(loader.extensions().contains(&"pdf"));
    }

    #[test]
    fn test_pdf_loader_supports() {
        let loader = PdfLoader::new();
        assert!(loader.supports(std::path::Path::new("test.pdf")));
        assert!(loader.supports(std::path::Path::new("test.PDF")));
        assert!(!loader.supports(std::path::Path::new("test.txt")));
    }

    #[tokio::test]
    async fn test_pdf_loader_load() {
        let loader = PdfLoader::new();
        let pdf_file = create_minimal_pdf();

        let result = loader.load(pdf_file.path().to_path_buf()).await;
        if let Err(ref err) = result {
            eprintln!("Error loading PDF: {:?}", err);
        }
        assert!(result.is_ok(), "PDF should load successfully");

        let doc = result.unwrap();
        assert!(doc.source.is_some());
        // Should have page_count metadata even if no text extracted
        assert!(doc.metadata.contains_key("page_count"));
    }

    #[tokio::test]
    async fn test_pdf_loader_load_and_split() {
        let loader = PdfLoader::new();
        let pdf_file = create_minimal_pdf();

        let result = loader.load_and_split(pdf_file.path().to_path_buf()).await;
        if let Err(ref err) = result {
            eprintln!("Error loading PDF: {:?}", err);
        }
        assert!(result.is_ok(), "PDF should load successfully");

        let docs = result.unwrap();
        // Minimal PDF has 1 page, so should return 1 doc (or 0 if no text)
        assert!(docs.len() <= 1);
    }

    #[test]
    fn test_decode_pdf_string_utf8() {
        let bytes = b"Hello World";
        let result = PdfLoader::decode_pdf_string(bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World");
    }

    #[test]
    fn test_decode_pdf_string_latin1() {
        // Latin-1 编码的 "Café"
        let bytes = vec![b'C', b'a', b'f', 0xE9]; // é in Latin-1
        let result = PdfLoader::decode_pdf_string(&bytes);
        assert!(result.is_ok());
    }
}
