use lazy_static::lazy_static;
use rayon::prelude::*;
use sha2::Digest;
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use crate::document::{Document, DocumentID};

pub type ChunkID = [u8; 32];

fn compute_chunk_id(doc_id: &DocumentID, chunk_text: &str) -> ChunkID {
    let mut hash = sha2::Sha256::new();
    hash.update(doc_id);
    hash.update(chunk_text.as_bytes());
    hash.finalize().into()
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: ChunkID,              // primary key
    pub doc_id: DocumentID,       // foreign key id of the document that the chunk is attached to
    pub text: String,             // content of the chunk
    pub chunk_type: &'static str, // whatever is returned by node.kind() with tree-sitter (or "paragraph"/"document")
    pub char_count: usize,        // amount of characters
}

pub fn chunk_all_documents(docs: &[Document]) -> (Vec<Chunk>, HashMap<DocumentID, usize>) {
    let chunks: Vec<Chunk> = docs.par_iter().flat_map(chunk_document).collect();

    let id_to_idx: HashMap<ChunkID, usize> =
        chunks.iter().enumerate().map(|(i, c)| (c.id, i)).collect();

    (chunks, id_to_idx)
}

fn chunk_document(doc: &Document) -> Vec<Chunk> {
    if let Some(lang) = LANGUAGE_MAP.get(&doc.ext.as_str()) {
        chunk_with_treesitter(&doc, lang)
    } else {
        naive_chunk_document(&doc.text, doc.id)
    }
}

lazy_static! {
    pub static ref LANGUAGE_MAP: HashMap<&'static str, Language> = {
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

fn chunk_with_treesitter(doc: &Document, lang: &Language) -> Vec<Chunk> {
    let mut chunks = vec![];

    let mut parser = Parser::new();
    parser.set_language(lang).expect("Bad language for parser");
    let tree = match parser.parse(&doc.text, None) {
        Some(t) => t,
        None => {
            return naive_chunk_document(&doc.text, doc.id);
        }
    };
    let root = tree.root_node();

    // Get both container and function queries
    let (container_query_str, function_query_str) = get_queries_from_extension(&doc.ext);

    // Process container-level chunks (classes, structs, etc.)
    if let Some(container_str) = container_query_str {
        if let Ok(query) = Query::new(lang, &container_str) {
            let mut cursor = QueryCursor::new();
            let b_text = doc.text.as_bytes();
            let mut qmatches = cursor.matches(&query, root, b_text);

            while let Some(m) = qmatches.next() {
                for capture in m.captures {
                    let node = capture.node;

                    let is_top_level = node
                        .parent()
                        .map(|p| {
                            p.kind() == "source_file"
                                || p.kind() == "module"
                                || p.kind() == "program"
                        })
                        .unwrap_or(false);

                    if !is_top_level {
                        continue;
                    }

                    let raw_text = node.utf8_text(doc.text.as_bytes()).ok().expect(":D");
                    if raw_text.trim().is_empty() {
                        continue;
                    }

                    let id = compute_chunk_id(&doc.id, raw_text);

                    chunks.push(Chunk {
                        id,
                        doc_id: doc.id,
                        text: raw_text.trim().to_string(),
                        chunk_type: node.kind(),
                        char_count: raw_text.len(),
                    });
                }
            }
        }
    }

    // Process function-level chunks
    if let Some(function_str) = function_query_str {
        if let Ok(query) = Query::new(lang, &function_str) {
            let mut cursor = QueryCursor::new();
            let b_text = doc.text.as_bytes();
            let mut qmatches = cursor.matches(&query, root, b_text);

            while let Some(m) = qmatches.next() {
                for capture in m.captures {
                    let node = capture.node;

                    let is_top_level = node
                        .parent()
                        .map(|p| {
                            p.kind() == "source_file"
                                || p.kind() == "module"
                                || p.kind() == "program"
                        })
                        .unwrap_or(false);

                    if !is_top_level {
                        continue;
                    }

                    let raw_text = node.utf8_text(doc.text.as_bytes()).ok().expect(":D");
                    if raw_text.trim().is_empty() {
                        continue;
                    }

                    let id = compute_chunk_id(&doc.id, raw_text);

                    chunks.push(Chunk {
                        id,
                        doc_id: doc.id,
                        text: raw_text.trim().to_string(),
                        chunk_type: node.kind(),
                        char_count: raw_text.len(),
                    });
                }
            }
        }
    }

    if chunks.is_empty() {
        let id = compute_chunk_id(&doc.id, &doc.text.to_string());
        chunks.push(Chunk {
            id,
            doc_id: doc.id,
            text: doc.text.trim().to_string(),
            chunk_type: "document",
            char_count: doc.text.len(),
        });
    }

    chunks
}

fn naive_chunk_document(doc_text: &str, doc_id: DocumentID) -> Vec<Chunk> {
    let mut chunks = vec![];
    for para in doc_text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let id = compute_chunk_id(&doc_id, &para.to_string());
        let tcount = para.len();
        chunks.push(Chunk {
            id,
            doc_id,
            text: para.to_string(),
            chunk_type: "paragraph",
            char_count: tcount,
        });
    }

    if chunks.is_empty() {
        let id = compute_chunk_id(&doc_id, &doc_text.to_string());
        chunks.push(Chunk {
            id,
            doc_id,
            text: doc_text.trim().to_string(),
            chunk_type: "document",
            char_count: doc_text.len(),
        });
    }

    chunks
}

