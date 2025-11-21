use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum RAGError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to read file at {path}: {source}")]
    FileRead { path: PathBuf, source: io::Error },

    #[error("Invalid UTF-8 in file {path}")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("Tree-sitter parsing failed for {extension}")]
    ParsingFailed { extension: String },

    #[error("Embedding model initialization failed: {0}")]
    ModelInit(String),

    #[error("Embedding generation failed: {0}")]
    Embedding(String),

    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Deserialization failed: {0}")]
    Deserialization(String),

    #[error("Empty embeddings vector")]
    EmptyEmbeddings,

    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("Invalid index: {0}")]
    InvalidIndex(usize),

    #[error("No chunks produced for document {doc_id:?}")]
    NoChunks { doc_id: [u8; 32] },
}

pub type Result<T> = std::result::Result<T, RAGError>;
