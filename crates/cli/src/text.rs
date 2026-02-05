use dup_code_check_core::ScanStats;

use crate::args::{Localization, tr};
use crate::json::{
    JsonDuplicateGroup, JsonDuplicateSpanGroup, JsonDuplicationReport, JsonSimilarityPair,
};

pub(crate) fn has_fatal_skips(stats: &ScanStats) -> bool {
    stats.skipped_permission_denied > 0
        || stats.skipped_outside_root > 0
        || stats.skipped_relativize_failed > 0
        || stats.skipped_walk_errors > 0
        || stats.skipped_bucket_truncated > 0
        || stats.skipped_budget_max_files > 0
        || stats.skipped_budget_max_total_bytes > 0
        || stats.skipped_budget_max_normalized_chars > 0
        || stats.skipped_budget_max_tokens > 0
}

pub(crate) fn format_fatal_skip_warning(
    localization: Localization,
    stats: &ScanStats,
    has_stats: bool,
) -> String {
    if !has_fatal_skips(stats) {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(tr(
        localization,
        "Warning: scan was incomplete (fatal skips):\n",
        "警告：扫描不完整（致命跳过）：\n",
    ));

    let push_item = |out: &mut String,
                     key_camel: &str,
                     key_snake: &str,
                     value: u64,
                     hint_en: &'static str,
                     hint_zh: &'static str| {
        if value == 0 {
            return;
        }
        out.push_str(&format!(
            "- {key_camel}={value} ({key_snake}): {}\n",
            tr(localization, hint_en, hint_zh)
        ));
    };

    push_item(
        &mut out,
        "skippedPermissionDenied",
        "permission_denied",
        stats.skipped_permission_denied,
        "some files could not be read; check permissions.",
        "部分文件无法读取；请检查权限。",
    );
    push_item(
        &mut out,
        "skippedOutsideRoot",
        "outside_root",
        stats.skipped_outside_root,
        "some paths were outside roots (symlink targets or unsafe paths); consider disabling --follow-symlinks or checking for unexpected paths.",
        "部分路径位于 root 之外（可能是符号链接目标或不安全路径）；可考虑关闭 --follow-symlinks 或检查是否存在异常路径。",
    );
    push_item(
        &mut out,
        "skippedRelativizeFailed",
        "relativize_failed",
        stats.skipped_relativize_failed,
        "some paths could not be made relative to the provided roots (unexpected); re-run with --stats and report a bug if this persists.",
        "部分路径无法相对化到提供的 root（不符合预期）；请使用 --stats 重新运行，若持续出现请提交 issue。",
    );
    push_item(
        &mut out,
        "skippedWalkErrors",
        "walk_errors",
        stats.skipped_walk_errors,
        "filesystem traversal/read errors occurred; check the underlying errors.",
        "文件系统遍历/读取出错；请检查底层错误。",
    );
    push_item(
        &mut out,
        "skippedBucketTruncated",
        "bucket_truncated",
        stats.skipped_bucket_truncated,
        "high-frequency fingerprints were truncated; consider increasing --min-match-len/--min-token-len or using --ignore-dir to skip generated/vendor dirs.",
        "高频 fingerprint bucket 被截断；可考虑提高 --min-match-len/--min-token-len，或用 --ignore-dir 跳过生成物/依赖目录。",
    );
    push_item(
        &mut out,
        "skippedBudgetMaxFiles",
        "budget_max_files",
        stats.skipped_budget_max_files,
        "hit --max-files; increase the budget or remove the limit.",
        "触发 --max-files 预算；请提高预算或移除限制。",
    );
    push_item(
        &mut out,
        "skippedBudgetMaxTotalBytes",
        "budget_max_total_bytes",
        stats.skipped_budget_max_total_bytes,
        "hit --max-total-bytes; increase the budget or remove the limit.",
        "触发 --max-total-bytes 预算；请提高预算或移除限制。",
    );
    push_item(
        &mut out,
        "skippedBudgetMaxNormalizedChars",
        "budget_max_normalized_chars",
        stats.skipped_budget_max_normalized_chars,
        "hit --max-normalized-chars; increase the budget or remove the limit.",
        "触发 --max-normalized-chars 预算；请提高预算或移除限制。",
    );
    push_item(
        &mut out,
        "skippedBudgetMaxTokens",
        "budget_max_tokens",
        stats.skipped_budget_max_tokens,
        "hit --max-tokens; increase the budget or remove the limit.",
        "触发 --max-tokens 预算；请提高预算或移除限制。",
    );

    if !has_stats {
        out.push_str(tr(
            localization,
            "Re-run with --stats for full details.\n",
            "请使用 --stats 重新运行以查看完整统计。\n",
        ));
    }

    out
}

pub(crate) fn format_scan_stats(localization: Localization, stats: &ScanStats) -> String {
    let mut out = String::new();
    out.push_str(tr(localization, "== scan stats ==\n", "== 扫描统计 ==\n"));
    out.push_str(&format!(
        "candidates={} scanned={} bytes={}\n",
        stats.candidate_files, stats.scanned_files, stats.scanned_bytes
    ));
    if stats.git_fast_path_fallbacks > 0 {
        out.push_str(&format!(
            "git_fast_path_fallbacks={}\n",
            stats.git_fast_path_fallbacks
        ));
    }

    let mut skips: Vec<(&str, u64)> = vec![
        ("not_found", stats.skipped_not_found),
        ("permission_denied", stats.skipped_permission_denied),
        ("too_large", stats.skipped_too_large),
        ("binary", stats.skipped_binary),
        ("outside_root", stats.skipped_outside_root),
        ("relativize_failed", stats.skipped_relativize_failed),
        ("walk_errors", stats.skipped_walk_errors),
        ("bucket_truncated", stats.skipped_bucket_truncated),
        ("budget_max_files", stats.skipped_budget_max_files),
        (
            "budget_max_total_bytes",
            stats.skipped_budget_max_total_bytes,
        ),
        (
            "budget_max_normalized_chars",
            stats.skipped_budget_max_normalized_chars,
        ),
        ("budget_max_tokens", stats.skipped_budget_max_tokens),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_truncated_is_fatal_skip() {
        let stats = ScanStats {
            skipped_bucket_truncated: 1,
            ..ScanStats::default()
        };
        assert!(has_fatal_skips(&stats));
    }

    #[test]
    fn relativize_failed_is_fatal_skip() {
        let stats = ScanStats {
            skipped_relativize_failed: 1,
            ..ScanStats::default()
        };
        assert!(has_fatal_skips(&stats));
    }

    #[test]
    fn fatal_skip_warning_is_actionable_en() {
        let stats = ScanStats {
            skipped_bucket_truncated: 3,
            ..ScanStats::default()
        };
        let msg = format_fatal_skip_warning(Localization::En, &stats, false);
        assert!(msg.contains("skippedBucketTruncated=3"));
        assert!(msg.contains("(bucket_truncated)"));
        assert!(msg.contains("--ignore-dir"));
        assert!(msg.contains("--stats"));
    }

    #[test]
    fn fatal_skip_warning_is_actionable_zh() {
        let stats = ScanStats {
            skipped_budget_max_files: 1,
            ..ScanStats::default()
        };
        let msg = format_fatal_skip_warning(Localization::Zh, &stats, false);
        assert!(msg.contains("skippedBudgetMaxFiles=1"));
        assert!(msg.contains("(budget_max_files)"));
        assert!(msg.contains("请使用 --stats"));
    }
}
