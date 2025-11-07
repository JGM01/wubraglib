extern crate wubrag;

use std::path::Path;

use wubrag::*;

#[test]
fn test_print_documents() {
    let p = Path::new("tests/examples/ladybird");
    let _ = grab_all_documents_optimized(p);
    assert!(true);
}
//#[test]
fn test_print_stats() {
    let p = Path::new("tests/examples/dolphin");
    let q = Path::new("tests/examples/ladybird");
    run_all(p);
    run_all(q);
    assert!(true);
}
