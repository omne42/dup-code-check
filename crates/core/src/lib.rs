use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ignore::WalkBuilder;

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
    pub skipped_walk_errors: u64,
    pub skipped_budget_max_files: u64,
    pub skipped_budget_max_total_bytes: u64,
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

fn validate_roots(roots: &[PathBuf]) -> io::Result<()> {
    for root in roots {
        let meta = fs::metadata(root)
            .map_err(|err| io::Error::new(err.kind(), format!("root {}: {err}", root.display())))?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("root {} is not a directory", root.display()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Repo {
    id: usize,
    root: PathBuf,
    label: String,
}

#[derive(Debug, Clone)]
struct RepoFile {
    repo_id: usize,
    repo_label: String,
    root: PathBuf,
    abs_path: PathBuf,
}

#[derive(Debug)]
struct NormalizedFile {
    repo_id: usize,
    repo_label: String,
    rel_path: String,
    normalized: Vec<u32>,
    line_map: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedFileView<'a> {
    repo_id: usize,
    repo_label: &'a str,
    rel_path: &'a str,
    normalized: &'a [u32],
    line_map: &'a [u32],
}

#[derive(Debug)]
struct SpanGroupBuilder {
    content_hash: u64,
    normalized_len: usize,
    sample: Vec<u32>,
    preview: String,
    occurrences: Vec<DuplicateSpanOccurrence>,
    occurrence_keys: HashSet<(usize, usize)>,
    repo_ids: HashSet<usize>,
}

pub fn find_duplicate_files(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateGroup>> {
    Ok(find_duplicate_files_with_stats(roots, options)?.result)
}

pub fn find_duplicate_files_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let mut stats = ScanStats::default();
    let mut all_files = Vec::new();
    for repo in &repos {
        let files = collect_repo_files(repo, options, &mut stats)?;
        stats.candidate_files = stats.candidate_files.saturating_add(files.len() as u64);
        all_files.extend(files);
    }

    #[derive(Debug)]
    struct GroupBuilder {
        content_hash: u64,
        normalized_len: usize,
        sample: Vec<u8>,
        files: Vec<DuplicateFile>,
        repo_ids: HashSet<usize>,
    }

    let mut groups: HashMap<(u64, usize), Vec<GroupBuilder>> = HashMap::new();

    let total_files = all_files.len();
    for (idx, repo_file) in all_files.into_iter().enumerate() {
        if let Some(max_files) = options.max_files
            && stats.scanned_files as usize >= max_files
        {
            stats.skipped_budget_max_files = stats
                .skipped_budget_max_files
                .saturating_add((total_files - idx) as u64);
            break;
        }

        let metadata = match fs::metadata(&repo_file.abs_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if let Some(max_file_size) = options.max_file_size
            && metadata.len() > max_file_size
        {
            stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
            continue;
        }
        if let Some(max_total_bytes) = options.max_total_bytes
            && stats.scanned_bytes.saturating_add(metadata.len()) > max_total_bytes
        {
            stats.skipped_budget_max_total_bytes =
                stats.skipped_budget_max_total_bytes.saturating_add(1);
            continue;
        }

        let bytes = match fs::read(&repo_file.abs_path) {
            Ok(b) => b,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if bytes.contains(&0) {
            stats.skipped_binary = stats.skipped_binary.saturating_add(1);
            continue;
        }
        stats.scanned_files = stats.scanned_files.saturating_add(1);
        stats.scanned_bytes = stats.scanned_bytes.saturating_add(bytes.len() as u64);

        let normalized = normalize_whitespace(&bytes);
        let content_hash = fnv1a64(&normalized);

        let key = (content_hash, normalized.len());
        let bucket = groups.entry(key).or_default();

        let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
        let file = DuplicateFile {
            repo_id: repo_file.repo_id,
            repo_label: repo_file.repo_label.clone(),
            path: rel_path,
        };

        if let Some(existing) = bucket.iter_mut().find(|g| g.sample == normalized) {
            existing.repo_ids.insert(file.repo_id);
            existing.files.push(file);
            continue;
        }

        let mut repo_ids = HashSet::new();
        repo_ids.insert(file.repo_id);
        bucket.push(GroupBuilder {
            content_hash,
            normalized_len: normalized.len(),
            sample: normalized,
            files: vec![file],
            repo_ids,
        });
    }

    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.files.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder
                .files
                .sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
            out.push(DuplicateGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                files: builder.files,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.files.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.files.len(),
        ))
    });
    Ok(ScanOutcome { result: out, stats })
}

pub fn find_duplicate_code_spans(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateSpanGroup>> {
    Ok(find_duplicate_code_spans_with_stats(roots, options)?.result)
}

pub fn find_duplicate_code_spans_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateSpanGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;

    let min_match_len = options.min_match_len.max(1);
    let fingerprint_len = min_match_len.clamp(1, 25);
    let window_size = min_match_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let mut stats = ScanStats::default();
    let mut repo_files = Vec::new();
    for repo in &repos {
        let files = collect_repo_files(repo, options, &mut stats)?;
        stats.candidate_files = stats.candidate_files.saturating_add(files.len() as u64);
        repo_files.extend(files);
    }

    let mut files = Vec::new();
    let total_files = repo_files.len();
    for (idx, repo_file) in repo_files.into_iter().enumerate() {
        if let Some(max_files) = options.max_files
            && stats.scanned_files as usize >= max_files
        {
            stats.skipped_budget_max_files = stats
                .skipped_budget_max_files
                .saturating_add((total_files - idx) as u64);
            break;
        }

        let metadata = match fs::metadata(&repo_file.abs_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if let Some(max_file_size) = options.max_file_size
            && metadata.len() > max_file_size
        {
            stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
            continue;
        }
        if let Some(max_total_bytes) = options.max_total_bytes
            && stats.scanned_bytes.saturating_add(metadata.len()) > max_total_bytes
        {
            stats.skipped_budget_max_total_bytes =
                stats.skipped_budget_max_total_bytes.saturating_add(1);
            continue;
        }

        let bytes = match fs::read(&repo_file.abs_path) {
            Ok(b) => b,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if bytes.contains(&0) {
            stats.skipped_binary = stats.skipped_binary.saturating_add(1);
            continue;
        }
        stats.scanned_files = stats.scanned_files.saturating_add(1);
        stats.scanned_bytes = stats.scanned_bytes.saturating_add(bytes.len() as u64);

        let normalized = normalize_for_code_spans(&bytes);
        if normalized.chars.len() < min_match_len {
            continue;
        }

        let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
        files.push(NormalizedFile {
            repo_id: repo_file.repo_id,
            repo_label: repo_file.repo_label,
            rel_path,
            normalized: normalized.chars,
            line_map: normalized.line_map,
        });
    }

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in files.iter().enumerate() {
        for (hash, pos) in winnowed_fingerprints(&file.normalized, fingerprint_len, window_size) {
            fingerprints
                .entry(hash)
                .or_default()
                .push(FingerprintOcc { file_id, pos });
        }
    }

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    struct MatchKey {
        file_a: usize,
        start_a: usize,
        file_b: usize,
        start_b: usize,
        len: usize,
    }

    const MAX_BUCKET: usize = 512;

    let mut seen_matches: HashSet<MatchKey> = HashSet::new();
    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for mut occs in fingerprints.into_values() {
        if occs.len() <= 1 {
            continue;
        }
        if occs.len() > MAX_BUCKET {
            occs.truncate(MAX_BUCKET);
        }

        for i in 0..occs.len() {
            for j in (i + 1)..occs.len() {
                let a = occs[i];
                let b = occs[j];
                if a.file_id == b.file_id && a.pos == b.pos {
                    continue;
                }

                let (start_a, start_b, len) = match maximal_match(
                    &files[a.file_id].normalized,
                    a.pos,
                    &files[b.file_id].normalized,
                    b.pos,
                    fingerprint_len,
                ) {
                    Some(v) => v,
                    None => continue,
                };

                if len < min_match_len {
                    continue;
                }

                if a.file_id == b.file_id {
                    let a_end = start_a + len;
                    let b_end = start_b + len;
                    if start_a < b_end && start_b < a_end {
                        continue;
                    }
                }

                let (file_a, file_b, start_a, start_b) =
                    canonicalize_match(a.file_id, b.file_id, start_a, start_b);
                let key = MatchKey {
                    file_a,
                    start_a,
                    file_b,
                    start_b,
                    len,
                };
                if !seen_matches.insert(key) {
                    continue;
                }

                let sample = files[file_a].normalized[start_a..start_a + len].to_vec();
                let content_hash = fnv1a64_u32(&sample);

                let bucket = groups.entry((content_hash, len)).or_default();
                let builder = match bucket.iter_mut().find(|g| g.sample == sample) {
                    Some(existing) => existing,
                    None => {
                        bucket.push(SpanGroupBuilder {
                            content_hash,
                            normalized_len: len,
                            sample,
                            preview: String::new(),
                            occurrences: Vec::new(),
                            occurrence_keys: HashSet::new(),
                            repo_ids: HashSet::new(),
                        });
                        bucket.last_mut().expect("just pushed")
                    }
                };

                add_occurrence(builder, &files[file_a], file_a, start_a, len);
                add_occurrence(builder, &files[file_b], file_b, start_b, len);
            }
        }
    }

    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.occurrences.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder.occurrences.sort_by(|a, b| {
                (a.repo_id, &a.repo_label, &a.path, a.start_line, a.end_line).cmp(&(
                    b.repo_id,
                    &b.repo_label,
                    &b.path,
                    b.start_line,
                    b.end_line,
                ))
            });

            out.push(DuplicateSpanGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                preview: make_preview(&builder.sample, 80),
                occurrences: builder.occurrences,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.occurrences.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.occurrences.len(),
        ))
    });
    Ok(ScanOutcome { result: out, stats })
}

#[derive(Debug)]
struct ScannedTextFile {
    repo_id: usize,
    repo_label: String,
    path: String,
    text: String,
    code_chars: Vec<u32>,
    code_char_lines: Vec<u32>,
    line_tokens: Vec<u32>,
    line_token_lines: Vec<u32>,
    line_token_char_lens: Vec<usize>,
    tokens: Vec<u32>,
    token_lines: Vec<u32>,
    blocks: Vec<BlockNode>,
}

#[derive(Debug, Clone)]
struct BlockNode {
    start_token: usize,
    end_token: usize,
    start_line: u32,
    end_line: u32,
    depth: u32,
    children: Vec<usize>,
}

pub fn generate_duplication_report(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<DuplicationReport> {
    Ok(generate_duplication_report_with_stats(roots, options)?.result)
}

pub fn generate_duplication_report_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<DuplicationReport>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: DuplicationReport {
                file_duplicates: Vec::new(),
                code_span_duplicates: Vec::new(),
                line_span_duplicates: Vec::new(),
                token_span_duplicates: Vec::new(),
                block_duplicates: Vec::new(),
                ast_subtree_duplicates: Vec::new(),
                similar_blocks_minhash: Vec::new(),
                similar_blocks_simhash: Vec::new(),
            },
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;
    if options.max_report_items == 0 {
        return Ok(ScanOutcome {
            result: DuplicationReport {
                file_duplicates: Vec::new(),
                code_span_duplicates: Vec::new(),
                line_span_duplicates: Vec::new(),
                token_span_duplicates: Vec::new(),
                block_duplicates: Vec::new(),
                ast_subtree_duplicates: Vec::new(),
                similar_blocks_minhash: Vec::new(),
                similar_blocks_simhash: Vec::new(),
            },
            stats: ScanStats::default(),
        });
    }

    let mut stats = ScanStats::default();
    let (files, file_duplicates) = scan_text_files_for_report(roots, options, &mut stats)?;

    let code_span_duplicates = detect_duplicate_code_spans(&files, options);
    let line_span_duplicates = detect_duplicate_line_spans(&files, options);
    let token_span_duplicates = detect_duplicate_token_spans(&files, options);
    let block_duplicates = detect_duplicate_blocks(&files, options);
    let ast_subtree_duplicates = detect_duplicate_ast_subtrees(&files, options);
    let similar_blocks_minhash = find_similar_blocks_minhash(&files, options);
    let similar_blocks_simhash = find_similar_blocks_simhash(&files, options);

    Ok(ScanOutcome {
        result: DuplicationReport {
            file_duplicates,
            code_span_duplicates,
            line_span_duplicates,
            token_span_duplicates,
            block_duplicates,
            ast_subtree_duplicates,
            similar_blocks_minhash,
            similar_blocks_simhash,
        },
        stats,
    })
}

fn scan_text_files_for_report(
    roots: &[PathBuf],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<(Vec<ScannedTextFile>, Vec<DuplicateGroup>)> {
    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let mut repo_files = Vec::new();
    for repo in &repos {
        let files = collect_repo_files(repo, options, stats)?;
        stats.candidate_files = stats.candidate_files.saturating_add(files.len() as u64);
        repo_files.extend(files);
    }

    #[derive(Debug)]
    struct FileGroupBuilder {
        content_hash: u64,
        normalized_len: usize,
        sample: Vec<u8>,
        files: Vec<DuplicateFile>,
        repo_ids: HashSet<usize>,
    }

    let mut file_groups: HashMap<(u64, usize), Vec<FileGroupBuilder>> = HashMap::new();
    let mut files = Vec::new();

    let total_files = repo_files.len();
    for (idx, repo_file) in repo_files.into_iter().enumerate() {
        if let Some(max_files) = options.max_files
            && stats.scanned_files as usize >= max_files
        {
            stats.skipped_budget_max_files = stats
                .skipped_budget_max_files
                .saturating_add((total_files - idx) as u64);
            break;
        }

        let metadata = match fs::metadata(&repo_file.abs_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if let Some(max_file_size) = options.max_file_size
            && metadata.len() > max_file_size
        {
            stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
            continue;
        }
        if let Some(max_total_bytes) = options.max_total_bytes
            && stats.scanned_bytes.saturating_add(metadata.len()) > max_total_bytes
        {
            stats.skipped_budget_max_total_bytes =
                stats.skipped_budget_max_total_bytes.saturating_add(1);
            continue;
        }

        let bytes = match fs::read(&repo_file.abs_path) {
            Ok(b) => b,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };
        if bytes.contains(&0) {
            stats.skipped_binary = stats.skipped_binary.saturating_add(1);
            continue;
        }
        stats.scanned_files = stats.scanned_files.saturating_add(1);
        stats.scanned_bytes = stats.scanned_bytes.saturating_add(bytes.len() as u64);

        let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);

        // 1) File duplicates (whitespace-insensitive)
        let normalized_ws = normalize_whitespace(&bytes);
        let content_hash = fnv1a64(&normalized_ws);
        let key = (content_hash, normalized_ws.len());
        let bucket = file_groups.entry(key).or_default();

        let file = DuplicateFile {
            repo_id: repo_file.repo_id,
            repo_label: repo_file.repo_label.clone(),
            path: rel_path.clone(),
        };

        if let Some(existing) = bucket.iter_mut().find(|g| g.sample == normalized_ws) {
            existing.repo_ids.insert(file.repo_id);
            existing.files.push(file);
        } else {
            let mut repo_ids = HashSet::new();
            repo_ids.insert(file.repo_id);
            bucket.push(FileGroupBuilder {
                content_hash,
                normalized_len: normalized_ws.len(),
                sample: normalized_ws,
                files: vec![file],
                repo_ids,
            });
        }

        // 2) Text-based detectors
        let text = String::from_utf8_lossy(&bytes).to_string();
        let code_norm = normalize_for_code_spans(&bytes);
        let line_norm = normalize_lines_for_dup_detection(&text);
        let tokenized = tokenize_for_dup_detection(&text);
        let blocks = parse_brace_blocks(&tokenized.tokens, &tokenized.token_lines);

        files.push(ScannedTextFile {
            repo_id: repo_file.repo_id,
            repo_label: repo_file.repo_label,
            path: rel_path,
            text,
            code_chars: code_norm.chars,
            code_char_lines: code_norm.line_map,
            line_tokens: line_norm.line_tokens,
            line_token_lines: line_norm.line_lines,
            line_token_char_lens: line_norm.line_lens,
            tokens: tokenized.tokens,
            token_lines: tokenized.token_lines,
            blocks,
        });
    }

    let mut file_duplicates = Vec::new();
    for builders in file_groups.into_values() {
        for builder in builders {
            if builder.files.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            let mut files = builder.files;
            files.sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
            file_duplicates.push(DuplicateGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                files,
            });
        }
    }

    sort_duplicate_groups_for_report(&mut file_duplicates);
    file_duplicates.truncate(options.max_report_items);

    Ok((files, file_duplicates))
}

#[derive(Debug)]
struct LineNormalizedText {
    line_tokens: Vec<u32>,
    line_lines: Vec<u32>,
    line_lens: Vec<usize>,
}

fn normalize_lines_for_dup_detection(text: &str) -> LineNormalizedText {
    let mut line: u32 = 1;
    let mut current: Vec<u32> = Vec::new();

    let mut line_tokens = Vec::new();
    let mut line_lines = Vec::new();
    let mut line_lens = Vec::new();

    for ch in text.chars() {
        if ch == '\n' {
            if !current.is_empty() {
                line_lens.push(current.len());
                line_tokens.push(fold_u64_to_u32(fnv1a64_u32(&current)));
                line_lines.push(line);
            }
            current.clear();
            line = line.saturating_add(1);
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch as u32);
        }
    }

    if !current.is_empty() {
        line_lens.push(current.len());
        line_tokens.push(fold_u64_to_u32(fnv1a64_u32(&current)));
        line_lines.push(line);
    }

    LineNormalizedText {
        line_tokens,
        line_lines,
        line_lens,
    }
}

