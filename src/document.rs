use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use sha2::Digest;
use std::{path::Path, time::Duration};

pub type DocumentID = [u8; 32];
fn normalized_path_for_id(relative: &Path) -> String {
    relative.to_string_lossy().replace('\\', "/")
}
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

pub fn grab_all_documents(root: &Path) -> Vec<Document> {
    WalkDir::new(root)
        .parallelism(jwalk::Parallelism::RayonDefaultPool {
            busy_timeout: Duration::new(100, 0),
        })
        .into_iter()
        .par_bridge()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            load_document(root, &entry)
        })
        .collect()
}

/*pub fn grab_all_documents(root: &Path) -> Vec<Document> {
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
}*/

fn load_document(root: &Path, entry: &jwalk::DirEntry<((), ())>) -> Option<Document> {
    if !entry.file_type.is_file() {
        return None;
    }

    let path = entry.path();
    let relative = path.strip_prefix(root).ok()?;
    let relative_str = normalized_path_for_id(relative);

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => return None, // not UTF-8
        Err(e) => {
            log::warn!("Failed to read {}: {}", path.display(), e);
            return None;
        }
    };

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    let id = compute_document_id(&relative_str, &text);

    let meta = match entry.metadata() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to get metadata {}: {}", path.display(), e);
            return None;
        }
    };

    let size = meta.len();

    Some(Document {
        id,
        path: relative_str,
        text,
        ext,
        size,
    })
}
/*fn load_document(root: &Path, relative: &Path) -> Option<Document> {
    let path = root.join(relative);

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => return None, // not UTF-8
        Err(e) => {
            log::warn!("Failed to read {}: {}", path.display(), e);
            return None;
        }
    };

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let size = path.metadata().ok()?.len();

    let id: DocumentID = compute_document_id(&relative.display().to_string(), &text);

    Some(Document {
        id,
        path: relative.display().to_string(),
        text,
        ext,
        size,
    })
}*/
