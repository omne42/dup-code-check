use std::env;
use std::io;
use std::path::{Component, Path, PathBuf};

use dup_code_check_core::{ScanOptions, ScanStats};
use serde::Serialize;

const HELP_TEXT_EN: &str = concat!(
    "dup-code-check (duplicate files / suspected duplicate code spans)\n",
    "\n",
    "Usage:\n",
    "  dup-code-check [options] [root ...]\n",
    "\n",
    "Options:\n",
    "  --localization <en|zh>  Set output language (default: en)\n",
    "  --report                Run all detectors and output a report\n",
    "  --code-spans            Find suspected duplicate code spans\n",
    "  --json                  Output JSON\n",
    "  --stats                 Include scan stats (JSON) or print to stderr\n",
    "  --strict                Exit non-zero if scan was incomplete\n",
    "  --cross-repo-only       Only report groups spanning >= 2 roots\n",
    "  --no-gitignore          Do not respect .gitignore rules\n",
    "  --min-match-len <n>     Code spans: minimum normalized length (default: 50)\n",
    "  --min-token-len <n>     Token-based: minimum token length (default: 50)\n",
    "  --similarity-threshold <f>  Similarity: 0..1 (default: 0.85)\n",
    "  --simhash-max-distance <n>  SimHash: max Hamming distance (default: 3)\n",
    "  --max-report-items <n>  Limit items per report section (default: 200)\n",
    "  --max-files <n>         Stop after scanning n files\n",
    "  --max-total-bytes <n>   Skip files that would exceed total scanned bytes\n",
    "  --max-file-size <n>     Skip files larger than n bytes (default: 10485760)\n",
    "  --ignore-dir <name>     Add an ignored directory name (repeatable)\n",
    "  --follow-symlinks       Follow symlinks (default: off)\n",
    "  -h, --help              Show help\n",
    "\n",
    "Examples:\n",
    "  dup-code-check .\n",
    "  dup-code-check --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --code-spans --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --report --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --ignore-dir vendor --ignore-dir .venv .\n",
    "\n"
);

const HELP_TEXT_ZH: &str = concat!(
    "dup-code-check（重复文件 / 疑似重复代码片段）\n",
    "\n",
    "用法:\n",
    "  dup-code-check [options] [root ...]\n",
    "\n",
    "选项:\n",
    "  --localization <en|zh>  输出语言（默认: en）\n",
    "  --report                运行全部检测器并输出报告\n",
    "  --code-spans            查找疑似重复代码片段\n",
    "  --json                  输出 JSON\n",
    "  --stats                 输出扫描统计（JSON 模式合并到输出；文本模式写 stderr）\n",
    "  --strict                扫描不完整时返回非 0 退出码\n",
    "  --cross-repo-only       仅输出跨 >= 2 个 root 的重复组\n",
    "  --no-gitignore          不尊重 .gitignore 规则\n",
    "  --min-match-len <n>     code spans：最小归一化长度（默认: 50）\n",
    "  --min-token-len <n>     token 检测：最小 token 长度（默认: 50）\n",
    "  --similarity-threshold <f>  相似度阈值：0..1（默认: 0.85）\n",
    "  --simhash-max-distance <n>  SimHash 最大汉明距离（默认: 3）\n",
    "  --max-report-items <n>  每个报告 section 的最大条目数（默认: 200）\n",
    "  --max-files <n>         最多扫描 n 个文件\n",
    "  --max-total-bytes <n>   跳过会导致累计扫描字节数超出预算的文件\n",
    "  --max-file-size <n>     跳过大于 n 字节的文件（默认: 10485760）\n",
    "  --ignore-dir <name>     忽略目录名（可重复）\n",
    "  --follow-symlinks       跟随符号链接（默认: 关闭）\n",
    "  -h, --help              显示帮助\n",
    "\n",
    "示例:\n",
    "  dup-code-check .\n",
    "  dup-code-check --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --code-spans --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --report --cross-repo-only /repoA /repoB\n",
    "  dup-code-check --ignore-dir vendor --ignore-dir .venv .\n",
    "\n"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Localization {
    En,
    Zh,
}

impl Localization {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en_us" => Some(Self::En),
            "zh" | "zh-cn" | "zh_cn" | "cn" => Some(Self::Zh),
            _ => None,
        }
    }
}