fn fold_u64_to_u32(value: u64) -> u32 {
    (value as u32) ^ ((value >> 32) as u32)
}

#[derive(Debug)]
struct TokenizedText {
    tokens: Vec<u32>,
    token_lines: Vec<u32>,
}

fn tokenize_for_dup_detection(text: &str) -> TokenizedText {
    const TOK_IDENT: u32 = 1;
    const TOK_NUM: u32 = 2;
    const TOK_STR: u32 = 3;
    const TOK_PUNCT_BASE: u32 = 10_000;

    fn keyword_token(ident: &str) -> Option<u32> {
        Some(match ident {
            "if" => 100,
            "else" => 101,
            "for" => 102,
            "while" => 103,
            "do" => 104,
            "switch" => 105,
            "case" => 106,
            "break" => 107,
            "continue" => 108,
            "return" => 109,
            "try" => 110,
            "catch" => 111,
            "finally" => 112,
            "throw" => 113,
            "fn" => 114,
            "function" => 115,
            "class" => 116,
            "struct" => 117,
            "enum" => 118,
            "impl" => 119,
            "trait" => 120,
            "const" => 121,
            "let" => 122,
            "var" => 123,
            "static" => 124,
            "public" => 125,
            "private" => 126,
            "protected" => 127,
            "async" => 128,
            "await" => 129,
            _ => return None,
        })
    }

    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut line: u32 = 1;

    let mut tokens = Vec::new();
    let mut token_lines = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line = line.saturating_add(1);
            i += 1;
            continue;
        }
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'\n' {
                    line = line.saturating_add(1);
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b == b'#' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if b == b'"' || b == b'\'' {
            let quote = b;
            let start_line = line;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\n' {
                    line = line.saturating_add(1);
                }
                if c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(TOK_STR);
            token_lines.push(start_line);
            continue;
        }

        if (b as char).is_ascii_alphabetic() || b == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if (c as char).is_ascii_alphanumeric() || c == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let ident = &text[start..i];
            let tok = keyword_token(ident).unwrap_or(TOK_IDENT);
            tokens.push(tok);
            token_lines.push(line);
            continue;
        }

        if (b as char).is_ascii_digit() {
            i += 1;
            while i < bytes.len() && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(TOK_NUM);
            token_lines.push(line);
            continue;
        }

        tokens.push(TOK_PUNCT_BASE + u32::from(b));
        token_lines.push(line);
        i += 1;
    }

    TokenizedText {
        tokens,
        token_lines,
    }
}

