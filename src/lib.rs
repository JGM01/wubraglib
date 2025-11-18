use fastembed::{InitOptions, TextEmbedding};
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
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: u32,            // primary key, u32 because linux kernel is like 40million LOC
    pub doc_id: u32,        // foreign key id of the document that the chunk is attached to
    pub text: String,       // content of the chunk
    pub chunk_type: String, // whatever is returned by node.kind() with tree-sitter
    pub char_count: usize,  // amount of tokens for logic stuff
}

pub struct Index {
    pub chunks: Vec<Chunk>,
    pub id_to_idx: HashMap<u32, usize>,
    pub embeddings: Vec<Vec<f32>>,
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

pub fn create_index() -> Index {
    todo!()
}

pub fn embed_chunks(chunks: &mut Vec<Chunk>) -> Vec<Vec<f32>> {
    let model = TextEmbedding::try_new(
        InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_show_download_progress(true),
    );

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let embeddings = model
        .expect("oopsie model")
        .embed(texts, None)
        .expect("oopsie embed");

    embeddings
}

lazy_static! {
    static ref LANGUAGE_MAP: HashMap<&'static str, Language> = {
        let mut m = HashMap::new();
        m.insert("rs", tree_sitter_rust::LANGUAGE.into());
        m.insert("cpp", tree_sitter_cpp::LANGUAGE.into());
        m.insert("hpp", tree_sitter_cpp::LANGUAGE.into());
        m.insert("c", tree_sitter_c::LANGUAGE.into());
        m.insert("h", tree_sitter_c::LANGUAGE.into());
        m.insert("js", tree_sitter_javascript::LANGUAGE.into());
        m.insert("py", tree_sitter_python::LANGUAGE.into());
        m.insert("cu", tree_sitter_cuda::LANGUAGE.into());
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

        let query_str = get_query_from_extension(doc_ext).unwrap_or("".to_string());
        let query = Query::new(lang, &query_str).expect("invalid query");

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
                let start = node.start_byte() as usize;
                let end = node.end_byte() as usize;
                let chunk_text = &doc_text[start..end].trim();
                let token_count = chunk_text.len();

                let id = next_id.fetch_add(1, Ordering::SeqCst);
                chunks.push(Chunk {
                    id,
                    doc_id,
                    text: chunk_text.to_string(),
                    chunk_type: node.kind().to_string(),
                    char_count: token_count,
                });
            }
        }
        if chunks.is_empty() {
            let id = next_id.fetch_add(1, Ordering::SeqCst);
            chunks.push(Chunk {
                id,
                doc_id,
                text: doc_text.trim().to_string(),
                chunk_type: "document".to_string(),
                char_count: doc_text.len(),
            });
        }
    } else {
        chunks = naive_chunk_document(doc_text, doc_id, next_id);
    };

    chunks.par_sort_unstable_by_key(|c| c.id);
    chunks
}

fn naive_chunk_document(doc_text: &str, doc_id: u32, next_id: &AtomicU32) -> Vec<Chunk> {
    let mut chunks = vec![];
    for para in doc_text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let id = next_id.fetch_add(1, Ordering::SeqCst);
        let tcount = para.len();
        chunks.push(Chunk {
            id,
            doc_id,
            text: para.to_string(),
            chunk_type: "paragraph".to_string(),
            char_count: tcount,
        });
    }
    chunks
}

fn get_query_from_extension(extension: &str) -> Option<String> {
    match extension {
        "rs" => Some(
            r#"
            ;; Rust top-level items
            (function_item) @chunk
            (struct_item) @chunk
            (impl_item) @chunk
            (mod_item) @chunk
            (enum_item) @chunk
            (trait_item) @chunk
            "#
            .to_string(),
        ),
        "py" => Some(
            r#"
            ;; Python top-level definitions
            (function_definition) @chunk
            (class_definition) @chunk
            "#
            .to_string(),
        ),
        "js" => Some(
            r#"
            ;; JavaScript / TypeScript top-level definitions
            (function_declaration) @chunk
            (arrow_function) @chunk
            (class_declaration) @chunk
            (method_definition) @chunk
            (variable_declaration) @chunk
            "#
            .to_string(),
        ),
        "c" => Some(
            r#"
            ;; C top-level items
            (function_definition) @chunk
            (struct_specifier) @chunk
            (union_specifier) @chunk
            (enum_specifier) @chunk
            (declaration
                (type_specifier) @type_decl
            ) @chunk
            "#
            .to_string(),
        ),
        "cpp" | "cu" => Some(
            r#"
            ;; C++ top-level items
            (function_definition) @chunk
            (class_specifier) @chunk
            (struct_specifier) @chunk
            (enum_specifier) @chunk
            (template_declaration) @chunk
            (declaration
                (type_specifier) @type_decl
            ) @chunk
            "#
            .to_string(),
        ),
        "html" => Some(
            r#"
            ;; HTML fallback: treat entire file as a single chunk
            (element) @chunk
            "#
            .to_string(),
        ),
        _ => None,
    }
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
            .unwrap_or("".to_string()),
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

use std::fmt::Write as FmtWrite;

pub fn print_document_names(docs: &[Document]) {
    if docs.is_empty() {
        println!("No documents found.");
        return;
    }

    let mut output = String::with_capacity(docs.len() * 64);
    writeln!(&mut output, "Found {} documents:", docs.len()).unwrap();
    writeln!(&mut output, "------------------------").unwrap();

    for (i, doc) in docs.iter().enumerate() {
        writeln!(&mut output, "{:>3}. {}", i + 1, doc.path).unwrap();
    }

    println!("{output}");
}