// Returns (container_query, function_query)
fn get_queries_from_extension(extension: &str) -> (Option<String>, Option<String>) {
    match extension {
        "rs" => (
            // Container-level chunks
            Some(
                r#"
                ;; Rust container items
                (struct_item) @chunk
                (impl_item) @chunk
                (mod_item) @chunk
                (enum_item) @chunk
                (trait_item) @chunk
                "#
                .to_string(),
            ),
            // Function-level chunks
            Some(
                r#"
                ;; Rust functions
                (function_item) @chunk
                "#
                .to_string(),
            ),
        ),
        "py" => (
            // Container-level chunks
            Some(
                r#"
                ;; Python classes
                (class_definition) @chunk
                "#
                .to_string(),
            ),
            // Function-level chunks
            Some(
                r#"
                ;; Python functions
                (function_definition) @chunk
                "#
                .to_string(),
            ),
        ),
        "js" => (
            // Container-level chunks
            Some(
                r#"
                ;; JavaScript/TypeScript classes
                (class_declaration) @chunk
                "#
                .to_string(),
            ),
            // Function-level chunks
            Some(
                r#"
                ;; JavaScript/TypeScript functions
                (function_declaration) @chunk
                (arrow_function) @chunk
                (method_definition) @chunk
                (variable_declaration) @chunk
                "#
                .to_string(),
            ),
        ),
        "c" => (
            // Container-level chunks
            Some(
                r#"
                ;; C containers
                (struct_specifier) @chunk
                (union_specifier) @chunk
                (enum_specifier) @chunk
                "#
                .to_string(),
            ),
            // Function-level chunks
            Some(
                r#"
                ;; C functions
                (function_definition) @chunk
                (declaration
                    (type_specifier) @type_decl
                ) @chunk
                "#
                .to_string(),
            ),
        ),
        "cpp" | "cu" => (
            // Container-level chunks
            Some(
                r#"
                ;; C++/CUDA containers
                (class_specifier) @chunk
                (struct_specifier) @chunk
                (enum_specifier) @chunk
                (namespace_definition) @chunk
                "#
                .to_string(),
            ),
            // Function-level chunks
            Some(
                r#"
                ;; C++/CUDA functions
                (function_definition) @chunk
                (template_declaration) @chunk
                (declaration
                    (type_specifier) @type_decl
                ) @chunk
                "#
                .to_string(),
            ),
        ),
        "html" => (
            Some(
                r#"
                ;; HTML fallback: treat top-level elements as chunks
                (element) @chunk
                "#
                .to_string(),
            ),
            None,
        ),
        _ => (None, None),
    }
}