fn parse_brace_blocks(tokens: &[u32], token_lines: &[u32]) -> Vec<BlockNode> {
    const TOK_PUNCT_BASE: u32 = 10_000;
    let open = TOK_PUNCT_BASE + u32::from(b'{');
    let close = TOK_PUNCT_BASE + u32::from(b'}');

    let mut nodes: Vec<BlockNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for (idx, &tok) in tokens.iter().enumerate() {
        if tok == open {
            let depth = (stack.len() as u32) + 1;
            let node_id = nodes.len();
            nodes.push(BlockNode {
                start_token: idx,
                end_token: idx,
                start_line: token_lines.get(idx).copied().unwrap_or(1),
                end_line: token_lines.get(idx).copied().unwrap_or(1),
                depth,
                children: Vec::new(),
            });
            if let Some(parent_id) = stack.last().copied() {
                nodes[parent_id].children.push(node_id);
            }
            stack.push(node_id);
        } else if tok == close {
            let Some(node_id) = stack.pop() else {
                continue;
            };
            nodes[node_id].end_token = idx;
            nodes[node_id].end_line = token_lines
                .get(idx)
                .copied()
                .unwrap_or(nodes[node_id].start_line);
        }
    }

    nodes
}

fn preview_from_lines(text: &str, start_line: u32, end_line: u32, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = (idx as u32) + 1;
        if line_no < start_line {
            continue;
        }
        if line_no > end_line {
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        if out.len() >= max_chars {
            out.truncate(max_chars);
            break;
        }
    }
    out
}

