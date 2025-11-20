use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
};

pub type DocumentID = [u8; 32];

fn compute_document_id(path: &str, content: &str) -> DocumentID {
    todo!()
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentID,
    pub path: String,
    pub text: String,
    pub meta: DocumentMetadata,
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub extension: String,
    pub size_bytes: u64,
}

pub fn grab_all_documents(root: &Path) -> Vec<Document> {
    let paths: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let binding = e.path();
            let rel = binding.strip_prefix(root).ok()?;
            Some(rel.to_string_lossy().into_owned())
        })
        .collect();

    paths
        .par_iter()
        .filter_map(|relative| load_document(root, Path::new(relative)))
        .collect()
}

fn load_document(root: &Path, relative: &Path) -> Option<Document> {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).ok()?;

    let meta = DocumentMetadata {
        extension: path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string(),
        size_bytes: path.metadata().ok()?.len(),
    };

    let id: DocumentID = compute_document_id(&relative.display().to_string(), &text);

    Some(Document {
        id,
        path: relative.display().to_string(),
        text,
        meta,
    })
}
