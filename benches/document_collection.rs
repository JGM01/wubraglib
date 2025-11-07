use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;

use wubrag::{grab_all_documents, grab_all_documents_optimized};

fn bench_grab_documents(c: &mut Criterion) {
    let root_path = Path::new("tests/examples/ladybird");

    c.bench_function("grab_all_documents", |b| {
        b.iter(|| {
            let docs = grab_all_documents(std::hint::black_box(&root_path));
            std::hint::black_box(docs);
        })
    });
}

fn bench_grab_documents_optimized(c: &mut Criterion) {
    let root_path = Path::new("tests/examples/ladybird");

    c.bench_function("grab_all_documents_optimized", |b| {
        b.iter(|| {
            let docs = grab_all_documents_optimized(std::hint::black_box(&root_path));
            std::hint::black_box(docs);
        })
    });
}

criterion_group! {
    name = doc_benches;
    config = Criterion::default().sample_size(30);
    targets = bench_grab_documents, bench_grab_documents_optimized
}

criterion_main!(doc_benches);
