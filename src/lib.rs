use jwalk::WalkDir;
use lazy_static::lazy_static;
use rayon::{
    iter::{IntoParallelRefIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use std::{
    collections::HashMap,
    iter,
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
};
use tiktoken_rs::o200k_base;
use tree_sitter::{
    Language, Node, Parser, Query, QueryCursor, QueryMatch, QueryMatches, StreamingIterator,
};

#[derive(Debug, Clone)]
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

pub fn chunk_all_documents(docs: &[Document]) -> (Vec<Chunk>, HashMap<u32, usize>) {
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

        let query_str = get_query_from_extension(doc_ext);

        let query = Query::new(lang, query_str).expect("invalid query");

        let mut cursor = QueryCursor::new();
        let b_text = doc_text.as_bytes();

        let text_callback = |node: Node| {
            let start = node.start_byte() as usize;
            let end = node.end_byte() as usize;
            let slice = &b_text[start..end];
            iter::once(slice)
        };

        let mut qmatches = cursor.matches(&query, root, text_callback);

        while let Some(m) = qmatches.next() {
            for capture in m.captures {
                let node = capture.node;
                let mut children = traverse_and_chunk(&node, doc_text, next_id, doc_id, None);

                let chunk_text = &doc_text[node.start_byte()..node.end_byte()];
                let chunk_token_count = o200k_base().unwrap().encode_ordinary(doc_text).len();
                let id = next_id.fetch_add(1, Ordering::SeqCst);

                let chunk = Chunk {
                    id,
                    doc_id,
                    text: chunk_text.to_string(),
                    chunk_type: node.kind().to_string(),
                    parent_id: None,
                    children_ids: children.iter().map(|c| c.id).collect(),
                    token_count: chunk_token_count,
                };

                for child in &mut children {
                    if child.parent_id.is_none() {
                        child.parent_id = Some(id);
                    }
                }
                chunks.extend(children);
                chunks.push(chunk);
            }
        }
    } else {
        chunks = naive_chunk_document(doc_text, doc_id, next_id);
    };

    chunks.par_sort_unstable_by_key(|c| c.id);
    chunks
}

fn traverse_and_chunk(
    node: &Node,
    doc_text: &str,
    next_id: &AtomicU32,
    doc_id: u32,
    parent_id: Option<u32>,
) -> Vec<Chunk> {
    todo!()
}

fn naive_chunk_document(doc_text: &str, doc_id: u32, next_id: &AtomicU32) -> Vec<Chunk> {
    let mut chunks = vec![];
    for para in doc_text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let id = next_id.fetch_add(1, Ordering::SeqCst);
        let tcount = o200k_base().unwrap().encode_ordinary(para).len();
        chunks.push(Chunk {
            id,
            doc_id,
            text: para.to_string(),
            chunk_type: "paragraph".to_string(),
            parent_id: None,
            children_ids: vec![],
            token_count: tcount,
        });
    }
    chunks
}

fn get_query_from_extension(extension: &str) -> &str {
    todo!()
}

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
