use std::{collections::HashMap, path::Path, sync::atomic::AtomicU32};

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
    extension: String,
    size_bytes: u64,
}

lazy_static! {
    static ref LANGUAGE_MAP: HashMap<&'static str, Language> = {
        let mut m = HashMap::new();
        m.insert("rs", tree_sitter_rust::LANGUAGE.into());
        m.insert("toml", tree_sitter_rust::LANGUAGE.into());
        m
    };
}

fn chunk_all_documents(docs: &[Document]) -> (Vec<Chunk>, HashMap<u32, usize>) {
    let next_id = AtomicU32::new(0);

    let doc_chunks_vec: Vec<Vec<Chunk>> = docs
        .par_iter()
        .map(|doc| chunk_document(doc.id, &doc.text, &doc.meta.extension, &next_id))
        .collect();

    let mut all_chunks =
        Vec::with_capacity(doc_chunks_vec.iter().map(|dc| dc.len()).sum::<usize>());

    for doc_chunks in doc_chunks_vec {
        all_chunks.extend(doc_chunks);
    }

    let id_to_idx: HashMap<u32, usize> = all_chunks
        .iter()
        .enumerate()
        .map(|(idx, doc)| (doc.id, idx))
        .collect();

    (all_chunks, id_to_idx)
}

fn chunk_document(doc_id: u32, doc_text: &str, doc_ext: &str, next_id: &AtomicU32) -> Vec<Chunk> {
    let mut chunks = vec![];

    let language = LANGUAGE_MAP.get(doc_ext);

    if let Some(lang) = language {
        let mut parser = Parser::new();
        parser.set_language(lang).expect("Bad language!");
        let tree = parser.parse(doc_text, None).expect("Parse failed");
        let root = tree.root_node();

        let query = get_query_from_extension(doc_ext);

        todo!()
    } else {
        chunks = naive_chunk_document(doc_text, doc_id, next_id);
    };

    chunks
}

fn naive_chunk_document(doc_text: &str, doc_id: u32, next_id: &AtomicU32) -> Vec<Chunk> {
    todo!()
}

fn get_query_from_extension(extension: &str) -> Query {
    todo!()
}

use jwalk::WalkDir;
use lazy_static::lazy_static;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tree_sitter::{Language, Parser, Query};

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
            .map(|e| e.to_string())
            .unwrap(),
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
