use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;

use wubrag::{chunking::Chunker, document::grab_all_documents, embedding::Embedder};

fn bench_embed_for_dir(c: &mut Criterion, name: &str, dir: &str) {
    let root_path = Path::new(dir);

    let docs = grab_all_documents(std::hint::black_box(&root_path))
        .map_err(|e| {
            eprintln!(
                "Failed to load documents from {}: {}",
                root_path.display(),
                e
            )
        })
        .unwrap();

    let chunker = Chunker::new();
    let (mut chunks, _) = chunker.chunk_all_documents(std::hint::black_box(&docs));

    let mut embedder = Embedder::new();

    c.bench_function(name, |b| {
        b.iter(|| {
            let _ = embedder.embed_chunks(std::hint::black_box(&mut chunks));
        })
    });
}

fn bench_embed_coreutils(c: &mut Criterion) {
    bench_embed_for_dir(c, "embed_coreutils", "tests/examples/coreutils/src/bin");
}

fn bench_embed_ladybird(c: &mut Criterion) {
    bench_embed_for_dir(
        c,
        "embed_ladybird",
        "tests/examples/ladybird/Services/WebDriver",
    );
}

fn bench_embed_dolphin(c: &mut Criterion) {
    bench_embed_for_dir(
        c,
        "embed_dolphin",
        "tests/examples/dolphin/Source/Core/WinUpdater/",
    );
}

fn bench_embed_ratatui(c: &mut Criterion) {
    bench_embed_for_dir(c, "embed_ratatui", "tests/examples/ratatui/src/buffer");
}

criterion_group! {
    name = doc_benches;
    config = Criterion::default();
    targets =
        bench_embed_ratatui,
        bench_embed_dolphin,
        bench_embed_ladybird,
        bench_embed_coreutils
}

criterion_main!(doc_benches);