fn sort_duplicate_groups_for_report(groups: &mut Vec<DuplicateGroup>) {
    groups.sort_by(|a, b| {
        b.files
            .len()
            .cmp(&a.files.len())
            .then_with(|| b.normalized_len.cmp(&a.normalized_len))
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
}

fn sort_span_groups_for_report(groups: &mut Vec<DuplicateSpanGroup>) {
    groups.sort_by(|a, b| {
        b.occurrences
            .len()
            .cmp(&a.occurrences.len())
            .then_with(|| b.normalized_len.cmp(&a.normalized_len))
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
}

fn detect_duplicate_code_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_match_len = options.min_match_len.max(1);
    let fingerprint_len = min_match_len.clamp(1, 25);
    let window_size = min_match_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let mut normalized = Vec::new();
    for file in files {
        if file.code_chars.len() < min_match_len {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.code_chars,
            line_map: &file.code_char_lines,
        });
    }

    if normalized.is_empty() {
        return Vec::new();
    }

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in normalized.iter().enumerate() {
        for (hash, pos) in winnowed_fingerprints(file.normalized, fingerprint_len, window_size) {
            fingerprints
                .entry(hash)
                .or_default()
                .push(FingerprintOcc { file_id, pos });
        }
    }

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    struct MatchKey {
        file_a: usize,
        start_a: usize,
        file_b: usize,
        start_b: usize,
        len: usize,
    }

    const MAX_BUCKET: usize = 512;

    let mut seen_matches: HashSet<MatchKey> = HashSet::new();
    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for mut occs in fingerprints.into_values() {
        if occs.len() <= 1 {
            continue;
        }
        if occs.len() > MAX_BUCKET {
            occs.truncate(MAX_BUCKET);
        }

        for i in 0..occs.len() {
            for j in (i + 1)..occs.len() {
                let a = occs[i];
                let b = occs[j];
                if a.file_id == b.file_id && a.pos == b.pos {
                    continue;
                }

                let (start_a, start_b, len) = match maximal_match(
                    normalized[a.file_id].normalized,
                    a.pos,
                    normalized[b.file_id].normalized,
                    b.pos,
                    fingerprint_len,
                ) {
                    Some(v) => v,
                    None => continue,
                };

                if len < min_match_len {
                    continue;
                }

                if a.file_id == b.file_id {
                    let a_end = start_a + len;
                    let b_end = start_b + len;
                    if start_a < b_end && start_b < a_end {
                        continue;
                    }
                }

                let (file_a, file_b, start_a, start_b) =
                    canonicalize_match(a.file_id, b.file_id, start_a, start_b);
                let key = MatchKey {
                    file_a,
                    start_a,
                    file_b,
                    start_b,
                    len,
                };
                if !seen_matches.insert(key) {
                    continue;
                }

                let sample = normalized[file_a].normalized[start_a..start_a + len].to_vec();
                let content_hash = fnv1a64_u32(&sample);
                let preview = make_preview(&sample, 80);

                let bucket = groups.entry((content_hash, len)).or_default();
                let builder = match bucket.iter_mut().find(|g| g.sample == sample) {
                    Some(existing) => existing,
                    None => {
                        bucket.push(SpanGroupBuilder {
                            content_hash,
                            normalized_len: len,
                            sample,
                            preview,
                            occurrences: Vec::new(),
                            occurrence_keys: HashSet::new(),
                            repo_ids: HashSet::new(),
                        });
                        bucket.last_mut().expect("just pushed")
                    }
                };

                add_occurrence_view(builder, &normalized[file_a], file_a, start_a, len);
                add_occurrence_view(builder, &normalized[file_b], file_b, start_b, len);
            }
        }
    }

    let mut out = finalize_span_groups(groups, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

fn detect_duplicate_line_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_char_len = options.min_match_len.max(1);

    let mut normalized = Vec::new();
    let mut file_line_lens = Vec::new();
    let mut file_texts = Vec::new();

    for file in files {
        if file.line_tokens.is_empty() {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.line_tokens,
            line_map: &file.line_token_lines,
        });
        file_line_lens.push(file.line_token_char_lens.as_slice());
        file_texts.push(file.text.as_str());
    }

    detect_duplicate_span_groups_with_len_filter(
        &normalized,
        2,
        2,
        8,
        options.cross_repo_only,
        |file_id, start, len| {
            let lens = file_line_lens[file_id];
            let mut total = 0usize;
            for &l in &lens[start..start + len] {
                total += l;
                if total >= min_char_len {
                    return true;
                }
            }
            false
        },
        |file_id, start_line, end_line| {
            preview_from_lines(file_texts[file_id], start_line, end_line, 120)
        },
        options.max_report_items,
    )
}

fn detect_duplicate_token_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);
    let fingerprint_len = min_token_len.clamp(1, 25);
    let window_size = min_token_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let mut normalized = Vec::new();
    let mut file_texts = Vec::new();

    for file in files {
        if file.tokens.len() < min_token_len {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.tokens,
            line_map: &file.token_lines,
        });
        file_texts.push(file.text.as_str());
    }

    detect_duplicate_span_groups_with_len_filter(
        &normalized,
        min_token_len,
        fingerprint_len,
        window_size,
        options.cross_repo_only,
        |_file_id, _start, _len| true,
        |file_id, start_line, end_line| {
            preview_from_lines(file_texts[file_id], start_line, end_line, 120)
        },
        options.max_report_items,
    )
}

