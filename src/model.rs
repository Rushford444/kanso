use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Severity assigned to a diagnostic.
pub enum Severity {
    Error,
    Warn,
    Info,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// One finding associated with a source location.
pub struct Diagnostic {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Instruction file path and original UTF-8 source.
pub struct Document {
    pub path: PathBuf,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Per-file score and token count.
pub struct FileReport {
    pub path: PathBuf,
    pub tokens: usize,
    pub score: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Complete analysis result, including per-file and global scores.
pub struct Report {
    pub schema: String,
    pub files: Vec<FileReport>,
    pub diagnostics: Vec<Diagnostic>,
    pub score: u8,
    pub total_tokens: usize,
}
