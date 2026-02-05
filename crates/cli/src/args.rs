use std::env;
use std::path::PathBuf;

use dup_code_check_core::ScanOptions;

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
    "  --strict                Exit non-zero on fatal skips (perm/traversal/budget/bucket/relativize)\n",
    "  --cross-repo-only       Only report groups spanning >= 2 roots\n",
    "  --no-gitignore          Do not respect .gitignore rules\n",
    "  --gitignore             Respect .gitignore rules (default: on)\n",
    "  --min-match-len <n>     Code spans: minimum normalized length (default: 50)\n",
    "  --min-token-len <n>     Token-based: minimum token length (default: 50)\n",
    "  --similarity-threshold <f>  Similarity: 0..1 (default: 0.85)\n",
    "  --simhash-max-distance <n>  SimHash: max Hamming distance (default: 3)\n",
    "  --max-report-items <n>  Limit items per report section (default: 200)\n",
    "  --max-files <n>         Stop after scanning n files\n",
    "  --max-total-bytes <n>   Skip files that would exceed total scanned bytes\n",
    "  --max-file-size <n>     Skip files larger than n bytes (default: 10485760)\n",
    "  --ignore-dir <name>     Add an ignored directory name (repeatable)\n",
    "  --follow-symlinks       Follow symlinks (within each root; default: off)\n",
    "  -V, --version           Show version\n",
    "  -h, --help              Show help\n",
    "\n",
    "Notes:\n",
    "  - --cross-repo-only requires 2+ roots (roots are the CLI paths)\n",
    "  - In text mode, --stats prints to stderr\n",
    "  - In --report mode, --max-total-bytes defaults to 256 MiB (268435456 bytes); override with --max-total-bytes\n",
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
    "  --strict                若出现“致命跳过”（权限/遍历错误/预算中断/bucket 截断/无法相对化路径）则退出码非 0\n",
    "  --cross-repo-only       仅输出跨 >= 2 个 root 的重复组\n",
    "  --no-gitignore          不尊重 .gitignore 规则\n",
    "  --gitignore             启用 .gitignore 过滤（默认：开启）\n",
    "  --min-match-len <n>     code spans：最小归一化长度（默认: 50）\n",
    "  --min-token-len <n>     token 检测：最小 token 长度（默认: 50）\n",
    "  --similarity-threshold <f>  相似度阈值：0..1（默认: 0.85）\n",
    "  --simhash-max-distance <n>  SimHash 最大汉明距离（默认: 3）\n",
    "  --max-report-items <n>  每个报告 section 的最大条目数（默认: 200）\n",
    "  --max-files <n>         最多扫描 n 个文件\n",
    "  --max-total-bytes <n>   跳过会导致累计扫描字节数超出预算的文件\n",
    "  --max-file-size <n>     跳过大于 n 字节的文件（默认: 10485760）\n",
    "  --ignore-dir <name>     忽略目录名（可重复）\n",
    "  --follow-symlinks       跟随符号链接（仅限 root 内；默认: 关闭）\n",
    "  -V, --version           显示版本\n",
    "  -h, --help              显示帮助\n",
    "\n",
    "说明:\n",
    "  - --cross-repo-only 需要 2+ 个 root（root 即命令行路径）\n",
    "  - 文本模式下 --stats 输出到 stderr\n",
    "  - 在 --report 模式下，--max-total-bytes 默认 256 MiB（268435456 bytes），可用 --max-total-bytes 覆盖\n",
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
pub(crate) enum Localization {
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

pub(crate) fn tr(localization: Localization, en: &'static str, zh: &'static str) -> &'static str {
    match localization {
        Localization::En => en,
        Localization::Zh => zh,
    }
}

pub(crate) fn print_help(localization: Localization) {
    print!(
        "{}",
        match localization {
            Localization::En => HELP_TEXT_EN,
            Localization::Zh => HELP_TEXT_ZH,
        }
    );
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedArgs {
    pub(crate) localization: Localization,
    pub(crate) json: bool,
    pub(crate) stats: bool,
    pub(crate) strict: bool,
    pub(crate) report: bool,
    pub(crate) code_spans: bool,
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) options: ScanOptions,
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
        let note = tr(
            localization,
            " (Number.MAX_SAFE_INTEGER)",
            "（Number.MAX_SAFE_INTEGER）",
        );
        return Err(format!(
            "{name} {} {MAX_SAFE_INTEGER}{note}",
            tr(localization, "must be <=", "必须 <="),
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

pub(crate) fn detect_localization(argv: &[String]) -> Result<Localization, String> {
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

pub(crate) fn parse_args(
    argv: &[String],
    localization: Localization,
) -> Result<ParsedArgs, String> {
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
        if arg.strip_prefix("--localization=").is_some() {
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
            let value = usize::try_from(value).map_err(|_| {
                format!(
                    "--max-files {} {max}",
                    tr(localization, "must be <=", "必须 <= "),
                    max = usize::MAX
                )
            })?;
            max_files = Some(value);
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
        if arg == "-V" || arg == "--version" {
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

    if report && code_spans {
        return Err(tr(
            localization,
            "--report conflicts with --code-spans",
            "--report 与 --code-spans 不能同时使用",
        )
        .to_string());
    }

    let mut options = ScanOptions {
        respect_gitignore,
        cross_repo_only,
        follow_symlinks,
        ..ScanOptions::default()
    };
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

    if cross_repo_only && roots.len() < 2 {
        return Err(tr(
            localization,
            "--cross-repo-only requires at least 2 roots",
            "--cross-repo-only 需要至少 2 个 root",
        )
        .to_string());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn report_and_code_spans_are_mutually_exclusive_en() {
        let err =
            parse_args(&argv(&["--report", "--code-spans", "."]), Localization::En).unwrap_err();
        assert!(err.contains("conflicts"));
    }

    #[test]
    fn report_and_code_spans_are_mutually_exclusive_zh() {
        let err =
            parse_args(&argv(&["--report", "--code-spans", "."]), Localization::Zh).unwrap_err();
        assert!(err.contains("不能同时使用"));
    }

    #[test]
    fn max_safe_integer_error_is_localized_en() {
        let err =
            parse_u64_non_negative_safe(Localization::En, "--max-total-bytes", "9007199254740992")
                .unwrap_err();
        assert!(err.contains("must be <="));
        assert!(err.contains("Number.MAX_SAFE_INTEGER"));
    }

    #[test]
    fn max_safe_integer_error_is_localized_zh() {
        let err =
            parse_u64_non_negative_safe(Localization::Zh, "--max-total-bytes", "9007199254740992")
                .unwrap_err();
        assert!(err.contains("必须"));
        assert!(err.contains("Number.MAX_SAFE_INTEGER"));
    }
}
