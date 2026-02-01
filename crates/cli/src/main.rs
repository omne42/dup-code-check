use std::env;
use std::io;
use std::path::{Component, Path, PathBuf};

use dup_code_check_core::{ScanOptions, ScanStats};
use serde::Serialize;

const HELP_TEXT: &str = concat!(
    "dup-code-check (duplicate files / suspected duplicate code spans)\n",
    "\n",
    "Usage:\n",
    "  dup-code-check [options] [root ...]\n",
    "\n",
    "Options:\n",
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

#[derive(Debug, Clone)]
struct ParsedArgs {
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

fn print_help() {
    print!("{HELP_TEXT}");
}

fn parse_u64(name: &str, raw: &str) -> Result<u64, String> {
    raw.parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))
}

fn parse_u64_non_negative_safe(name: &str, raw: &str) -> Result<u64, String> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let value = parse_u64(name, raw)?;
    if value > MAX_SAFE_INTEGER {
        return Err(format!(
            "{name} must be <= {MAX_SAFE_INTEGER} (Number.MAX_SAFE_INTEGER)"
        ));
    }
    Ok(value)
}

fn parse_u32_in_range(name: &str, raw: &str, min: u32, max: u32) -> Result<u32, String> {
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{name} must be {min}..{max}"));
    }
    Ok(value)
}

fn parse_f64(name: &str, raw: &str) -> Result<f64, String> {
    raw.parse::<f64>()
        .map_err(|_| format!("{name} must be a number"))
}

fn parse_args(argv: &[String]) -> Result<Option<ParsedArgs>, String> {
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
            let raw = argv.get(i + 1).ok_or("--max-files requires a value")?;
            let value = parse_u64_non_negative_safe("--max-files", raw)?;
            max_files = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--max-total-bytes" {
            let raw = argv
                .get(i + 1)
                .ok_or("--max-total-bytes requires a value")?;
            let value = parse_u64_non_negative_safe("--max-total-bytes", raw)?;
            max_total_bytes = Some(value);
            i += 2;
            continue;
        }
        if arg == "--max-file-size" {
            let raw = argv.get(i + 1).ok_or("--max-file-size requires a value")?;
            let value = parse_u64_non_negative_safe("--max-file-size", raw)?;
            max_file_size = Some(value);
            i += 2;
            continue;
        }
        if arg == "--min-match-len" {
            let raw = argv.get(i + 1).ok_or("--min-match-len requires a value")?;
            let value = parse_u32_in_range("--min-match-len", raw, 1, u32::MAX)?;
            min_match_len = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--min-token-len" {
            let raw = argv.get(i + 1).ok_or("--min-token-len requires a value")?;
            let value = parse_u32_in_range("--min-token-len", raw, 1, u32::MAX)?;
            min_token_len = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--similarity-threshold" {
            let raw = argv
                .get(i + 1)
                .ok_or("--similarity-threshold requires a value")?;
            let value = parse_f64("--similarity-threshold", raw)?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err("--similarity-threshold must be 0..1".to_string());
            }
            similarity_threshold = Some(value);
            i += 2;
            continue;
        }
        if arg == "--simhash-max-distance" {
            let raw = argv
                .get(i + 1)
                .ok_or("--simhash-max-distance requires a value")?;
            let value = parse_u32_in_range("--simhash-max-distance", raw, 0, 64)?;
            simhash_max_distance = Some(value);
            i += 2;
            continue;
        }
        if arg == "--max-report-items" {
            let raw = argv
                .get(i + 1)
                .ok_or("--max-report-items requires a value")?;
            let value = parse_u32_in_range("--max-report-items", raw, 0, u32::MAX)?;
            max_report_items = Some(value as usize);
            i += 2;
            continue;
        }
        if arg == "--ignore-dir" {
            let value = argv.get(i + 1).ok_or("--ignore-dir requires a value")?;
            ignore_dirs.push(value.to_string());
            i += 2;
            continue;
        }
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        }
        if arg.starts_with('-') {
            return Err(format!("Unknown option: {arg}"));
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
        vec![env::current_dir().map_err(|e| format!("failed to get cwd: {e}"))?]
    } else {
        roots
    };

    Ok(Some(ParsedArgs {
        json,
        stats,
        strict,
        report,
        code_spans,
        roots,
        options,
    }))
}

fn has_fatal_skips(stats: &ScanStats) -> bool {
    stats.skipped_permission_denied > 0
        || stats.skipped_walk_errors > 0
        || stats.skipped_budget_max_files > 0
        || stats.skipped_budget_max_total_bytes > 0
}

fn format_scan_stats(stats: &ScanStats) -> String {
    let mut out = String::new();
    out.push_str("== scan stats ==\n");
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
        out.push_str("skipped:\n");
        for (k, v) in skips {
            out.push_str(&format!("- {k}={v}\n"));
        }
    }
    out.push('\n');
    out
}

fn format_text(groups: &[JsonDuplicateGroup]) -> String {
    let mut out = String::new();
    out.push_str(&format!("duplicate groups: {}\n", groups.len()));

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

fn format_text_code_spans(groups: &[JsonDuplicateSpanGroup]) -> String {
    let mut out = String::new();
    out.push_str(&format!("duplicate code span groups: {}\n", groups.len()));

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

fn format_text_similar_pairs(pairs: &[JsonSimilarityPair]) -> String {
    let mut out = String::new();
    out.push_str(&format!("similar pairs: {}\n", pairs.len()));
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

fn format_text_report(report: &JsonDuplicationReport) -> String {
    let mut out = String::new();

    out.push_str("== file duplicates ==\n");
    out.push_str(format_text(&report.file_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== code span duplicates ==\n");
    out.push_str(format_text_code_spans(&report.code_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== line span duplicates ==\n");
    out.push_str(format_text_code_spans(&report.line_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== token span duplicates ==\n");
    out.push_str(format_text_code_spans(&report.token_span_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== block duplicates ==\n");
    out.push_str(format_text_code_spans(&report.block_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== AST subtree duplicates ==\n");
    out.push_str(format_text_code_spans(&report.ast_subtree_duplicates).trim_end());
    out.push_str("\n\n");

    out.push_str("== similar blocks (minhash) ==\n");
    out.push_str(format_text_similar_pairs(&report.similar_blocks_minhash).trim_end());
    out.push_str("\n\n");

    out.push_str("== similar blocks (simhash) ==\n");
    out.push_str(format_text_similar_pairs(&report.similar_blocks_simhash).trim_end());
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
    let parsed = match parse_args(&args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            print_help();
            return;
        }
        Err(message) => {
            eprintln!("Error: {message}\n");
            print_help();
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
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };

    let need_stats = parsed.stats || parsed.strict;
    match run(&parsed, &roots, need_stats) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => {
            eprintln!("Error: {err}");
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
            print!("{}", format_text_report(&report));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(&stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(&stats));
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
            print!("{}", format_text_code_spans(&groups));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(&stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(&stats));
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
        print!("{}", format_text(&groups));
    }

    if let Some(stats) = scan_stats {
        if parsed.stats && !parsed.json {
            eprint!("{}", format_scan_stats(&stats));
        }
        if parsed.strict && has_fatal_skips(&stats) {
            if !parsed.stats {
                eprint!("{}", format_scan_stats(&stats));
            }
            return Ok(1);
        }
    }

    Ok(0)
}
