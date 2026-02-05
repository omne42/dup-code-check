use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::types::{DuplicateSpanOccurrence, ScanOptions, SimilarityPair};
use crate::util::fnv1a64_u32;

use super::super::ScannedTextFile;
use super::repo_label_arc;

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

pub(in crate::report) fn find_similar_blocks_minhash(
    repo_labels: &[Arc<str>],
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const SIG_SIZE: usize = 32;
    const BAND_SIZE: usize = 4;
    const BANDS: usize = SIG_SIZE / BAND_SIZE;

    let seeds: [u64; SIG_SIZE] = {
        let mut out = [0u64; SIG_SIZE];
        let mut s = 0x1234_5678_9abc_def0u64;
        for v in &mut out {
            s = splitmix64(s);
            *v = s;
        }
        out
    };

    #[derive(Debug)]
    struct BlockSig {
        occ: DuplicateSpanOccurrence,
        signature: [u32; SIG_SIZE],
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut mins = [u32::MAX; SIG_SIZE];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                for i in 0..SIG_SIZE {
                    let h = splitmix64(base ^ seeds[i]) as u32;
                    if h < mins[i] {
                        mins[i] = h;
                    }
                }
            }

            blocks.push(BlockSig {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: repo_label_arc(repo_labels, file.repo_id),
                    path: Arc::clone(&file.path),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                signature: mins,
            });
        }
    }

    let mut buckets: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let start = band * BAND_SIZE;
            let key_hash = fnv1a64_u32(&blk.signature[start..start + BAND_SIZE]);
            buckets.entry((band, key_hash)).or_default().push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let sig_a = &blocks[key.0].signature;
                let sig_b = &blocks[key.1].signature;
                let eq = sig_a.iter().zip(sig_b).filter(|(x, y)| x == y).count();
                let score = eq as f64 / SIG_SIZE as f64;
                if score < options.similarity_threshold {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: None,
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}

pub(in crate::report) fn find_similar_blocks_simhash(
    repo_labels: &[Arc<str>],
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const BANDS: usize = 4;
    const BAND_BITS: u32 = 16;

    #[derive(Debug)]
    struct BlockHash {
        occ: DuplicateSpanOccurrence,
        hash: u64,
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut sums = [0i32; 64];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                let h = splitmix64(base);
                for (bit, sum) in sums.iter_mut().enumerate() {
                    if (h >> bit) & 1 == 1 {
                        *sum += 1;
                    } else {
                        *sum -= 1;
                    }
                }
            }

            let mut hash = 0u64;
            for (bit, sum) in sums.iter().enumerate() {
                if *sum > 0 {
                    hash |= 1u64 << bit;
                }
            }

            blocks.push(BlockHash {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: repo_label_arc(repo_labels, file.repo_id),
                    path: Arc::clone(&file.path),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                hash,
            });
        }
    }

    let mut buckets: HashMap<(u32, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let shift = (band as u32) * BAND_BITS;
            let band_value = (blk.hash >> shift) & 0xffff;
            buckets
                .entry((band as u32, band_value))
                .or_default()
                .push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let hamming = (blocks[key.0].hash ^ blocks[key.1].hash).count_ones();
                if hamming > options.simhash_max_distance {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                let score = 1.0 - (hamming as f64 / 64.0);
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: Some(hamming),
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}
