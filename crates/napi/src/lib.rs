use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
pub struct ScanOptions {
    pub ignore_dirs: Option<Vec<String>>,
    pub max_file_size: Option<f64>,
    pub min_match_len: Option<f64>,
    pub min_token_len: Option<f64>,
    pub similarity_threshold: Option<f64>,
    pub simhash_max_distance: Option<f64>,
    pub max_report_items: Option<f64>,
    pub respect_gitignore: Option<bool>,
    pub cross_repo_only: Option<bool>,
    pub follow_symlinks: Option<bool>,
}

#[napi(object)]
pub struct DuplicateFile {
    pub repo_id: u32,
    pub repo_label: String,
    pub path: String,
}

#[napi(object)]
pub struct DuplicateGroup {
    pub hash: String,
    pub normalized_len: u32,
    pub files: Vec<DuplicateFile>,
}

#[napi(object)]
pub struct DuplicateSpanOccurrence {
    pub repo_id: u32,
    pub repo_label: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[napi(object)]
pub struct DuplicateSpanGroup {
    pub hash: String,
    pub normalized_len: u32,
    pub preview: String,
    pub occurrences: Vec<DuplicateSpanOccurrence>,
}

#[napi(object)]
pub struct SimilarityPair {
    pub a: DuplicateSpanOccurrence,
    pub b: DuplicateSpanOccurrence,
    pub score: f64,
    pub distance: Option<u32>,
}

#[napi(object)]
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

#[napi(js_name = "findDuplicateFiles")]
pub fn find_duplicate_files(
    roots: Vec<String>,
    options: Option<ScanOptions>,
) -> Result<Vec<DuplicateGroup>> {
    if roots.is_empty() {
        return Err(Error::from_reason("roots must not be empty"));
    }

    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    let options = to_core_options(options)?;

    let groups = code_checker_core::find_duplicate_files(&roots, &options)
        .map_err(|e| Error::from_reason(format!("scan failed: {e}")))?;

    Ok(groups
        .into_iter()
        .map(|g| DuplicateGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len as u32,
            files: g
                .files
                .into_iter()
                .map(|f| DuplicateFile {
                    repo_id: f.repo_id as u32,
                    repo_label: f.repo_label,
                    path: f.path,
                })
                .collect(),
        })
        .collect())
}

#[napi(js_name = "findDuplicateCodeSpans")]
pub fn find_duplicate_code_spans(
    roots: Vec<String>,
    options: Option<ScanOptions>,
) -> Result<Vec<DuplicateSpanGroup>> {
    if roots.is_empty() {
        return Err(Error::from_reason("roots must not be empty"));
    }

    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    let options = to_core_options(options)?;

    let groups = code_checker_core::find_duplicate_code_spans(&roots, &options)
        .map_err(|e| Error::from_reason(format!("scan failed: {e}")))?;

    Ok(groups
        .into_iter()
        .map(|g| DuplicateSpanGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len as u32,
            preview: g.preview,
            occurrences: g
                .occurrences
                .into_iter()
                .map(|o| DuplicateSpanOccurrence {
                    repo_id: o.repo_id as u32,
                    repo_label: o.repo_label,
                    path: o.path,
                    start_line: o.start_line,
                    end_line: o.end_line,
                })
                .collect(),
        })
        .collect())
}

#[napi(js_name = "generateDuplicationReport")]
pub fn generate_duplication_report(
    roots: Vec<String>,
    options: Option<ScanOptions>,
) -> Result<DuplicationReport> {
    if roots.is_empty() {
        return Err(Error::from_reason("roots must not be empty"));
    }

    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    let options = to_core_options(options)?;

    let report = code_checker_core::generate_duplication_report(&roots, &options)
        .map_err(|e| Error::from_reason(format!("scan failed: {e}")))?;

    Ok(DuplicationReport {
        file_duplicates: report
            .file_duplicates
            .into_iter()
            .map(|g| DuplicateGroup {
                hash: format!("{:016x}", g.content_hash),
                normalized_len: g.normalized_len as u32,
                files: g
                    .files
                    .into_iter()
                    .map(|f| DuplicateFile {
                        repo_id: f.repo_id as u32,
                        repo_label: f.repo_label,
                        path: f.path,
                    })
                    .collect(),
            })
            .collect(),
        code_span_duplicates: map_span_groups(report.code_span_duplicates),
        line_span_duplicates: map_span_groups(report.line_span_duplicates),
        token_span_duplicates: map_span_groups(report.token_span_duplicates),
        block_duplicates: map_span_groups(report.block_duplicates),
        ast_subtree_duplicates: map_span_groups(report.ast_subtree_duplicates),
        similar_blocks_minhash: report
            .similar_blocks_minhash
            .into_iter()
            .map(|p| SimilarityPair {
                a: DuplicateSpanOccurrence {
                    repo_id: p.a.repo_id as u32,
                    repo_label: p.a.repo_label,
                    path: p.a.path,
                    start_line: p.a.start_line,
                    end_line: p.a.end_line,
                },
                b: DuplicateSpanOccurrence {
                    repo_id: p.b.repo_id as u32,
                    repo_label: p.b.repo_label,
                    path: p.b.path,
                    start_line: p.b.start_line,
                    end_line: p.b.end_line,
                },
                score: p.score,
                distance: p.distance,
            })
            .collect(),
        similar_blocks_simhash: report
            .similar_blocks_simhash
            .into_iter()
            .map(|p| SimilarityPair {
                a: DuplicateSpanOccurrence {
                    repo_id: p.a.repo_id as u32,
                    repo_label: p.a.repo_label,
                    path: p.a.path,
                    start_line: p.a.start_line,
                    end_line: p.a.end_line,
                },
                b: DuplicateSpanOccurrence {
                    repo_id: p.b.repo_id as u32,
                    repo_label: p.b.repo_label,
                    path: p.b.path,
                    start_line: p.b.start_line,
                    end_line: p.b.end_line,
                },
                score: p.score,
                distance: p.distance,
            })
            .collect(),
    })
}