fn detect_duplicate_blocks(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for (file_id, file) in files.iter().enumerate() {
        for node in &file.blocks {
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < min_token_len {
                continue;
            }
            let content_hash = fnv1a64_u32(slice);
            let key = (content_hash, slice.len());
            let bucket = groups.entry(key).or_default();

            let builder = match bucket.iter_mut().find(|g| g.sample == slice) {
                Some(existing) => existing,
                None => {
                    let preview =
                        preview_from_lines(&file.text, node.start_line, node.end_line, 120);
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: slice.len(),
                        sample: slice.to_vec(),
                        preview: String::new(),
                        occurrences: Vec::new(),
                        occurrence_keys: HashSet::new(),
                        repo_ids: HashSet::new(),
                    });
                    let b = bucket.last_mut().expect("just pushed");
                    b.occurrences.push(DuplicateSpanOccurrence {
                        repo_id: file.repo_id,
                        repo_label: file.repo_label.clone(),
                        path: file.path.clone(),
                        start_line: node.start_line,
                        end_line: node.end_line,
                    });
                    b.repo_ids.insert(file.repo_id);
                    b.occurrence_keys.insert((file_id, node.start_token));
                    b.preview = preview;
                    continue;
                }
            };

            if !builder.occurrence_keys.insert((file_id, node.start_token)) {
                continue;
            }
            builder.repo_ids.insert(file.repo_id);
            builder.occurrences.push(DuplicateSpanOccurrence {
                repo_id: file.repo_id,
                repo_label: file.repo_label.clone(),
                path: file.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out = finalize_span_groups(groups, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

fn detect_duplicate_ast_subtrees(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    #[derive(Debug, Clone)]
    struct NodeRepr {
        hash: u64,
        repr: Vec<u32>,
    }

    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for (file_id, file) in files.iter().enumerate() {
        let mut reprs: Vec<Option<NodeRepr>> = vec![None; file.blocks.len()];
        let mut by_depth: Vec<usize> = (0..file.blocks.len()).collect();
        by_depth.sort_by_key(|&i| std::cmp::Reverse(file.blocks[i].depth));

        for node_id in by_depth {
            let node = &file.blocks[node_id];
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }

            let mut children: Vec<(usize, usize, usize)> = node
                .children
                .iter()
                .map(|&cid| {
                    let c = &file.blocks[cid];
                    (c.start_token, c.end_token, cid)
                })
                .collect();
            children.sort_by_key(|c| c.0);

            let mut repr = Vec::new();
            let mut idx = start;
            for (c_start, c_end, cid) in children {
                while idx < c_start && idx < node.end_token {
                    repr.push(file.tokens[idx]);
                    idx += 1;
                }
                if idx == c_start {
                    let child_hash = reprs[cid].as_ref().map(|r| r.hash).unwrap_or(0);
                    repr.push(50_000);
                    repr.push(fold_u64_to_u32(child_hash));
                    idx = c_end.saturating_add(1);
                }
            }
            while idx < node.end_token {
                repr.push(file.tokens[idx]);
                idx += 1;
            }

            let hash = fnv1a64_u32(&repr);
            reprs[node_id] = Some(NodeRepr { hash, repr });

            let repr_len = reprs[node_id].as_ref().map(|r| r.repr.len()).unwrap_or(0);
            if repr_len < min_token_len {
                continue;
            }

            let content_hash = hash;
            let key = (content_hash, repr_len);
            let bucket = groups.entry(key).or_default();

            let builder = match bucket.iter_mut().find(|g| {
                reprs[node_id].as_ref().map(|r| r.repr.as_slice()) == Some(g.sample.as_slice())
            }) {
                Some(existing) => existing,
                None => {
                    let preview =
                        preview_from_lines(&file.text, node.start_line, node.end_line, 120);
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: repr_len,
                        sample: reprs[node_id]
                            .as_ref()
                            .map(|r| r.repr.clone())
                            .unwrap_or_default(),
                        preview: String::new(),
                        occurrences: Vec::new(),
                        occurrence_keys: HashSet::new(),
                        repo_ids: HashSet::new(),
                    });
                    let b = bucket.last_mut().expect("just pushed");
                    b.occurrences.push(DuplicateSpanOccurrence {
                        repo_id: file.repo_id,
                        repo_label: file.repo_label.clone(),
                        path: file.path.clone(),
                        start_line: node.start_line,
                        end_line: node.end_line,
                    });
                    b.repo_ids.insert(file.repo_id);
                    b.occurrence_keys.insert((file_id, node.start_token));
                    b.preview = preview;
                    continue;
                }
            };

            if !builder.occurrence_keys.insert((file_id, node.start_token)) {
                continue;
            }
            builder.repo_ids.insert(file.repo_id);
            builder.occurrences.push(DuplicateSpanOccurrence {
                repo_id: file.repo_id,
                repo_label: file.repo_label.clone(),
                path: file.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out = finalize_span_groups(groups, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

#[allow(clippy::too_many_arguments)]
fn detect_duplicate_span_groups_with_len_filter<'a>(
    files: &[NormalizedFileView<'a>],
    min_len: usize,
    fingerprint_len: usize,
    window_size: usize,
    cross_repo_only: bool,
    accept_match: impl Fn(usize, usize, usize) -> bool,
    preview_from_occurrence: impl Fn(usize, u32, u32) -> String,
    max_items: usize,
) -> Vec<DuplicateSpanGroup> {
    if max_items == 0 {
        return Vec::new();
    }
    if files.is_empty() {
        return Vec::new();
    }

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in files.iter().enumerate() {
        if file.normalized.len() < fingerprint_len {
            continue;
        }
        for (hash, pos) in winnowed_fingerprints(file.normalized, fingerprint_len, window_size) {
            fingerprints
                .entry(hash)
                .or_default()
                .push(FingerprintOcc { file_id, pos });
        }
    }

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    struct MatchKey {
        file_a: usize,
        start_a: usize,
        file_b: usize,
        start_b: usize,
        len: usize,
    }

    const MAX_BUCKET: usize = 512;

    let mut seen_matches: HashSet<MatchKey> = HashSet::new();
    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for mut occs in fingerprints.into_values() {
        if occs.len() <= 1 {
            continue;
        }
        if occs.len() > MAX_BUCKET {
            occs.truncate(MAX_BUCKET);
        }

        for i in 0..occs.len() {
            for j in (i + 1)..occs.len() {
                let a = occs[i];
                let b = occs[j];
                if a.file_id == b.file_id && a.pos == b.pos {
                    continue;
                }

                let (start_a, start_b, len) = match maximal_match(
                    files[a.file_id].normalized,
                    a.pos,
                    files[b.file_id].normalized,
                    b.pos,
                    fingerprint_len,
                ) {
                    Some(v) => v,
                    None => continue,
                };

                if len < min_len {
                    continue;
                }
                if !accept_match(a.file_id, start_a, len) || !accept_match(b.file_id, start_b, len)
                {
                    continue;
                }

                if a.file_id == b.file_id {
                    let a_end = start_a + len;
                    let b_end = start_b + len;
                    if start_a < b_end && start_b < a_end {
                        continue;
                    }
                }

                let (file_a, file_b, start_a, start_b) =
                    canonicalize_match(a.file_id, b.file_id, start_a, start_b);
                let key = MatchKey {
                    file_a,
                    start_a,
                    file_b,
                    start_b,
                    len,
                };
                if !seen_matches.insert(key) {
                    continue;
                }

                let sample = files[file_a].normalized[start_a..start_a + len].to_vec();
                let content_hash = fnv1a64_u32(&sample);
                let bucket = groups.entry((content_hash, len)).or_default();
                let builder = match bucket.iter_mut().find(|g| g.sample == sample) {
                    Some(existing) => existing,
                    None => {
                        bucket.push(SpanGroupBuilder {
                            content_hash,
                            normalized_len: len,
                            sample,
                            preview: String::new(),
                            occurrences: Vec::new(),
                            occurrence_keys: HashSet::new(),
                            repo_ids: HashSet::new(),
                        });
                        bucket.last_mut().expect("just pushed")
                    }
                };

                if builder.occurrences.is_empty()
                    && let (Some(&start_line), Some(&end_line)) = (
                        files[file_a].line_map.get(start_a),
                        files[file_a].line_map.get(start_a + len - 1),
                    )
                {
                    builder.preview = preview_from_occurrence(file_a, start_line, end_line);
                }

                add_occurrence_view(builder, &files[file_a], file_a, start_a, len);
                add_occurrence_view(builder, &files[file_b], file_b, start_b, len);
            }
        }
    }

    let mut out = finalize_span_groups(
        groups,
        &ScanOptions {
            cross_repo_only,
            ..ScanOptions::default()
        },
    );
    sort_span_groups_for_report(&mut out);
    out.truncate(max_items);
    out
}

fn finalize_span_groups(
    groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>>,
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.occurrences.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder.occurrences.sort_by(|a, b| {
                (a.repo_id, &a.repo_label, &a.path, a.start_line, a.end_line).cmp(&(
                    b.repo_id,
                    &b.repo_label,
                    &b.path,
                    b.start_line,
                    b.end_line,
                ))
            });

            out.push(DuplicateSpanGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                preview: builder.preview,
                occurrences: builder.occurrences,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.occurrences.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.occurrences.len(),
        ))
    });
    out
}

fn find_similar_blocks_minhash(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const SIG_SIZE: usize = 32;
    const BAND_SIZE: usize = 4;
    const BANDS: usize = SIG_SIZE / BAND_SIZE;

    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    let seeds: [u64; SIG_SIZE] = {
        let mut out = [0u64; SIG_SIZE];
        let mut s = 0x1234_5678_9abc_def0u64;
        for v in &mut out {
            s = splitmix64(s);
            *v = s;
        }
        out
    };

    #[derive(Debug)]
    struct BlockSig {
        occ: DuplicateSpanOccurrence,
        signature: [u32; SIG_SIZE],
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut mins = [u32::MAX; SIG_SIZE];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                for i in 0..SIG_SIZE {
                    let h = splitmix64(base ^ seeds[i]) as u32;
                    if h < mins[i] {
                        mins[i] = h;
                    }
                }
            }

            blocks.push(BlockSig {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: file.repo_label.clone(),
                    path: file.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                signature: mins,
            });
        }
    }

    let mut buckets: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let start = band * BAND_SIZE;
            let key_hash = fnv1a64_u32(&blk.signature[start..start + BAND_SIZE]);
            buckets.entry((band, key_hash)).or_default().push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let sig_a = &blocks[key.0].signature;
                let sig_b = &blocks[key.1].signature;
                let eq = sig_a.iter().zip(sig_b).filter(|(x, y)| x == y).count();
                let score = eq as f64 / SIG_SIZE as f64;
                if score < options.similarity_threshold {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: None,
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}

fn find_similar_blocks_simhash(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const BANDS: usize = 4;
    const BAND_BITS: u32 = 16;

    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    #[derive(Debug)]
    struct BlockHash {
        occ: DuplicateSpanOccurrence,
        hash: u64,
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut sums = [0i32; 64];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                let h = splitmix64(base);
                for (bit, sum) in sums.iter_mut().enumerate() {
                    if (h >> bit) & 1 == 1 {
                        *sum += 1;
                    } else {
                        *sum -= 1;
                    }
                }
            }

            let mut hash = 0u64;
            for (bit, sum) in sums.iter().enumerate() {
                if *sum > 0 {
                    hash |= 1u64 << bit;
                }
            }

            blocks.push(BlockHash {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: file.repo_label.clone(),
                    path: file.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                hash,
            });
        }
    }

    let mut buckets: HashMap<(u32, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let shift = (band as u32) * BAND_BITS;
            let band_value = (blk.hash >> shift) & 0xffff;
            buckets
                .entry((band as u32, band_value))
                .or_default()
                .push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let hamming = (blocks[key.0].hash ^ blocks[key.1].hash).count_ones();
                if hamming > options.simhash_max_distance {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                let score = 1.0 - (hamming as f64 / 64.0);
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: Some(hamming),
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}

fn repo_label(root: &Path, id: usize) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("repo{id}"))
}

fn collect_repo_files(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Vec<RepoFile>> {
    if options.respect_gitignore
        && !options.follow_symlinks
        && let Some(files) = try_collect_repo_files_via_git(repo, options, stats)?
    {
        return Ok(files);
    }

    collect_repo_files_via_walk(repo, options, stats)
}

fn collect_repo_files_via_walk(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Vec<RepoFile>> {
    let ignore_dirs = options.ignore_dirs.clone();
    let follow_symlinks = options.follow_symlinks;
    let respect_gitignore = options.respect_gitignore;
    let is_git_repo = repo.root.join(".git").exists();

    let mut builder = WalkBuilder::new(&repo.root);
    builder
        .hidden(false)
        .follow_links(follow_symlinks)
        .ignore(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore && is_git_repo)
        .git_exclude(respect_gitignore && is_git_repo)
        .parents(false)
        .require_git(false);

    let walker = builder
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if !follow_symlinks && entry.path_is_symlink() {
                return false;
            }

            let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
            if !is_dir {
                return true;
            }

            match entry.file_name().to_str() {
                Some(name) => !ignore_dirs.contains(name),
                None => true,
            }
        })
        .build();

    let mut out = Vec::new();
    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                if let Some(io_err) = err.io_error() {
                    match io_err.kind() {
                        io::ErrorKind::NotFound => {
                            stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                            continue;
                        }
                        io::ErrorKind::PermissionDenied => {
                            stats.skipped_permission_denied =
                                stats.skipped_permission_denied.saturating_add(1);
                            continue;
                        }
                        _ => {}
                    }
                }
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                continue;
            }
        };

        if entry.depth() == 0 {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }

        out.push(RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path: entry.into_path(),
        });
    }

    Ok(out)
}

