mod detect;
mod scan_files;
mod util;

#[cfg(test)]
mod tests;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::scan::validate_roots;
use crate::tokenize::BlockNode;
use crate::types::{DuplicationReport, ScanOptions, ScanOutcome, ScanStats};

#[derive(Debug)]
struct ScannedTextFile {
    repo_id: usize,
    path: Arc<str>,
    abs_path: PathBuf,
    code_chars: Vec<u8>,
    code_line_starts: Vec<u32>,
    line_tokens: Vec<u32>,
    line_token_lines: Vec<u32>,
    line_token_char_lens: Vec<usize>,
    tokens: Vec<u32>,
    token_lines: Vec<u32>,
    blocks: Vec<BlockNode>,
}

fn empty_report() -> DuplicationReport {
    DuplicationReport {
        file_duplicates: Vec::new(),
        code_span_duplicates: Vec::new(),
        line_span_duplicates: Vec::new(),
        token_span_duplicates: Vec::new(),
        block_duplicates: Vec::new(),
        ast_subtree_duplicates: Vec::new(),
        similar_blocks_minhash: Vec::new(),
        similar_blocks_simhash: Vec::new(),
    }
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
            result: empty_report(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;
    if options.max_report_items == 0 {
        return Ok(ScanOutcome {
            result: empty_report(),
            stats: ScanStats::default(),
        });
    }
    options.validate_for_report()?;

    let mut stats = ScanStats::default();
    let (repo_labels, files, file_duplicates) =
        scan_files::scan_text_files_for_report(roots, options, &mut stats)?;

    let code_span_duplicates =
        detect::detect_duplicate_code_spans(&repo_labels, &files, options, &mut stats);
    let line_span_duplicates =
        detect::detect_duplicate_line_spans(&repo_labels, &files, options, &mut stats);
    let token_span_duplicates =
        detect::detect_duplicate_token_spans(&repo_labels, &files, options, &mut stats);
    let block_duplicates = detect::detect_duplicate_blocks(&repo_labels, &files, options);
    let ast_subtree_duplicates =
        detect::detect_duplicate_ast_subtrees(&repo_labels, &files, options);
    let similar_blocks_minhash = detect::find_similar_blocks_minhash(&repo_labels, &files, options);
    let similar_blocks_simhash = detect::find_similar_blocks_simhash(&repo_labels, &files, options);

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
