//! Kanso analyzes agent instruction files without network access or execution.
//!
//! ```no_run
//! use kanso::{Config, engine};
//! let report = engine::check(&[], &Config::default())?;
//! println!("{}", report.score);
//! # Ok::<(), kanso::KansoError>(())
//! ```
#![forbid(unsafe_code)]
pub mod config;
pub mod decider;
pub mod discovery;
pub mod engine;
pub mod model;
pub mod parser;
pub mod rules;
pub mod score;
pub use config::Config;
pub use model::{Diagnostic, Document, FileReport, Report, Severity};
/// Errors that prevent an analysis or configuration operation from completing.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum KansoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
}