fn tr(localization: Localization, en: &'static str, zh: &'static str) -> &'static str {
    match localization {
        Localization::En => en,
        Localization::Zh => zh,
    }
}

fn print_help(localization: Localization) {
    print!(
        "{}",
        match localization {
            Localization::En => HELP_TEXT_EN,
            Localization::Zh => HELP_TEXT_ZH,
        }
    );
}

#[derive(Debug, Clone)]
struct ParsedArgs {
    localization: Localization,
    json: bool,
    stats: bool,
    strict: bool,
    report: bool,
    code_spans: bool,
    roots: Vec<PathBuf>,
    options: ScanOptions,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonScanStats {
    candidate_files: u64,
    scanned_files: u64,
    scanned_bytes: u64,
    skipped_not_found: u64,
    skipped_permission_denied: u64,
    skipped_too_large: u64,
    skipped_binary: u64,
    skipped_walk_errors: u64,
    skipped_budget_max_files: u64,
    skipped_budget_max_total_bytes: u64,
}

impl From<ScanStats> for JsonScanStats {
    fn from(stats: ScanStats) -> Self {
        Self {
            candidate_files: stats.candidate_files,
            scanned_files: stats.scanned_files,
            scanned_bytes: stats.scanned_bytes,
            skipped_not_found: stats.skipped_not_found,
            skipped_permission_denied: stats.skipped_permission_denied,
            skipped_too_large: stats.skipped_too_large,
            skipped_binary: stats.skipped_binary,
            skipped_walk_errors: stats.skipped_walk_errors,
            skipped_budget_max_files: stats.skipped_budget_max_files,
            skipped_budget_max_total_bytes: stats.skipped_budget_max_total_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDuplicateFile {
    repo_id: usize,
    repo_label: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDuplicateGroup {
    hash: String,
    normalized_len: usize,
    files: Vec<JsonDuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDuplicateSpanOccurrence {
    repo_id: usize,
    repo_label: String,
    path: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDuplicateSpanGroup {
    hash: String,
    normalized_len: usize,
    preview: String,
    occurrences: Vec<JsonDuplicateSpanOccurrence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSimilarityPair {
    a: JsonDuplicateSpanOccurrence,
    b: JsonDuplicateSpanOccurrence,
    score: f64,
    distance: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDuplicationReport {
    file_duplicates: Vec<JsonDuplicateGroup>,
    code_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    line_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    token_span_duplicates: Vec<JsonDuplicateSpanGroup>,
    block_duplicates: Vec<JsonDuplicateSpanGroup>,
    ast_subtree_duplicates: Vec<JsonDuplicateSpanGroup>,
    similar_blocks_minhash: Vec<JsonSimilarityPair>,
    similar_blocks_simhash: Vec<JsonSimilarityPair>,
}

fn map_duplicate_groups(
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

fn map_span_groups(
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

fn map_report(report: dup_code_check_core::DuplicationReport) -> JsonDuplicationReport {
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

fn parse_u64(localization: Localization, name: &str, raw: &str) -> Result<u64, String> {
    raw.parse::<u64>().map_err(|_| {
        format!(
            "{} {}",
            name,
            tr(localization, "must be an integer", "必须是整数")
        )
    })
}

fn parse_u64_non_negative_safe(
    localization: Localization,
    name: &str,
    raw: &str,
) -> Result<u64, String> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let value = parse_u64(localization, name, raw)?;
    if value > MAX_SAFE_INTEGER {
        return Err(format!(
            "{name} must be <= {MAX_SAFE_INTEGER} (Number.MAX_SAFE_INTEGER)"
        ));
    }
    Ok(value)
}

fn parse_u32_in_range(
    localization: Localization,
    name: &str,
    raw: &str,
    min: u32,
    max: u32,
) -> Result<u32, String> {
    let value = raw.parse::<u32>().map_err(|_| {
        format!(
            "{} {}",
            name,
            tr(localization, "must be an integer", "必须是整数")
        )
    })?;
    if !(min..=max).contains(&value) {
        return Err(
            format!("{} {}", name, tr(localization, "must be", "必须在"),)
                + &format!(" {min}..{max}"),
        );
    }
    Ok(value)
}

fn parse_f64(localization: Localization, name: &str, raw: &str) -> Result<f64, String> {
    raw.parse::<f64>().map_err(|_| {
        format!(
            "{} {}",
            name,
            tr(localization, "must be a number", "必须是数字")
        )
    })
}

fn detect_localization(argv: &[String]) -> Result<Localization, String> {
    let mut localization = Localization::En;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "--" {
            break;
        }

        if let Some(raw) = arg.strip_prefix("--localization=") {
            localization = Localization::parse(raw)
                .ok_or_else(|| "--localization must be one of: en, zh (or zh-CN)".to_string())?;
            i += 1;
            continue;
        }

        if arg == "--localization" {
            let raw = argv.get(i + 1).ok_or("--localization requires a value")?;
            localization = Localization::parse(raw)
                .ok_or_else(|| "--localization must be one of: en, zh (or zh-CN)".to_string())?;
            i += 2;
            continue;
        }

        i += 1;
    }

    Ok(localization)
}

fn parse_args(argv: &[String], localization: Localization) -> Result<ParsedArgs, String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut ignore_dirs: Vec<String> = Vec::new();
    let mut report = false;
    let mut code_spans = false;
    let mut json = false;
    let mut stats = false;
    let mut strict = false;
    let mut cross_repo_only = false;
    let mut respect_gitignore = true;
    let mut follow_symlinks = false;
    let mut max_file_size: Option<u64> = None;
    let mut max_files: Option<usize> = None;
    let mut max_total_bytes: Option<u64> = None;
    let mut min_match_len: Option<usize> = None;
    let mut min_token_len: Option<usize> = None;
    let mut similarity_threshold: Option<f64> = None;
    let mut simhash_max_distance: Option<u32> = None;
    let mut max_report_items: Option<usize> = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "--" {
            roots.extend(argv[(i + 1)..].iter().map(PathBuf::from));
            break;
        }
        if let Some(_) = arg.strip_prefix("--localization=") {
            i += 1;
            continue;
        }
        if arg == "--localization" {
            let _ = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--localization requires a value",
                    "--localization 需要一个值",
                )
                .to_string()
            })?;
            i += 2;
            continue;
        }
        if arg == "--report" {
            report = true;
            i += 1;
            continue;
        }
        if arg == "--code-spans" {
            code_spans = true;
            i += 1;
            continue;
        }
        if arg == "--json" {
            json = true;
            i += 1;
            continue;
        }
        if arg == "--stats" {
            stats = true;
            i += 1;
            continue;
        }
        if arg == "--strict" {
            strict = true;
            i += 1;
            continue;
        }
        if arg == "--cross-repo-only" {
            cross_repo_only = true;
            i += 1;
            continue;
        }
        if arg == "--no-gitignore" {
            respect_gitignore = false;
            i += 1;
            continue;
        }
        if arg == "--gitignore" {
            respect_gitignore = true;
            i += 1;
            continue;
        }
        if arg == "--follow-symlinks" {
            follow_symlinks = true;
            i += 1;
            continue;
        }
        if arg == "--max-files" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--max-files requires a value",
                    "--max-files 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u64_non_negative_safe(localization, "--max-files", raw)?;
            max_files = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--max-total-bytes" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--max-total-bytes requires a value",
                    "--max-total-bytes 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u64_non_negative_safe(localization, "--max-total-bytes", raw)?;
            max_total_bytes = Some(value);
            i += 2;
            continue;
        }
        if arg == "--max-file-size" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--max-file-size requires a value",
                    "--max-file-size 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u64_non_negative_safe(localization, "--max-file-size", raw)?;
            max_file_size = Some(value);
            i += 2;
            continue;
        }
        if arg == "--min-match-len" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--min-match-len requires a value",
                    "--min-match-len 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u32_in_range(localization, "--min-match-len", raw, 1, u32::MAX)?;
            min_match_len = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--min-token-len" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--min-token-len requires a value",
                    "--min-token-len 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u32_in_range(localization, "--min-token-len", raw, 1, u32::MAX)?;
            min_token_len = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--similarity-threshold" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--similarity-threshold requires a value",
                    "--similarity-threshold 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_f64(localization, "--similarity-threshold", raw)?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(tr(
                    localization,
                    "--similarity-threshold must be 0..1",
                    "--similarity-threshold 必须在 0..1 范围内",
                )
                .to_string());
            }
            similarity_threshold = Some(value);
            i += 2;
            continue;
        }
        if arg == "--simhash-max-distance" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--simhash-max-distance requires a value",
                    "--simhash-max-distance 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u32_in_range(localization, "--simhash-max-distance", raw, 0, 64)?;
            simhash_max_distance = Some(value);
            i += 2;
            continue;
        }
        if arg == "--max-report-items" {
            let raw = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--max-report-items requires a value",
                    "--max-report-items 需要一个值",
                )
                .to_string()
            })?;
            let value = parse_u32_in_range(localization, "--max-report-items", raw, 0, u32::MAX)?;
            max_report_items = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--ignore-dir" {
            let value = argv.get(i + 1).ok_or_else(|| {
                tr(
                    localization,
                    "--ignore-dir requires a value",
                    "--ignore-dir 需要一个值",
                )
                .to_string()
            })?;
            ignore_dirs.push(value.to_string());
            i += 2;
            continue;
        }
        if arg == "-h" || arg == "--help" {
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!(
                "{} {arg}",
                tr(localization, "Unknown option:", "未知参数:"),
            ));
        }
        roots.push(PathBuf::from(arg));
        i += 1;
    }

    let mut options = ScanOptions::default();
    options.respect_gitignore = respect_gitignore;
    options.cross_repo_only = cross_repo_only;
    options.follow_symlinks = follow_symlinks;
    if let Some(max_file_size) = max_file_size {
        options.max_file_size = Some(max_file_size);
    }
    if let Some(max_files) = max_files {
        options.max_files = Some(max_files);
    }
    if let Some(max_total_bytes) = max_total_bytes {
        options.max_total_bytes = Some(max_total_bytes);
    }
    if let Some(min_match_len) = min_match_len {
        options.min_match_len = min_match_len;
    }
    if let Some(min_token_len) = min_token_len {
        options.min_token_len = min_token_len;
    }
    if let Some(similarity_threshold) = similarity_threshold {
        options.similarity_threshold = similarity_threshold;
    }
    if let Some(simhash_max_distance) = simhash_max_distance {
        options.simhash_max_distance = simhash_max_distance;
    }
    if let Some(max_report_items) = max_report_items {
        options.max_report_items = max_report_items;
    }
    options.ignore_dirs.extend(ignore_dirs);

    let roots = if roots.is_empty() {
        vec![env::current_dir().map_err(|e| {
            format!(
                "{} {e}",
                tr(localization, "failed to get cwd:", "无法获取当前目录:"),
            )
        })?]
    } else {
        roots
    };

    Ok(ParsedArgs {
        localization,
        json,
        stats,
        strict,
        report,
        code_spans,
        roots,
        options,
    })
}

