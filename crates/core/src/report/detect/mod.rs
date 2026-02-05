mod blocks;
mod code_spans;
mod line_spans;
mod similarity;
mod span_groups;
mod token_spans;

use std::sync::Arc;

pub(super) use blocks::{detect_duplicate_ast_subtrees, detect_duplicate_blocks};
pub(super) use code_spans::detect_duplicate_code_spans;
pub(super) use line_spans::detect_duplicate_line_spans;
pub(super) use similarity::{find_similar_blocks_minhash, find_similar_blocks_simhash};
pub(super) use token_spans::detect_duplicate_token_spans;

fn repo_label_arc(repo_labels: &[Arc<str>], repo_id: usize) -> Arc<str> {
    Arc::clone(&repo_labels[repo_id])
}