fn try_collect_repo_files_via_git(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Option<Vec<RepoFile>>> {
    if !repo.root.join(".git").exists() {
        return Ok(None);
    }

    let output = match Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
    {
        Ok(out) => out,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let mut rel_paths = Vec::new();
    for part in output.stdout.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        rel_paths.push(String::from_utf8_lossy(part).to_string());
    }

    if rel_paths.is_empty() {
        return Ok(Some(Vec::new()));
    }

    let ignored = match git_check_ignore(&repo.root, &rel_paths) {
        Ok(set) => set,
        Err(_) => return Ok(None),
    };

    let mut out = Vec::new();
    for rel in rel_paths {
        if ignored.contains(&rel) {
            continue;
        }

        let rel = rel.replace('\\', "/");
        let mut segs = rel.split('/');
        segs.next_back();
        if segs.any(|seg| options.ignore_dirs.contains(seg)) {
            continue;
        }

        let abs_path = repo.root.join(&rel);
        let meta = match fs::symlink_metadata(&abs_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };

        if meta.file_type().is_symlink() && !options.follow_symlinks {
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        out.push(RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path,
        });
    }

    Ok(Some(out))
}

fn git_check_ignore(root: &Path, rel_paths: &[String]) -> io::Result<HashSet<String>> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let Some(mut stdin) = child.stdin.take() else {
            return Err(io::Error::other("git check-ignore stdin not available"));
        };
        for rel in rel_paths {
            stdin.write_all(rel.as_bytes())?;
            stdin.write_all(&[0])?;
        }
    }

    let output = child.wait_with_output()?;
    if output.status.code() == Some(1) {
        return Ok(HashSet::new());
    }
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git check-ignore failed (status={:?})",
            output.status.code()
        )));
    }

    let mut out = HashSet::new();
    for part in output.stdout.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        out.insert(String::from_utf8_lossy(part).to_string());
    }
    Ok(out)
}

fn make_rel_path(root: &Path, abs_path: &Path) -> String {
    match abs_path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => {
            let name = abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>");
            let hash = fnv1a64(abs_path.to_string_lossy().as_bytes());
            format!("<external:{hash:016x}>/{name}")
        }
    }
}

fn normalize_whitespace(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if !b.is_ascii_whitespace() {
            out.push(b);
        }
    }
    out
}

#[derive(Debug)]
struct NormalizedText {
    chars: Vec<u32>,
    line_map: Vec<u32>,
}

