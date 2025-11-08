use std::{path::Path, sync::atomic::AtomicU32};

pub struct Chunk {
    pub id: u32,                // primary key, u32 because linux kernel is like 40million LOC
    pub doc_id: u32,            // foreign key id of the document that the chunk is attached to
    pub text: String,           // content of the chunk
    pub chunk_type: String,     // whatever is returned by node.kind() with tree-sitter
    pub parent_id: Option<u32>, // could have a parent, could not
    pub children_ids: Vec<u32>, // children id vec (no Option because it can just be empty)
    pub token_count: usize,     // amount of tokens for logic stuff
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: u32,
    pub path: String,
    pub text: String,
    pub meta: DocumentMetadata,
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    extension: Option<String>,
    size_bytes: u64,
}

use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

static ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn f2doc(root: &Path, relative_path: &Path) -> Option<Document> {
    let path = root.join(relative_path);

    let text = match std::fs::read_to_string(&path) {
        Ok(txt) => txt,
        Err(_) => return None,
    };
    let meta = DocumentMetadata {
        extension: path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_string()),
        size_bytes: path.metadata().expect("oopsie").len(),
    };

    Some(Document {
        id: ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        path: relative_path.display().to_string(),
        text,
        meta,
    })
}
pub fn grab_all_documents(root: &Path) -> Vec<Document> {
    let paths: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let relative_path = e
                .path()
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some(relative_path)
        })
        .collect();

    let docs: Vec<Document> = paths
        .par_iter()
        .filter_map(|relative_path| f2doc(root, Path::new(relative_path)))
        .collect();
    docs
}
