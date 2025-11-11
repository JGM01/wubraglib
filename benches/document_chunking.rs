use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;
use wubrag::*;

fn bench_chunk_coreutils(c: &mut Criterion) {
    let root_path = Path::new("coreutils");
    let docs = grab_all_documents(std::hint::black_box(&root_path));

    c.bench_function("chunk_coreutils", |b| {
        b.iter(|| {
            let _ = chunk_all_documents(std::hint::black_box(&docs));
        })
    });
}
fn bench_chunk_ladybird(c: &mut Criterion) {
    let root_path = Path::new("ladybird");
    let docs = grab_all_documents(std::hint::black_box(&root_path));

    c.bench_function("chunk_ladybird", |b| {
        b.iter(|| {
            let _ = chunk_all_documents(std::hint::black_box(&docs));
        })
    });
}
fn bench_chunk_dolphin(c: &mut Criterion) {
    let root_path = Path::new("dolphin");
    let docs = grab_all_documents(std::hint::black_box(&root_path));

    c.bench_function("chunk_dolphin", |b| {
        b.iter(|| {
            let _ = chunk_all_documents(std::hint::black_box(&docs));
        })
    });
}
fn bench_chunk_ratatui(c: &mut Criterion) {
    let root_path = Path::new("ratatui");
    let docs = grab_all_documents(std::hint::black_box(&root_path));

    c.bench_function("chunk_ratatui", |b| {
        b.iter(|| {
            let _ = chunk_all_documents(std::hint::black_box(&docs));
        })
    });
}

criterion_group! {
    name = doc_benches;
    config = Criterion::default().sample_size(10);
    targets = bench_chunk_ratatui, bench_chunk_dolphin, bench_chunk_ladybird, bench_chunk_coreutils
}

criterion_main!(doc_benches);
