use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;

use wubrag::{chunking::Chunker, document::grab_all_documents};

fn bench_chunk_for_dir(c: &mut Criterion, name: &str, dir: &str) {
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

    c.bench_function(name, |b| {
        b.iter(|| {
            let _ = chunker.chunk_all_documents(std::hint::black_box(&docs));
        })
    });
}

fn bench_chunk_coreutils(c: &mut Criterion) {
    bench_chunk_for_dir(c, "chunk_coreutils", "tests/examples/coreutils");
}

fn bench_chunk_ladybird(c: &mut Criterion) {
    bench_chunk_for_dir(c, "chunk_ladybird", "tests/examples/ladybird");
}

fn bench_chunk_dolphin(c: &mut Criterion) {
    bench_chunk_for_dir(c, "chunk_dolphin", "tests/examples/dolphin");
}

fn bench_chunk_ratatui(c: &mut Criterion) {
    bench_chunk_for_dir(c, "chunk_ratatui", "tests/examples/ratatui");
}

criterion_group! {
    name = doc_benches;
    config = Criterion::default().sample_size(10);
    targets =
        bench_chunk_ratatui,
        bench_chunk_dolphin,
        bench_chunk_ladybird,
        bench_chunk_coreutils
}

criterion_main!(doc_benches);
