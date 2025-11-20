use std::collections::HashMap;

use rayon::{
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

use crate::chunking::{Chunk, ChunkID};

pub struct Index {
    pub chunks: Vec<Chunk>,
    pub embeddings: Vec<Vec<f32>>,
    id_to_idx: HashMap<ChunkID, usize>,
}

impl Index {
    pub fn new(chunks: Vec<Chunk>, embeddings: Vec<Vec<f32>>) -> Self {
        let id_to_idx = chunks.iter().enumerate().map(|(i, c)| (c.id, i)).collect();

        Self {
            chunks,
            embeddings,
            id_to_idx,
        }
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let mut scored: Vec<_> = self
            .embeddings
            .par_iter()
            .enumerate()
            .map(|(i, emb)| (i, cosine(query, emb)))
            .collect();

        scored.par_sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(k);
        scored
    }

    pub fn retrieve(&self, idx: usize) -> &Chunk {
        &self.chunks[idx]
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }

    dot / (na.sqrt() * nb.sqrt())
}
