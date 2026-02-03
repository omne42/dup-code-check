use std::collections::HashSet;

use crate::types::DuplicateSpanOccurrence;

#[derive(Debug)]
pub(crate) struct NormalizedFile {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: String,
    pub(crate) rel_path: String,
    pub(crate) normalized: Vec<u32>,
    pub(crate) line_map: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalizedFileView<'a> {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: &'a str,
    pub(crate) rel_path: &'a str,
    pub(crate) normalized: &'a [u32],
    pub(crate) line_map: &'a [u32],
}

#[derive(Debug)]
pub(crate) struct SpanGroupBuilder {
    pub(crate) content_hash: u64,
    pub(crate) normalized_len: usize,
    pub(crate) sample: Vec<u32>,
    pub(crate) preview: String,
    pub(crate) occurrences: Vec<DuplicateSpanOccurrence>,
    pub(crate) occurrence_keys: HashSet<(usize, usize)>,
    pub(crate) repo_ids: HashSet<usize>,
}

#[derive(Debug)]
pub(crate) struct NormalizedText {
    pub(crate) chars: Vec<u32>,
    pub(crate) line_map: Vec<u32>,
}

pub(crate) fn normalize_whitespace(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if !b.is_ascii_whitespace() {
            out.push(b);
        }
    }
    out
}

pub(crate) fn normalize_for_code_spans(bytes: &[u8]) -> NormalizedText {
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

pub(crate) fn fold_u64_to_u32(value: u64) -> u32 {
    (value as u32) ^ ((value >> 32) as u32)
}

pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub(crate) fn fnv1a64_u32(codepoints: &[u32]) -> u64 {
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

pub(crate) fn winnowed_fingerprints(
    chars: &[u32],
    k: usize,
    window_size: usize,
) -> Vec<(u64, usize)> {
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

pub(crate) fn maximal_match(
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

pub(crate) fn canonicalize_match(
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

pub(crate) fn add_occurrence_view(
    builder: &mut SpanGroupBuilder,
    file: &NormalizedFileView<'_>,
    file_id: usize,
    start: usize,
    len: usize,
) {
    if len == 0 {
        return;
    }
    if !builder.occurrence_keys.insert((file_id, start)) {
        return;
    }

    debug_assert_eq!(
        file.line_map.len(),
        file.normalized.len(),
        "line_map and normalized must have the same length"
    );
    let start_line = file.line_map.get(start).copied().unwrap_or(1);
    let end_line = file
        .line_map
        .get(start + len - 1)
        .copied()
        .unwrap_or(start_line);

    builder.repo_ids.insert(file.repo_id);
    builder.occurrences.push(DuplicateSpanOccurrence {
        repo_id: file.repo_id,
        repo_label: file.repo_label.to_string(),
        path: file.rel_path.to_string(),
        start_line,
        end_line,
    });
}

pub(crate) fn make_preview(codepoints: &[u32], max_len: usize) -> String {
    codepoints
        .iter()
        .take(max_len)
        .filter_map(|&cp| char::from_u32(cp))
        .collect()
}
