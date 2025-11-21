use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;

use wubrag::document::grab_all_documents;

fn bench_grab_for_dir(c: &mut Criterion, name: &str, dir: &str) {
    let root_path = Path::new(dir);

    c.bench_function(name, |b| {
        b.iter(|| {
            let _ = grab_all_documents(std::hint::black_box(&root_path));
        })
    });
}

fn bench_grab_coreutils(c: &mut Criterion) {
    bench_grab_for_dir(c, "grab_coreutils", "tests/examples/coreutils");
}

fn bench_grab_ladybird(c: &mut Criterion) {
    bench_grab_for_dir(c, "grab_ladybird", "tests/examples/ladybird");
}

fn bench_grab_dolphin(c: &mut Criterion) {
    bench_grab_for_dir(c, "grab_dolphin", "tests/examples/dolphin");
}

fn bench_grab_ratatui(c: &mut Criterion) {
    bench_grab_for_dir(c, "grab_ratatui", "tests/examples/ratatui");
}

criterion_group! {
    name = doc_benches;
    config = Criterion::default().sample_size(10);
    targets =
        bench_grab_ratatui,
        bench_grab_dolphin,
        bench_grab_ladybird,
        bench_grab_coreutils
}

criterion_main!(doc_benches);