fn has_fatal_skips(stats: &ScanStats) -> bool {
    stats.skipped_permission_denied > 0
        || stats.skipped_walk_errors > 0
        || stats.skipped_budget_max_files > 0
        || stats.skipped_budget_max_total_bytes > 0
}

fn format_scan_stats(localization: Localization, stats: &ScanStats) -> String {
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

fn format_text(localization: Localization, groups: &[JsonDuplicateGroup]) -> String {
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

fn format_text_code_spans(localization: Localization, groups: &[JsonDuplicateSpanGroup]) -> String {
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

fn format_text_similar_pairs(localization: Localization, pairs: &[JsonSimilarityPair]) -> String {
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

fn format_text_report(localization: Localization, report: &JsonDuplicationReport) -> String {
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

fn resolve_path(p: &Path) -> io::Result<PathBuf> {
    let base = if p.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir()?
    };
    Ok(normalize_path(&base.join(p)))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn write_json<T: Serialize>(value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("json encode: {e}")))?;
    println!("{json}");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let localization = match detect_localization(&args) {
        Ok(localization) => localization,
        Err(message) => {
            eprintln!("Error: {message}\n");
            print_help(Localization::En);
            std::process::exit(2);
        }
    };

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help(localization);
        return;
    }

    let parsed = match parse_args(&args, localization) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{}: {message}\n", tr(localization, "Error", "错误"),);
            print_help(localization);
            std::process::exit(2);
        }
    };

    let roots: Vec<PathBuf> = match parsed
        .roots
        .iter()
        .map(|p| resolve_path(p))
        .collect::<io::Result<Vec<_>>>()
    {
        Ok(v) => v,
        Err(err) => {
            eprintln!("{}: {err}", tr(localization, "Error", "错误"));
            std::process::exit(1);
        }
    };

    let need_stats = parsed.stats || parsed.strict;
    match run(&parsed, &roots, need_stats) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => {
            eprintln!("{}: {err}", tr(localization, "Error", "错误"));
            std::process::exit(1);
        }
    }
}

