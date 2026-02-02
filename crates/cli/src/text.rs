use dup_code_check_core::ScanStats;

use crate::args::{Localization, tr};
use crate::json::{
    JsonDuplicateGroup, JsonDuplicateSpanGroup, JsonDuplicationReport, JsonSimilarityPair,
};

pub(crate) fn has_fatal_skips(stats: &ScanStats) -> bool {
    stats.skipped_permission_denied > 0
        || stats.skipped_walk_errors > 0
        || stats.skipped_budget_max_files > 0
        || stats.skipped_budget_max_total_bytes > 0
}

pub(crate) fn format_scan_stats(localization: Localization, stats: &ScanStats) -> String {
    let mut out = String::new();
    out.push_str(tr(localization, "== scan stats ==\n", "== 扫描统计 ==\n"));
    out.push_str(&format!(
        "candidates={} scanned={} bytes={}\n",
        stats.candidate_files, stats.scanned_files, stats.scanned_bytes
    ));

    let mut skips: Vec<(&str, u64)> = vec![
        ("not_found", stats.skipped_not_found),
        ("permission_denied", stats.skipped_permission_denied),
        ("too_large", stats.skipped_too_large),
        ("binary", stats.skipped_binary),
        ("outside_root", stats.skipped_outside_root),
        ("walk_errors", stats.skipped_walk_errors),
        ("budget_max_files", stats.skipped_budget_max_files),
        (
            "budget_max_total_bytes",
            stats.skipped_budget_max_total_bytes,
        ),
    ];
    skips.retain(|(_, v)| *v > 0);
    if !skips.is_empty() {
        out.push_str(tr(localization, "skipped:\n", "跳过:\n"));
        for (k, v) in skips {
            out.push_str(&format!("- {k}={v}\n"));
        }
    }
    out.push('\n');
    out
}

pub(crate) fn format_text(localization: Localization, groups: &[JsonDuplicateGroup]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        tr(localization, "duplicate groups", "重复文件组"),
        groups.len()
    ));

    for group in groups {
        out.push('\n');
        out.push_str(&format!(
            "hash={} normalized_len={} files={}\n",
            group.hash,
            group.normalized_len,
            group.files.len()
        ));
        for file in &group.files {
            out.push_str(&format!("- [{}] {}\n", file.repo_label, file.path));
        }
    }

    out.push('\n');
    out
}

pub(crate) fn format_text_code_spans(
    localization: Localization,
    groups: &[JsonDuplicateSpanGroup],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        tr(
            localization,
            "duplicate code span groups",
            "疑似重复代码片段组"
        ),
        groups.len()
    ));

    for group in groups {
        out.push('\n');
        out.push_str(&format!(
            "hash={} normalized_len={} occurrences={}\n",
            group.hash,
            group.normalized_len,
            group.occurrences.len()
        ));
        out.push_str(&format!("preview={}\n", group.preview));
        for occ in &group.occurrences {
            out.push_str(&format!(
                "- [{}] {}:{}-{}\n",
                occ.repo_label, occ.path, occ.start_line, occ.end_line
            ));
        }
    }

    out.push('\n');
    out
}

pub(crate) fn format_text_similar_pairs(
    localization: Localization,
    pairs: &[JsonSimilarityPair],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        tr(localization, "similar pairs", "相似对"),
        pairs.len()
    ));
    for pair in pairs {
        if let Some(distance) = pair.distance {
            out.push_str(&format!("score={} distance={distance}\n", pair.score));
        } else {
            out.push_str(&format!("score={}\n", pair.score));
        }
        out.push_str(&format!(
            "- A [{}] {}:{}-{}\n",
            pair.a.repo_label, pair.a.path, pair.a.start_line, pair.a.end_line
        ));
        out.push_str(&format!(
            "- B [{}] {}:{}-{}\n",
            pair.b.repo_label, pair.b.path, pair.b.start_line, pair.b.end_line
        ));
    }
    out.push('\n');
    out
}

pub(crate) fn format_text_report(
    localization: Localization,
    report: &JsonDuplicationReport,
) -> String {
    let mut out = String::new();

    out.push_str(tr(
        localization,
        "== file duplicates ==\n",
        "== 重复文件 ==\n",
    ));
    out.push_str(format_text(localization, &report.file_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== code span duplicates ==\n",
        "== 重复代码片段 ==\n",
    ));
    out.push_str(format_text_code_spans(localization, &report.code_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== line span duplicates ==\n",
        "== 行片段重复 ==\n",
    ));
    out.push_str(format_text_code_spans(localization, &report.line_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== token span duplicates ==\n",
        "== Token 片段重复 ==\n",
    ));
    out.push_str(format_text_code_spans(localization, &report.token_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== block duplicates ==\n",
        "== 块重复 ==\n",
    ));
    out.push_str(format_text_code_spans(localization, &report.block_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== AST subtree duplicates ==\n",
        "== AST 子树重复（近似） ==\n",
    ));
    out.push_str(format_text_code_spans(localization, &report.ast_subtree_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== similar blocks (minhash) ==\n",
        "== 相似块对（minhash） ==\n",
    ));
    out.push_str(
        format_text_similar_pairs(localization, &report.similar_blocks_minhash).trim_end(),
    );
    out.push_str("\n\n");

    out.push_str(tr(
        localization,
        "== similar blocks (simhash) ==\n",
        "== 相似块对（simhash） ==\n",
    ));
    out.push_str(
        format_text_similar_pairs(localization, &report.similar_blocks_simhash).trim_end(),
    );
    out.push_str("\n\n");

    out
}
