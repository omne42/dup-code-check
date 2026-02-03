use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub ignore_dirs: HashSet<String>,
    pub max_file_size: Option<u64>,
    pub max_files: Option<usize>,
    pub max_total_bytes: Option<u64>,
    pub min_match_len: usize,
    pub min_token_len: usize,
    pub similarity_threshold: f64,
    pub simhash_max_distance: u32,
    pub max_report_items: usize,
    pub respect_gitignore: bool,
    pub cross_repo_only: bool,
    pub follow_symlinks: bool,
}

pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            ignore_dirs: default_ignore_dirs(),
            max_file_size: Some(DEFAULT_MAX_FILE_SIZE_BYTES),
            max_files: None,
            max_total_bytes: None,
            min_match_len: 50,
            min_token_len: 50,
            similarity_threshold: 0.85,
            simhash_max_distance: 3,
            max_report_items: 200,
            respect_gitignore: true,
            cross_repo_only: false,
            follow_symlinks: false,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanStats {
    pub candidate_files: u64,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub skipped_not_found: u64,
    pub skipped_permission_denied: u64,
    pub skipped_too_large: u64,
    pub skipped_binary: u64,
    pub skipped_outside_root: u64,
    pub skipped_walk_errors: u64,
    pub skipped_budget_max_files: u64,
    pub skipped_budget_max_total_bytes: u64,
    pub skipped_bucket_truncated: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome<T> {
    pub result: T,
    pub stats: ScanStats,
}

pub fn default_ignore_dirs() -> HashSet<String> {
    [
        ".git",
        ".hg",
        ".svn",
        "node_modules",
        "target",
        "dist",
        "build",
        "out",
        ".next",
        ".turbo",
        ".cache",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFile {
    pub repo_id: usize,
    pub repo_label: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub content_hash: u64,
    pub normalized_len: usize,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSpanOccurrence {
    pub repo_id: usize,
    pub repo_label: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSpanGroup {
    pub content_hash: u64,
    pub normalized_len: usize,
    pub preview: String,
    pub occurrences: Vec<DuplicateSpanOccurrence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimilarityPair {
    pub a: DuplicateSpanOccurrence,
    pub b: DuplicateSpanOccurrence,
    pub score: f64,
    pub distance: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicationReport {
    pub file_duplicates: Vec<DuplicateGroup>,
    pub code_span_duplicates: Vec<DuplicateSpanGroup>,
    pub line_span_duplicates: Vec<DuplicateSpanGroup>,
    pub token_span_duplicates: Vec<DuplicateSpanGroup>,
    pub block_duplicates: Vec<DuplicateSpanGroup>,
    pub ast_subtree_duplicates: Vec<DuplicateSpanGroup>,
    pub similar_blocks_minhash: Vec<SimilarityPair>,
    pub similar_blocks_simhash: Vec<SimilarityPair>,
}
