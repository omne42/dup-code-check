use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::tokenize::tokenize_for_dup_detection;
use crate::util::{normalize_for_code_spans, normalize_whitespace};
use crate::{DEFAULT_MAX_FILE_SIZE_BYTES, find_duplicate_code_spans, find_duplicate_files};

#[test]
fn normalize_whitespace_removes_ascii_whitespace() {
    let input = b"a 
	b
c";
    assert_eq!(normalize_whitespace(input), b"abc");
}

#[test]
fn finds_duplicates_within_single_repo() -> io::Result<()> {
    let root = temp_dir("single");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("a.txt"),
        "a b
c",
    )?;
    fs::write(root.join("b.txt"), "ab	c")?;
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

    fs::write(
        repo_a.join("same.txt"),
        "a b
c",
    )?;
    fs::write(repo_b.join("same.txt"), "ab	c")?;
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
    let input = b"a + b
_c
123";
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

    fs::write(
        repo_a.join("a.txt"),
        format!(
            "////
P{snippet}Q
"
        ),
    )?;
    fs::write(
        repo_b.join("b.txt"),
        format!(
            "####
R{snippet}S
"
        ),
    )?;

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
    fs::write(
        root.join(".gitignore"),
        "ignored.txt
",
    )?;
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
    fs::write(
        sub.join(".gitignore"),
        "ignored.txt
",
    )?;
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
    fs::write(
        root.join(".gitignore"),
        "ignored.txt
",
    )?;
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
        "////
function f(x) { return x + 1; }
",
    )?;
    fs::write(
        repo_b.join("b.js"),
        "####
function g(y) { return y + 1; }
",
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

        fs::write(
            root.join("a.txt"),
            "a b
c",
        )?;
        fs::write(root.join("b.txt"), "ab	c")?;
        symlink("a.txt", root.join("link.txt"))?;

        let options_no = ScanOptions::default();
        let groups_no = find_duplicate_files(std::slice::from_ref(&root), &options_no)?;
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
fn git_fast_path_still_used_with_budgets() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Stdio;

        struct RestorePerm {
            path: PathBuf,
        }

        impl Drop for RestorePerm {
            fn drop(&mut self) {
                let perms = fs::Permissions::from_mode(0o755);
                let _ = fs::set_permissions(&self.path, perms);
            }
        }

        let root = temp_dir("git_fast_path_budgets");
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

        fs::write(
            root.join("a.txt"),
            "a b
c",
        )?;
        fs::write(root.join("b.txt"), "ab	c")?;

        // Make an unreadable directory; `git ls-files --others` prints a warning but still exits 0.
        // This makes the walk-based scanner accumulate PermissionDenied, while the git fast path doesn't.
        let secret_dir = root.join("secret_dir");
        fs::create_dir_all(&secret_dir)?;
        let mut perms = fs::metadata(&secret_dir)?.permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&secret_dir, perms)?;
        let _guard = RestorePerm {
            path: secret_dir.clone(),
        };

        // `maxFiles`: once limit is hit, remaining candidates are skipped.
        let options_files = ScanOptions {
            max_files: Some(1),
            ..ScanOptions::default()
        };
        let outcome_files =
            crate::find_duplicate_files_with_stats(std::slice::from_ref(&root), &options_files)?;
        assert_eq!(outcome_files.stats.skipped_permission_denied, 0);
        assert!(outcome_files.stats.skipped_budget_max_files > 0);

        // `maxTotalBytes`: files that would exceed the budget are skipped.
        let options_bytes = ScanOptions {
            max_total_bytes: Some(1),
            ..ScanOptions::default()
        };
        let outcome_bytes =
            crate::find_duplicate_files_with_stats(std::slice::from_ref(&root), &options_bytes)?;
        assert_eq!(outcome_bytes.stats.skipped_permission_denied, 0);
        assert!(outcome_bytes.stats.skipped_budget_max_total_bytes > 0);
    }

    Ok(())
}

#[test]
fn follow_symlinks_does_not_escape_root() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink_escape_root");
        let external = temp_dir("symlink_escape_external");
        fs::create_dir_all(&root)?;
        fs::create_dir_all(&external)?;

        fs::write(root.join("a.txt"), "same")?;
        fs::write(external.join("b.txt"), "same")?;
        symlink(&external, root.join("ext"))?;

        let options = ScanOptions {
            follow_symlinks: true,
            ..ScanOptions::default()
        };
        let outcome = crate::find_duplicate_files_with_stats(&[root], &options)?;
        assert_eq!(outcome.result.len(), 0);
        assert_eq!(outcome.stats.skipped_outside_root, 1);
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
        fs::write(
            root.join("a.txt"),
            "a b
c",
        )?;
        fs::write(root.join("b.txt"), "ab	c")?;

        let secret_path = root.join("secret.txt");
        fs::write(&secret_path, "ab	c")?;

        let mut perms = fs::metadata(&secret_path)?.permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&secret_path, perms)?;

        let options = ScanOptions::default();
        let groups = find_duplicate_files(std::slice::from_ref(&root), &options)?;

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
    std::env::temp_dir().join(format!("dup-code-check-core-{suffix}-{nanos}"))
}
