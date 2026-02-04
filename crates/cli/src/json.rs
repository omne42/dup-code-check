use std::io;

use dup_code_check_core::ScanStats;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonScanStats {
    pub(crate) candidate_files: u64,
    pub(crate) scanned_files: u64,
    pub(crate) scanned_bytes: u64,
    pub(crate) git_fast_path_fallbacks: u64,
    pub(crate) skipped_not_found: u64,
    pub(crate) skipped_permission_denied: u64,
    pub(crate) skipped_too_large: u64,
    pub(crate) skipped_binary: u64,
    pub(crate) skipped_outside_root: u64,
    pub(crate) skipped_walk_errors: u64,
    pub(crate) skipped_budget_max_files: u64,
    pub(crate) skipped_budget_max_total_bytes: u64,
    pub(crate) skipped_bucket_truncated: u64,
}

impl From<ScanStats> for JsonScanStats {
    fn from(stats: ScanStats) -> Self {
        Self {
            candidate_files: stats.candidate_files,
            scanned_files: stats.scanned_files,
            scanned_bytes: stats.scanned_bytes,
            git_fast_path_fallbacks: stats.git_fast_path_fallbacks,
            skipped_not_found: stats.skipped_not_found,
            skipped_permission_denied: stats.skipped_permission_denied,
            skipped_too_large: stats.skipped_too_large,
            skipped_binary: stats.skipped_binary,
            skipped_outside_root: stats.skipped_outside_root,
            skipped_walk_errors: stats.skipped_walk_errors,
            skipped_budget_max_files: stats.skipped_budget_max_files,
            skipped_budget_max_total_bytes: stats.skipped_budget_max_total_bytes,
            skipped_bucket_truncated: stats.skipped_bucket_truncated,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonDuplicateFile {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonDuplicateGroup {
    pub(crate) hash: String,
    pub(crate) normalized_len: usize,
    pub(crate) files: Vec<JsonDuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonDuplicateSpanOccurrence {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: String,
    pub(crate) path: String,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonDuplicateSpanGroup {
    pub(crate) hash: String,
    pub(crate) normalized_len: usize,
    pub(crate) preview: String,
    pub(crate) occurrences: Vec<JsonDuplicateSpanOccurrence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonSimilarityPair {
    pub(crate) a: JsonDuplicateSpanOccurrence,
    pub(crate) b: JsonDuplicateSpanOccurrence,
    pub(crate) score: f64,
    pub(crate) distance: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonDuplicationReport {
    pub(crate) file_duplicates: Vec<JsonDuplicateGroup>,
    pub(crate) code_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    pub(crate) line_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    pub(crate) token_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    pub(crate) block_duplicates: Vec<JsonDuplicateSpanGroup>,
    pub(crate) ast_subtree_duplicates: Vec<JsonDuplicateSpanGroup>,
    pub(crate) similar_blocks_minhash: Vec<JsonSimilarityPair>,
    pub(crate) similar_blocks_simhash: Vec<JsonSimilarityPair>,
}

pub(crate) fn map_duplicate_groups(
    groups: Vec<dup_code_check_core::DuplicateGroup>,
) -> Vec<JsonDuplicateGroup> {
    groups
        .into_iter()
        .map(|g| JsonDuplicateGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len,
            files: g
                .files
                .into_iter()
                .map(|f| JsonDuplicateFile {
                    repo_id: f.repo_id,
                    repo_label: f.repo_label,
                    path: f.path,
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn map_span_groups(
    groups: Vec<dup_code_check_core::DuplicateSpanGroup>,
) -> Vec<JsonDuplicateSpanGroup> {
    groups
        .into_iter()
        .map(|g| JsonDuplicateSpanGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len,
            preview: g.preview,
            occurrences: g
                .occurrences
                .into_iter()
                .map(|o| JsonDuplicateSpanOccurrence {
                    repo_id: o.repo_id,
                    repo_label: o.repo_label,
                    path: o.path,
                    start_line: o.start_line,
                    end_line: o.end_line,
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn map_report(report: dup_code_check_core::DuplicationReport) -> JsonDuplicationReport {
    JsonDuplicationReport {
        file_duplicates: map_duplicate_groups(report.file_duplicates),
        code_span_duplicates: map_span_groups(report.code_span_duplicates),
        line_span_duplicates: map_span_groups(report.line_span_duplicates),
        token_span_duplicates: map_span_groups(report.token_span_duplicates),
        block_duplicates: map_span_groups(report.block_duplicates),
        ast_subtree_duplicates: map_span_groups(report.ast_subtree_duplicates),
        similar_blocks_minhash: report
            .similar_blocks_minhash
            .into_iter()
            .map(|p| JsonSimilarityPair {
                a: JsonDuplicateSpanOccurrence {
                    repo_id: p.a.repo_id,
                    repo_label: p.a.repo_label,
                    path: p.a.path,
                    start_line: p.a.start_line,
                    end_line: p.a.end_line,
                },
                b: JsonDuplicateSpanOccurrence {
                    repo_id: p.b.repo_id,
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
            .map(|p| JsonSimilarityPair {
                a: JsonDuplicateSpanOccurrence {
                    repo_id: p.a.repo_id,
                    repo_label: p.a.repo_label,
                    path: p.a.path,
                    start_line: p.a.start_line,
                    end_line: p.a.end_line,
                },
                b: JsonDuplicateSpanOccurrence {
                    repo_id: p.b.repo_id,
                    repo_label: p.b.repo_label,
                    path: p.b.path,
                    start_line: p.b.start_line,
                    end_line: p.b.end_line,
                },
                score: p.score,
                distance: p.distance,
            })
            .collect(),
    }
}

pub(crate) fn write_json<T: Serialize>(value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| io::Error::other(format!("json encode: {e}")))?;
    println!("{json}");
    Ok(())
}
