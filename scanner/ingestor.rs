// ======================================================================
// FILE INGESTOR - PRODUCTION READY
// File: src/scanner/ingestor.rs
// Description: Reads ANY file type and extracts text
//              Supports: PDF, DOCX, images (OCR), archives (ZIP/TAR/GZ),
//              code files, markdown, HTML, JSON, TOML, YAML
// ======================================================================

use std::path::Path;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use regex::Regex;

pub struct Ingestor {
    max_file_size_bytes: u64,
    supported_extensions: Vec<&'static str>,
}

impl Ingestor {
    pub fn new() -> Self {
        Self {
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            supported_extensions: vec![
                "txt", "md", "rs", "py", "js", "ts", "go", "c", "cpp", "h", "hpp",
                "json", "toml", "yaml", "yml", "xml", "html", "css", "scss",
                "pdf", "docx", "jpg", "jpeg", "png", "gif", "bmp",
                "zip", "tar", "gz", "tgz",
            ],
        }
    }
    
    pub async fn ingest_file(&self, path: &Path) -> Result<String> {
        // Check file size
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > self.max_file_size_bytes {
            anyhow::bail!("File too large: {} bytes (max: {})", metadata.len(), self.max_file_size_bytes);
        }
        
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        debug!("Ingesting {:?} (type: {})", path, extension);
        
        let content = match extension.as_str() {
            // Text-based files
            "txt" | "md" | "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "hpp" |
            "json" | "toml" | "yaml" | "yml" | "xml" | "html" | "css" | "scss" => {
                self.read_text_file(path)?
            }
            // PDF
            "pdf" => self.read_pdf_file(path).await?,
            // DOCX
            "docx" => self.read_docx_file(path).await?,
            // Images (OCR)
            "jpg" | "jpeg" | "png" | "gif" | "bmp" => self.read_image_file(path).await?,
            // Archives
            "zip" => self.read_zip_file(path).await?,
            "tar" => self.read_tar_file(path).await?,
            "gz" | "tgz" => self.read_gz_file(path).await?,
            // Unknown
            _ => anyhow::bail!("Unsupported file type: .{}", extension),
        };
        
        Ok(content)
    }
    
    fn read_text_file(&self, path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read text file: {:?}", path))?;
        Ok(content)
    }
    
    async fn read_pdf_file(&self, path: &Path) -> Result<String> {
        // Use pdf-extract for robust PDF parsing
        let content = pdf_extract::extract_text(path)
            .with_context(|| format!("Failed to extract PDF text: {:?}", path))?;
        Ok(content)
    }
    
    async fn read_docx_file(&self, path: &Path) -> Result<String> {
        use docx::DocxFile;
        
        let docx = DocxFile::from_file(path)
            .with_context(|| format!("Failed to open DOCX: {:?}", path))?;
        let content = docx.extract_text()
            .with_context(|| format!("Failed to extract DOCX text: {:?}", path))?;
        Ok(content)
    }
    
    async fn read_image_file(&self, path: &Path) -> Result<String> {
        // Use tesseract for OCR
        let image = image::open(path)
            .with_context(|| format!("Failed to open image: {:?}", path))?;
        
        // Save temporary file for tesseract
        let temp_path = std::env::temp_dir().join(format!("ocr_{}.png", uuid::Uuid::new_v4()));
        image.save(&temp_path)?;
        
        let text = tesseract::recognize_text(&temp_path.to_string_lossy())
            .with_context(|| format!("OCR failed: {:?}", path))?;
        
        // Clean up temp file
        let _ = std::fs::remove_file(temp_path);
        
        Ok(text)
    }
    
    async fn read_zip_file(&self, path: &Path) -> Result<String> {
        use zip::read::ZipArchive;
        
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;
        let mut contents = Vec::new();
        
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_file() {
                let mut text = String::new();
                entry.read_to_string(&mut text)?;
                contents.push(format!("--- {} ---\n{}", entry.name(), text));
            }
        }
        
        Ok(contents.join("\n\n"))
    }
    
    async fn read_tar_file(&self, path: &Path) -> Result<String> {
        use tar::Archive;
        
        let file = File::open(path)?;
        let mut archive = Archive::new(file);
        let mut contents = Vec::new();
        
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            contents.push(format!("--- {} ---\n{}", path.display(), text));
        }
        
        Ok(contents.join("\n\n"))
    }
    
    async fn read_gz_file(&self, path: &Path) -> Result<String> {
        use flate2::read::GzDecoder;
        
        let file = File::open(path)?;
        let decoder = GzDecoder::new(file);
        let mut reader = BufReader::new(decoder);
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Ok(content)
    }
}

impl Default for Ingestor {
    fn default() -> Self {
        Self::new()
    }
}
