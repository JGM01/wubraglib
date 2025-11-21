use crate::error::{RAGError, Result};
use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sha2::Digest;
use std::{io, path::Path};
pub type DocumentID = [u8; 32];

fn compute_document_id(path: &str, content: &str) -> DocumentID {
    let mut hash = sha2::Sha256::new();
    hash.update(path.as_bytes());
    hash.update(content.as_bytes());
    hash.finalize().into()
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentID,
    pub path: String,
    pub text: String,
    pub ext: String,
    pub size: u64,
}

pub fn grab_all_documents(root: &Path) -> Result<Vec<Document>> {
    if !root.exists() {
        return Err(RAGError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Root path does not exist: {}", root.display()),
        )));
    }

    let paths: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) if entry.file_type().is_file() => entry
                .path()
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_path_buf()),
            Ok(_) => None,
            Err(err) => {
                // Log but don't fail on individual file errors
                eprintln!("Warning: Failed to walk directory entry: {}", err);
                None
            }
        })
        .collect();

    let results: Result<Vec<Document>> = paths
        .par_iter()
        .map(|relative| load_document(root, relative))
        .collect();

    results
}

fn load_document(root: &Path, relative: &Path) -> Result<Document> {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).map_err(|e| RAGError::FileRead {
        path: path.clone(),
        source: e,
    })?;

    let metadata = std::fs::metadata(&path).map_err(|e| RAGError::FileRead {
        path: path.clone(),
        source: e,
    })?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    let size = metadata.len();

    let id: DocumentID = compute_document_id(&relative.display().to_string(), &text);

    Ok(Document {
        id,
        path: relative.display().to_string(),
        text,
        ext,
        size,
    })
}
