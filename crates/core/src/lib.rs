mod duplicates;
mod report;
mod scan;
mod tokenize;
mod types;
mod util;

pub use duplicates::{
    find_duplicate_code_spans, find_duplicate_code_spans_with_stats, find_duplicate_files,
    find_duplicate_files_with_stats,
};

pub use report::{generate_duplication_report, generate_duplication_report_with_stats};

pub use types::{
    DEFAULT_MAX_FILE_SIZE_BYTES, DuplicateFile, DuplicateGroup, DuplicateSpanGroup,
    DuplicateSpanOccurrence, DuplicationReport, ScanOptions, ScanOutcome, ScanStats,
    SimilarityPair, default_ignore_dirs,
};