fn run(parsed: &ParsedArgs, roots: &[PathBuf], need_stats: bool) -> io::Result<i32> {
    if parsed.report {
        let (report, scan_stats) = if need_stats {
            let outcome = dup_code_check_core::generate_duplication_report_with_stats(
                roots,
                &parsed.options,
            )?;
            (map_report(outcome.result), Some(outcome.stats))
        } else {
            let report = dup_code_check_core::generate_duplication_report(roots, &parsed.options)?;
            (map_report(report), None)
        };

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "report": report,
                    "scanStats": scan_stats.clone().map(JsonScanStats::from),
                }))?;
            } else {
                write_json(&report)?;
            }
        } else {
            print!("{}", format_text_report(parsed.localization, &report));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(parsed.localization, &stats));
                }
                return Ok(1);
            }
        }

        return Ok(0);
    }

    if parsed.code_spans {
        let (groups, scan_stats) = if need_stats {
            let outcome =
                dup_code_check_core::find_duplicate_code_spans_with_stats(roots, &parsed.options)?;
            (map_span_groups(outcome.result), Some(outcome.stats))
        } else {
            let groups = dup_code_check_core::find_duplicate_code_spans(roots, &parsed.options)?;
            (map_span_groups(groups), None)
        };

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "groups": groups,
                    "scanStats": scan_stats.clone().map(JsonScanStats::from),
                }))?;
            } else {
                write_json(&groups)?;
            }
        } else {
            print!("{}", format_text_code_spans(parsed.localization, &groups));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(parsed.localization, &stats));
                }
                return Ok(1);
            }
        }

        return Ok(0);
    }

    let (groups, scan_stats) = if need_stats {
        let outcome = dup_code_check_core::find_duplicate_files_with_stats(roots, &parsed.options)?;
        (map_duplicate_groups(outcome.result), Some(outcome.stats))
    } else {
        let groups = dup_code_check_core::find_duplicate_files(roots, &parsed.options)?;
        (map_duplicate_groups(groups), None)
    };

    if parsed.json {
        if parsed.stats {
            write_json(&serde_json::json!({
                "groups": groups,
                "scanStats": scan_stats.clone().map(JsonScanStats::from),
            }))?;
        } else {
            write_json(&groups)?;
        }
    } else {
        print!("{}", format_text(parsed.localization, &groups));
    }

    if let Some(stats) = scan_stats {
        if parsed.stats && !parsed.json {
            eprint!("{}", format_scan_stats(parsed.localization, &stats));
        }
        if parsed.strict && has_fatal_skips(&stats) {
            if !parsed.stats {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            return Ok(1);
        }
    }

    Ok(0)
}