fn normalize_for_code_spans(bytes: &[u8]) -> NormalizedText {
    let text = String::from_utf8_lossy(bytes);
    let mut line: u32 = 1;
    let mut chars = Vec::new();
    let mut line_map = Vec::new();

    for ch in text.chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            chars.push(ch as u32);
            line_map.push(line);
        }
    }

    NormalizedText { chars, line_map }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn fnv1a64_u32(codepoints: &[u32]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &cp in codepoints {
        for b in cp.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn winnowed_fingerprints(chars: &[u32], k: usize, window_size: usize) -> Vec<(u64, usize)> {
    use std::collections::VecDeque;

    if k == 0 || window_size == 0 || chars.len() < k {
        return Vec::new();
    }

    const BASE: u64 = 911382323;

    let mut pow = 1u64;
    for _ in 1..k {
        pow = pow.wrapping_mul(BASE);
    }

    let mut hash = 0u64;
    for &cp in &chars[..k] {
        hash = hash
            .wrapping_mul(BASE)
            .wrapping_add(u64::from(cp).wrapping_add(1));
    }

    let mut out = Vec::new();
    let mut deque: VecDeque<(usize, u64)> = VecDeque::new();
    let last_start = chars.len() - k;

    for i in 0..=last_start {
        if i != 0 {
            let out_cp = u64::from(chars[i - 1]).wrapping_add(1);
            let in_cp = u64::from(chars[i + k - 1]).wrapping_add(1);
            hash = hash
                .wrapping_sub(out_cp.wrapping_mul(pow))
                .wrapping_mul(BASE)
                .wrapping_add(in_cp);
        }

        while let Some(&(idx, _)) = deque.front() {
            if idx + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(_, h)) = deque.back() {
            if hash <= h {
                deque.pop_back();
            } else {
                break;
            }
        }
        deque.push_back((i, hash));

        if i + 1 >= window_size {
            let (min_idx, min_hash) = *deque.front().expect("window has items");
            if out.last().map(|&(_, idx)| idx) != Some(min_idx) {
                out.push((min_hash, min_idx));
            }
        }
    }

    out
}

fn maximal_match(
    a: &[u32],
    a_pos: usize,
    b: &[u32],
    b_pos: usize,
    k: usize,
) -> Option<(usize, usize, usize)> {
    if k == 0 || a_pos.checked_add(k)? > a.len() || b_pos.checked_add(k)? > b.len() {
        return None;
    }
    if a[a_pos..a_pos + k] != b[b_pos..b_pos + k] {
        return None;
    }

    let mut start_a = a_pos;
    let mut start_b = b_pos;
    while start_a > 0 && start_b > 0 && a[start_a - 1] == b[start_b - 1] {
        start_a -= 1;
        start_b -= 1;
    }

    let mut end_a = a_pos + k;
    let mut end_b = b_pos + k;
    while end_a < a.len() && end_b < b.len() && a[end_a] == b[end_b] {
        end_a += 1;
        end_b += 1;
    }

    Some((start_a, start_b, end_a - start_a))
}

fn canonicalize_match(
    file_a: usize,
    file_b: usize,
    start_a: usize,
    start_b: usize,
) -> (usize, usize, usize, usize) {
    if (file_a, start_a) <= (file_b, start_b) {
        (file_a, file_b, start_a, start_b)
    } else {
        (file_b, file_a, start_b, start_a)
    }
}

fn add_occurrence(
    builder: &mut SpanGroupBuilder,
    file: &NormalizedFile,
    file_id: usize,
    start: usize,
    len: usize,
) {
    if !builder.occurrence_keys.insert((file_id, start)) {
        return;
    }

    let Some(&start_line) = file.line_map.get(start) else {
        return;
    };
    let Some(&end_line) = file.line_map.get(start + len - 1) else {
        return;
    };

    builder.repo_ids.insert(file.repo_id);
    builder.occurrences.push(DuplicateSpanOccurrence {
        repo_id: file.repo_id,
        repo_label: file.repo_label.clone(),
        path: file.rel_path.clone(),
        start_line,
        end_line,
    });
}

fn add_occurrence_view(
    builder: &mut SpanGroupBuilder,
    file: &NormalizedFileView<'_>,
    file_id: usize,
    start: usize,
    len: usize,
) {
    if !builder.occurrence_keys.insert((file_id, start)) {
        return;
    }

    let Some(&start_line) = file.line_map.get(start) else {
        return;
    };
    let Some(&end_line) = file.line_map.get(start + len - 1) else {
        return;
    };

    builder.repo_ids.insert(file.repo_id);
    builder.occurrences.push(DuplicateSpanOccurrence {
        repo_id: file.repo_id,
        repo_label: file.repo_label.to_string(),
        path: file.rel_path.to_string(),
        start_line,
        end_line,
    });
}

fn make_preview(codepoints: &[u32], max_len: usize) -> String {
    codepoints
        .iter()
        .take(max_len)
        .filter_map(|&cp| char::from_u32(cp))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn normalize_whitespace_removes_ascii_whitespace() {
        let input = b"a \n\tb\r\nc";
        assert_eq!(normalize_whitespace(input), b"abc");
    }

    #[test]
    fn finds_duplicates_within_single_repo() -> io::Result<()> {
        let root = temp_dir("single");
        fs::create_dir_all(&root)?;
        fs::write(root.join("a.txt"), "a b\nc")?;
        fs::write(root.join("b.txt"), "ab\tc")?;
        fs::write(root.join("c.txt"), "different")?;

        let options = ScanOptions::default();
        let groups = find_duplicate_files(&[root], &options)?;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        Ok(())
    }

    #[test]
    fn finds_cross_repo_duplicates_when_enabled() -> io::Result<()> {
        let repo_a = temp_dir("repo_a");
        let repo_b = temp_dir("repo_b");
        fs::create_dir_all(&repo_a)?;
        fs::create_dir_all(&repo_b)?;

        fs::write(repo_a.join("same.txt"), "a b\nc")?;
        fs::write(repo_b.join("same.txt"), "ab\tc")?;
        fs::write(repo_b.join("diff.txt"), "different")?;

        let options = ScanOptions {
            cross_repo_only: true,
            ..ScanOptions::default()
        };

        let groups = find_duplicate_files(&[repo_a, repo_b], &options)?;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        Ok(())
    }

    #[test]
    fn normalize_for_code_spans_strips_symbols_and_whitespace() {
        let input = b"a + b\n_c\r\n123";
        let normalized = normalize_for_code_spans(input);
        let as_string: String = normalized
            .chars
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_eq!(as_string, "ab_c123");
        assert_eq!(normalized.line_map, vec![1, 1, 2, 2, 3, 3, 3]);
    }

    #[test]
    fn finds_duplicate_code_spans_with_line_numbers() -> io::Result<()> {
        let repo_a = temp_dir("span_a");
        let repo_b = temp_dir("span_b");
        fs::create_dir_all(&repo_a)?;
        fs::create_dir_all(&repo_b)?;

        let snippet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

        fs::write(repo_a.join("a.txt"), format!("////\nP{snippet}Q\n"))?;
        fs::write(repo_b.join("b.txt"), format!("####\nR{snippet}S\n"))?;

        let options = ScanOptions::default();
        let groups = find_duplicate_code_spans(&[repo_a.clone(), repo_b.clone()], &options)?;

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].normalized_len, snippet.len());
        assert_eq!(groups[0].occurrences.len(), 2);
        for occ in &groups[0].occurrences {
            assert_eq!(occ.start_line, 2);
            assert_eq!(occ.end_line, 2);
        }
        Ok(())
    }

    #[test]
    fn report_respects_gitignore() -> io::Result<()> {
        let root = temp_dir("gitignore");
        fs::create_dir_all(&root)?;
        fs::write(root.join(".gitignore"), "ignored.txt\n")?;
        fs::write(root.join("a.txt"), "same content")?;
        fs::write(root.join("ignored.txt"), "same content")?;

        let options = ScanOptions::default();
        let report = generate_duplication_report(&[root], &options)?;
        assert_eq!(report.file_duplicates.len(), 0);
        Ok(())
    }

    #[test]
    fn report_respects_nested_gitignore() -> io::Result<()> {
        let root = temp_dir("nested_gitignore");
        let sub = root.join("sub");
        fs::create_dir_all(&sub)?;
        fs::write(sub.join(".gitignore"), "ignored.txt\n")?;
        fs::write(root.join("a.txt"), "same content")?;
        fs::write(sub.join("ignored.txt"), "same content")?;

        let options = ScanOptions::default();
        let report = generate_duplication_report(&[root], &options)?;
        assert_eq!(report.file_duplicates.len(), 0);
        Ok(())
    }

    #[test]
    fn report_can_disable_gitignore() -> io::Result<()> {
        let root = temp_dir("disable_gitignore");
        fs::create_dir_all(&root)?;
        fs::write(root.join(".gitignore"), "ignored.txt\n")?;
        fs::write(root.join("a.txt"), "same content")?;
        fs::write(root.join("ignored.txt"), "same content")?;

        let options = ScanOptions {
            respect_gitignore: false,
            ..ScanOptions::default()
        };

        let report = generate_duplication_report(&[root], &options)?;
        assert_eq!(report.file_duplicates.len(), 1);
        assert_eq!(report.file_duplicates[0].files.len(), 2);
        Ok(())
    }

    #[test]
    fn report_truncates_file_duplicates() -> io::Result<()> {
        let root = temp_dir("report_truncate_files");
        fs::create_dir_all(&root)?;
        fs::write(root.join("a.txt"), "same1")?;
        fs::write(root.join("b.txt"), "same1")?;
        fs::write(root.join("c.txt"), "same2")?;
        fs::write(root.join("d.txt"), "same2")?;

        let options = ScanOptions {
            max_report_items: 1,
            ..ScanOptions::default()
        };
        let report = generate_duplication_report(&[root], &options)?;
        assert_eq!(report.file_duplicates.len(), 1);
        Ok(())
    }

    #[test]
    fn default_max_file_size_skips_large_files() -> io::Result<()> {
        let root = temp_dir("max_file_size");
        fs::create_dir_all(&root)?;

        let data = vec![b'a'; (DEFAULT_MAX_FILE_SIZE_BYTES + 1) as usize];
        fs::write(root.join("a.txt"), &data)?;
        fs::write(root.join("b.txt"), &data)?;

        let options = ScanOptions::default();
        let groups = find_duplicate_files(&[root], &options)?;
        assert_eq!(groups.len(), 0);
        Ok(())
    }

    #[test]
    fn report_finds_token_and_block_duplicates() -> io::Result<()> {
        let repo_a = temp_dir("report_a");
        let repo_b = temp_dir("report_b");
        fs::create_dir_all(&repo_a)?;
        fs::create_dir_all(&repo_b)?;

        fs::write(
            repo_a.join("a.js"),
            "////\nfunction f(x) { return x + 1; }\n",
        )?;
        fs::write(
            repo_b.join("b.js"),
            "####\nfunction g(y) { return y + 1; }\n",
        )?;

        let options = ScanOptions {
            cross_repo_only: true,
            min_match_len: 5,
            min_token_len: 5,
            similarity_threshold: 0.9,
            simhash_max_distance: 3,
            ..ScanOptions::default()
        };

        let report = generate_duplication_report(&[repo_a, repo_b], &options)?;

        assert!(!report.token_span_duplicates.is_empty());
        assert!(!report.block_duplicates.is_empty());
        assert!(!report.ast_subtree_duplicates.is_empty());
        assert!(!report.similar_blocks_minhash.is_empty());
        assert!(!report.similar_blocks_simhash.is_empty());
        Ok(())
    }

    #[test]
    fn follow_symlinks_includes_symlinked_files_in_git_repo() -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            use std::process::Stdio;

            let root = temp_dir("symlink_git");
            fs::create_dir_all(&root)?;

            let git_ok = std::process::Command::new("git")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !git_ok {
                return Ok(());
            }

            let init_ok = std::process::Command::new("git")
                .arg("init")
                .current_dir(&root)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !init_ok {
                return Ok(());
            }

            fs::write(root.join("a.txt"), "a b\nc")?;
            fs::write(root.join("b.txt"), "ab\tc")?;
            symlink("a.txt", root.join("link.txt"))?;

            let options_no = ScanOptions::default();
            let groups_no = find_duplicate_files(&[root.clone()], &options_no)?;
            assert_eq!(groups_no.len(), 1);
            assert_eq!(groups_no[0].files.len(), 2);

            let options_yes = ScanOptions {
                follow_symlinks: true,
                ..ScanOptions::default()
            };
            let groups_yes = find_duplicate_files(&[root], &options_yes)?;
            assert_eq!(groups_yes.len(), 1);
            assert_eq!(groups_yes[0].files.len(), 3);
        }

        Ok(())
    }

    #[test]
    fn scanning_skips_permission_denied_files() -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let root = temp_dir("perm_denied");
            fs::create_dir_all(&root)?;
            fs::write(root.join("a.txt"), "a b\nc")?;
            fs::write(root.join("b.txt"), "ab\tc")?;

            let secret_path = root.join("secret.txt");
            fs::write(&secret_path, "ab\tc")?;

            let mut perms = fs::metadata(&secret_path)?.permissions();
            perms.set_mode(0o000);
            fs::set_permissions(&secret_path, perms)?;

            let options = ScanOptions::default();
            let groups = find_duplicate_files(&[root.clone()], &options)?;

            let mut perms = fs::metadata(&secret_path)?.permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&secret_path, perms)?;

            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].files.len(), 2);
        }

        Ok(())
    }

    #[test]
    fn tokenize_tracks_string_start_line() {
        let text = "let a = \"x\ny\";\nlet b = 1;\n";
        let tokens = tokenize_for_dup_detection(text);

        let str_idx = tokens
            .tokens
            .iter()
            .position(|&tok| tok == 3)
            .expect("should contain TOK_STR");
        assert_eq!(tokens.token_lines[str_idx], 1);

        let semi_idx = tokens
            .tokens
            .iter()
            .position(|&tok| tok == 10_000 + u32::from(b';'))
            .expect("should contain ';' token");
        assert_eq!(tokens.token_lines[semi_idx], 2);

        let let_positions: Vec<usize> = tokens
            .tokens
            .iter()
            .enumerate()
            .filter_map(|(i, &tok)| (tok == 122).then_some(i))
            .collect();
        assert!(let_positions.len() >= 2);
        assert_eq!(tokens.token_lines[let_positions[1]], 3);
    }

    fn temp_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("code-checker-core-{suffix}-{nanos}"))
    }
}