fn map_span_groups(groups: Vec<code_checker_core::DuplicateSpanGroup>) -> Vec<DuplicateSpanGroup> {
    groups
        .into_iter()
        .map(|g| DuplicateSpanGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len as u32,
            preview: g.preview,
            occurrences: g
                .occurrences
                .into_iter()
                .map(|o| DuplicateSpanOccurrence {
                    repo_id: o.repo_id as u32,
                    repo_label: o.repo_label,
                    path: o.path,
                    start_line: o.start_line,
                    end_line: o.end_line,
                })
                .collect(),
        })
        .collect()
}

fn to_core_options(options: Option<ScanOptions>) -> Result<code_checker_core::ScanOptions> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let mut out = code_checker_core::ScanOptions::default();

    if let Some(options) = options {
        fn parse_u32_in_range(name: &str, value: f64, min: u32, max: u32) -> Result<u32> {
            if !value.is_finite() {
                return Err(Error::from_reason(format!(
                    "{name} must be a finite number"
                )));
            }
            if value.fract() != 0.0 {
                return Err(Error::from_reason(format!("{name} must be an integer")));
            }
            let min_f = min as f64;
            let max_f = max as f64;
            if !(min_f..=max_f).contains(&value) {
                return Err(Error::from_reason(format!("{name} must be {min}..{max}")));
            }
            Ok(value as u32)
        }

        if let Some(ignore_dirs) = options.ignore_dirs {
            out.ignore_dirs.extend(ignore_dirs);
        }
        if let Some(max_file_size) = options.max_file_size {
            if !max_file_size.is_finite() {
                return Err(Error::from_reason(
                    "maxFileSize must be a finite number".to_string(),
                ));
            }
            if max_file_size < 0.0 {
                return Err(Error::from_reason(
                    "maxFileSize must be a non-negative integer".to_string(),
                ));
            }
            if max_file_size.fract() != 0.0 {
                return Err(Error::from_reason(
                    "maxFileSize must be an integer".to_string(),
                ));
            }
            if max_file_size > (MAX_SAFE_INTEGER as f64) {
                return Err(Error::from_reason(format!(
                    "maxFileSize must be <= {MAX_SAFE_INTEGER} (Number.MAX_SAFE_INTEGER)"
                )));
            }
            out.max_file_size = Some(max_file_size as u64);
        }
        if let Some(min_match_len) = options.min_match_len {
            out.min_match_len =
                parse_u32_in_range("minMatchLen", min_match_len, 1, u32::MAX)? as usize;
        }
        if let Some(min_token_len) = options.min_token_len {
            out.min_token_len =
                parse_u32_in_range("minTokenLen", min_token_len, 1, u32::MAX)? as usize;
        }
        if let Some(similarity_threshold) = options.similarity_threshold {
            if !similarity_threshold.is_finite() {
                return Err(Error::from_reason(
                    "similarityThreshold must be a finite number".to_string(),
                ));
            }
            if !(0.0..=1.0).contains(&similarity_threshold) {
                return Err(Error::from_reason(
                    "similarityThreshold must be 0..1".to_string(),
                ));
            }
            out.similarity_threshold = similarity_threshold;
        }
        if let Some(simhash_max_distance) = options.simhash_max_distance {
            out.simhash_max_distance =
                parse_u32_in_range("simhashMaxDistance", simhash_max_distance, 0, 64)?;
        }
        if let Some(max_report_items) = options.max_report_items {
            out.max_report_items =
                parse_u32_in_range("maxReportItems", max_report_items, 0, u32::MAX)? as usize;
        }
        out.respect_gitignore = options.respect_gitignore.unwrap_or(out.respect_gitignore);
        out.cross_repo_only = options.cross_repo_only.unwrap_or(out.cross_repo_only);
        out.follow_symlinks = options.follow_symlinks.unwrap_or(out.follow_symlinks);
    }

    Ok(out)
}
