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
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, QueryMatch, QueryMatches};

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
        m.insert("rs", tree_sitter_rust::language());
        m.insert("toml", tree_sitter_toml::language());
        m
    };
}

pub fn print_chunks_tree(chunks: &[Chunk], chunk_index: Option<&HashMap<u32, usize>>) {
    let roots: Vec<u32> = chunks
        .iter()
        .filter_map(|c| c.parent_id.is_none().then_some(c.id))
        .collect();

    if roots.is_empty() {
        println!("No root chunks found.");
        return;
    }

    println!("Chunk Hierarchy Tree ({} total chunks):\n", chunks.len());
    for &root_id in &roots {
        let root_idx = chunk_index
            .as_ref()
            .and_then(|idx| idx.get(&root_id).copied())
            .unwrap_or_else(|| chunks.iter().position(|c| c.id == root_id).unwrap_or(0));
        print_chunk_recursive(&chunks[root_idx], chunks, chunk_index, 0);
        println!();
    }
}

fn print_chunk_recursive(
    chunk: &Chunk,
    chunks: &[Chunk],
    chunk_index: Option<&HashMap<u32, usize>>,
    indent: usize,
) {
    let indent_str = "  ".repeat(indent);
    let text_preview = if chunk.text.len() > 100 {
        format!("{}...", &chunk.text[0..100])
    } else {
        chunk.text.clone()
    };

    println!(
        "{}{} [{}]: {} tokens, {} children (parent: {:?})",
        indent_str,
        chunk.id,
        chunk.chunk_type,
        chunk.token_count,
        chunk.children_ids.len(),
        chunk.parent_id
    );
    println!("{}  Text: \"{}\"", indent_str, text_preview.trim());
    println!("{}  ---", indent_str);

    // Recurse on children
    for &child_id in &chunk.children_ids {
        let child_idx = chunk_index
            .as_ref()
            .and_then(|idx| idx.get(&child_id).copied())
            .unwrap_or_else(|| chunks.iter().position(|c| c.id == child_id).unwrap_or(0));
        if let Some(child_chunk) = chunks.get(child_idx) {
            print_chunk_recursive(child_chunk, chunks, chunk_index, indent + 1);
        } else {
            println!(
                "{}  [WARNING: Missing child chunk ID {}]",
                indent_str, child_id
            );
        }
    }
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
        parser.set_language(*lang).expect("Bad language!");
        let tree = parser.parse(doc_text, None).expect("Parse failed");
        let root = tree.root_node();

        let query_str = get_query_from_extension(doc_ext).unwrap_or("".to_string());
        let query = Query::new(*lang, &query_str).expect("invalid query");

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
                let children = traverse_and_chunk(&node, doc_text, next_id, doc_id, None);

                chunks.extend(children);
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
    let id = next_id.fetch_add(1, Ordering::SeqCst);

    let mut children = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if is_chunk_worthy(child.kind()) {
            let sub_chunks = traverse_and_chunk(&child, doc_text, next_id, doc_id, Some(id));
            children.extend(sub_chunks);
        }
    }

    let start = node.start_byte() as usize;
    let end = node.end_byte() as usize;
    let chunk_text = &doc_text[start..end];
    let token_count = o200k_base().unwrap().encode_ordinary(chunk_text).len();

    let chunk = Chunk {
        id,
        doc_id,
        text: chunk_text.to_string(),
        chunk_type: node.kind().to_string(),
        parent_id,
        children_ids: children.iter().map(|c| c.id).collect(),
        token_count,
    };

    let mut all = Vec::with_capacity(children.len() + 1);
    all.extend(children);
    all.push(chunk);
    all
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
fn is_chunk_worthy(kind: &str) -> bool {
    if kind.contains("function")
        || kind.contains("method")
        || kind.contains("class")
        || kind == "struct_item"
        || kind == "impl_item"
        || kind == "mod_item"
        || kind == "enum_item"
        || kind == "trait_item"
        || kind == "variable_declaration"
        || kind == "arguments"
        || kind == "expression_statement"
    {
        return true;
    }

    if kind == "table" || kind == "dotted_key" || kind == "array" || kind == "inline_table" {
        return true;
    }

    if kind == "arrow_function" {
        return true;
    }

    false
}

fn get_query_from_extension(extension: &str) -> Option<String> {
    match extension {
        "rs" => Some(
            r#"
            ;; Rust: Semantic items
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
            ;; Python: Functions, classes, top-level statements
            (function_definition) @chunk
            (class_definition) @chunk
            (arguments) @chunk  ;; For function signatures if needed
            (expression_statement) @chunk  ;; Fallback for loose code
            "#
            .to_string(),
        ),
        "js" => Some(
            r#"
            ;; JavaScript: Functions, classes, etc.
            (function) @chunk
            (arrow_function) @chunk
            (class_declaration) @chunk
            (method_definition) @chunk
            (variable_declaration) @chunk  ;; For top-level vars
            "#
            .to_string(),
        ),
        "toml" => Some(
            r#"
            ;; TOML: Config structures
            (table) @chunk          ;; [table] sections
            (dotted_key) @chunk     ;; [table.subtable]
            (inline_table) @chunk   ;; Inline { } tables
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
