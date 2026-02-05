use std::collections::HashSet;
use std::io;
use std::sync::Arc;

/// Scan configuration shared by the CLI and the core APIs.
///
/// This struct is `#[non_exhaustive]` so new options can be added without breaking callers.
/// Construct it via `ScanOptions::default()` and then override fields as needed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScanOptions {
    pub ignore_dirs: HashSet<String>,
    pub max_file_size: Option<u64>,
    pub max_files: Option<usize>,
    pub max_total_bytes: Option<u64>,
    pub max_normalized_chars: Option<usize>,
    pub max_tokens: Option<usize>,
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
            max_normalized_chars: None,
            max_tokens: None,
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

impl ScanOptions {
    /// Validate scan options for all detectors (strictest).
    ///
    /// This is equivalent to [`ScanOptions::validate_for_report`], since report mode exercises all
    /// threshold-related options.
    ///
    /// For narrower use, prefer:
    /// - [`ScanOptions::validate_for_file_duplicates`]
    /// - [`ScanOptions::validate_for_code_spans`]
    pub fn validate(&self) -> io::Result<()> {
        self.validate_for_report()
    }

    /// Validate options used by file-duplicate scanning.
    pub fn validate_for_file_duplicates(&self) -> io::Result<()> {
        Ok(())
    }

    /// Validate options used by code-span scanning.
    pub fn validate_for_code_spans(&self) -> io::Result<()> {
        if self.min_match_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "min_match_len must be >= 1",
            ));
        }
        Ok(())
    }

    /// Validate options used by report generation.
    pub fn validate_for_report(&self) -> io::Result<()> {
        self.validate_for_code_spans()?;

        if self.min_token_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "min_token_len must be >= 1",
            ));
        }

        let threshold = self.similarity_threshold;
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "similarity_threshold must be finite and in 0..=1",
            ));
        }

        if self.simhash_max_distance > 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "simhash_max_distance must be in 0..=64",
            ));
        }

        Ok(())
    }
}

/// Scan statistics collected during scanning/report generation.
///
/// This struct is `#[non_exhaustive]` so new counters can be added without breaking callers.
/// Construct it via `ScanStats::default()` and then read/update fields as needed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScanStats {
    pub candidate_files: u64,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub git_fast_path_fallbacks: u64,
    pub skipped_not_found: u64,
    pub skipped_permission_denied: u64,
    pub skipped_too_large: u64,
    pub skipped_binary: u64,
    pub skipped_outside_root: u64,
    pub skipped_relativize_failed: u64,
    pub skipped_walk_errors: u64,
    pub skipped_budget_max_files: u64,
    pub skipped_budget_max_total_bytes: u64,
    pub skipped_budget_max_normalized_chars: u64,
    pub skipped_budget_max_tokens: u64,
    pub skipped_bucket_truncated: u64,
}

impl ScanStats {
    #[must_use]
    pub fn has_fatal_skips(&self) -> bool {
        self.skipped_permission_denied > 0
            || self.skipped_outside_root > 0
            || self.skipped_relativize_failed > 0
            || self.skipped_walk_errors > 0
            || self.skipped_bucket_truncated > 0
            || self.skipped_budget_max_files > 0
            || self.skipped_budget_max_total_bytes > 0
            || self.skipped_budget_max_normalized_chars > 0
            || self.skipped_budget_max_tokens > 0
    }
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
    pub(crate) repo_id: usize,
    pub(crate) repo_label: Arc<str>,
    pub(crate) path: Arc<str>,
}

impl DuplicateFile {
    pub fn repo_id(&self) -> usize {
        self.repo_id
    }

    pub fn repo_label(&self) -> &str {
        self.repo_label.as_ref()
    }

    pub fn path(&self) -> &str {
        self.path.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub content_hash: u64,
    pub normalized_len: usize,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSpanOccurrence {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: Arc<str>,
    pub(crate) path: Arc<str>,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
}

impl DuplicateSpanOccurrence {
    pub fn repo_id(&self) -> usize {
        self.repo_id
    }

    pub fn repo_label(&self) -> &str {
        self.repo_label.as_ref()
    }

    pub fn path(&self) -> &str {
        self.path.as_ref()
    }

    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    pub fn end_line(&self) -> u32 {
        self.end_line
    }
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
