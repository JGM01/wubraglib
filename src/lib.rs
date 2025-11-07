use std::path::Path;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub text: String,
    pub meta: HashMap<String, String>,
}

use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::WalkDir as WalkDir2;

fn f2doc(root: &Path, relative_path: &Path) -> Option<Document> {
    let path = root.join(relative_path);

    let text = match std::fs::read_to_string(&relative_path) {
        Ok(txt) => txt,
        Err(_) => return None,
    };
    let mut meta = HashMap::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        meta.insert("extension".to_string(), ext.to_string());
    }
    meta.insert(
        "size_bytes".to_string(),
        path.metadata().expect("oopsie").len().to_string(),
    );

    Some(Document {
        id: relative_path.display().to_string(),
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

pub fn print_document_stats(docs: &[Document]) {
    if docs.is_empty() {
        println!("No documents to analyze.");
        return;
    }

    let mut stats = String::new();
    stats.push_str(&format!(
        "Document Statistics ({} documents):\n",
        docs.len()
    ));
    stats.push_str("----------------------------------------\n");

    // Count by extension
    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    let mut total_size: u64 = 0;
    let mut has_mod_time = false;
    let mut min_mod_secs: Option<u64> = None;
    let mut max_mod_secs: Option<u64> = None;

    for doc in docs {
        // Extension count
        if let Some(ext) = doc.meta.get("extension") {
            *ext_counts.entry(ext.clone()).or_insert(0) += 1;
        }

        // Total size
        if let Some(size_str) = doc.meta.get("size_bytes") {
            if let Ok(size) = size_str.parse::<u64>() {
                total_size += size;
            }
        }

        // Modification times
        if let Some(mod_str) = doc.meta.get("modified_ago_secs") {
            if let Ok(mod_secs) = mod_str.parse::<u64>() {
                has_mod_time = true;
                min_mod_secs = Some(min_mod_secs.unwrap_or(mod_secs).min(mod_secs));
                max_mod_secs = Some(max_mod_secs.unwrap_or(mod_secs).max(mod_secs));
            }
        }
    }

    // Total and average size
    stats.push_str(&format!("Total size: {} bytes\n", total_size));
    let avg_size = if docs.len() > 0 {
        total_size / docs.len() as u64
    } else {
        0
    };
    stats.push_str(&format!("Average size: {} bytes\n", avg_size));

    // Extension counts (sorted alphabetically by extension)
    if !ext_counts.is_empty() {
        let mut ext_vec: Vec<(&String, &usize)> = ext_counts.iter().collect();
        ext_vec.sort_by_key(|(ext, _)| *ext);
        stats.push_str("Documents by extension:\n");
        for (ext, count) in ext_vec {
            stats.push_str(&format!("  .{}: {} documents\n", ext, count));
        }
        stats.push_str("\n");
    }

    // Modification times
    if has_mod_time && min_mod_secs.is_some() && max_mod_secs.is_some() {
        stats.push_str(&format!(
            "Modification age range: {} to {} seconds ago\n",
            min_mod_secs.unwrap(),
            max_mod_secs.unwrap()
        ));
    }

    println!("{}", stats.trim_end());
}

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Basic repo statistics: file count, total bytes, average file size
pub fn repo_stats(root: &Path) {
    let mut count = 0usize;
    let mut total = 0u64;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        count += 1;
        if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }

    let avg = if count > 0 { total / count as u64 } else { 0 };
    println!(
        "Repo stats for {:?}:\n  files: {}\n  total bytes: {}\n  avg file size: {} bytes",
        root, count, total, avg
    );
}

/// Measure time spent on directory traversal only (no file reading)
pub fn measure_walk_time(root: &Path) {
    let start = Instant::now();
    let paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .collect();
    let elapsed = start.elapsed();
    println!(
        "Walked {} files under {:?} in {:.2?}",
        paths.len(),
        root,
        elapsed
    );
}

/// Measure serial read performance (read all files into memory one by one)
pub fn measure_serial_reads(root: &Path) {
    let paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .collect();

    let start = Instant::now();
    let mut total_bytes = 0usize;
    for p in &paths {
        if let Ok(data) = fs::read(p) {
            total_bytes += data.len();
        }
    }
    let elapsed = start.elapsed();
    println!(
        "Serially read {} files ({} bytes total) in {:.2?}",
        paths.len(),
        total_bytes,
        elapsed
    );
}

/// Measure parallel read performance using rayon
pub fn measure_parallel_reads(root: &Path, threads: Option<usize>) {
    if let Some(n) = threads {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    let paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .collect();

    let start = Instant::now();
    let total_bytes: usize = paths
        .par_iter()
        .filter_map(|p| fs::read(p).ok())
        .map(|data| data.len())
        .sum();
    let elapsed = start.elapsed();

    println!(
        "Parallel read ({} threads): {} files ({} bytes total) in {:.2?}",
        threads
            .map(|n| n.to_string())
            .unwrap_or_else(|| "default".into()),
        paths.len(),
        total_bytes,
        elapsed
    );
}

/// Convenience function to run all experiments together
pub fn run_all(root: &Path) {
    println!("=== Running repository I/O experiments for {:?} ===", root);
    repo_stats(root);
    measure_walk_time(root);
    measure_serial_reads(root);
    measure_parallel_reads(root, None);
    measure_parallel_reads(root, Some(4)); // 4-thread test for contrast
    println!("=== Done ===\n");
}
