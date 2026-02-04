use super::*;

use std::ffi::{OsStr, OsString};
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use std::fs;

#[test]
fn safe_relative_path_rejects_unsafe_paths() {
    assert!(is_safe_relative_path("a.txt"));
    assert!(is_safe_relative_path("dir/file.txt"));

    assert!(!is_safe_relative_path(""));
    assert!(!is_safe_relative_path("../a.txt"));
    assert!(!is_safe_relative_path("a/../b.txt"));
    assert!(!is_safe_relative_path("./a.txt"));

    #[cfg(unix)]
    assert!(!is_safe_relative_path("/etc/passwd"));

    #[cfg(windows)]
    assert!(!is_safe_relative_path("C:\\\\Windows\\\\System32"));
}

#[test]
fn read_repo_file_bytes_enforces_max_file_size_during_read() -> io::Result<()> {
    let root = temp_dir("read_repo_file_bytes_enforces_max_file_size_during_read");
    fs::create_dir_all(&root)?;

    let path = root.join("a.txt");
    fs::write(&path, b"aaaaaaaaaa")?;

    let repo_file = RepoFile {
        repo_id: 0,
        repo_label: "test".to_string(),
        root: root.clone(),
        abs_path: path.clone(),
    };

    let options = ScanOptions {
        max_file_size: Some(20),
        ..ScanOptions::default()
    };

    let mut stats = ScanStats::default();
    let out = read::with_test_before_open_hook(
        |read_path| {
            use std::io::Write;

            let mut file = fs::OpenOptions::new().append(true).open(read_path).unwrap();
            file.write_all(&[b'a'; 32]).unwrap();
        },
        || read_repo_file_bytes_with_path(&repo_file, None, &options, &mut stats),
    )?;

    assert!(out.is_none());
    assert_eq!(stats.skipped_too_large, 1);
    assert_eq!(stats.scanned_files, 1);
    assert_eq!(stats.scanned_bytes, 21);

    Ok(())
}

#[test]
fn read_repo_file_bytes_enforces_max_total_bytes_during_read() -> io::Result<()> {
    let root = temp_dir("read_repo_file_bytes_enforces_max_total_bytes_during_read");
    fs::create_dir_all(&root)?;

    let path = root.join("a.txt");
    fs::write(&path, b"aaaaaaaaaa")?;

    let repo_file = RepoFile {
        repo_id: 0,
        repo_label: "test".to_string(),
        root: root.clone(),
        abs_path: path.clone(),
    };

    let options = ScanOptions {
        max_total_bytes: Some(25),
        ..ScanOptions::default()
    };

    let mut stats = ScanStats::default();
    let out = read::with_test_before_open_hook(
        |read_path| {
            use std::io::Write;

            let mut file = fs::OpenOptions::new().append(true).open(read_path).unwrap();
            file.write_all(&[b'a'; 64]).unwrap();
        },
        || read_repo_file_bytes_with_path(&repo_file, None, &options, &mut stats),
    )?;

    assert!(out.is_none());
    assert_eq!(stats.skipped_budget_max_total_bytes, 1);
    assert_eq!(stats.scanned_files, 1);
    assert_eq!(stats.scanned_bytes, 25);

    Ok(())
}

#[test]
fn git_streaming_check_ignore_failure_falls_back_to_walker_without_double_scan() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::collections::HashSet;
        use std::os::unix::fs::PermissionsExt;

        const FILES: usize = 600;

        let root = temp_dir("git_streaming_check_ignore_failure");
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir)?;

        let mut rels = Vec::with_capacity(FILES);
        for idx in 0..FILES {
            let name = format!("f{idx:04}.txt");
            fs::write(root.join(&name), "x")?;
            rels.push(name);
        }

        let list_path = git_dir.join("filelist.txt");
        fs::write(&list_path, rels.join("\n"))?;
        let count_path = git_dir.join("check_ignore_count.txt");

        let fake_git_path = git_dir.join("fake_git.sh");
        fs::write(
            &fake_git_path,
            fake_git_script(&root, &list_path, &count_path),
        )?;
        let mut perms = fs::metadata(&fake_git_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_git_path, perms)?;

        let repo = Repo {
            id: 0,
            root: root.clone(),
            label: "test".to_string(),
        };
        let options = ScanOptions {
            max_files: Some(FILES + 10),
            ..ScanOptions::default()
        };

        let mut stats = ScanStats::default();
        let mut visited: Vec<String> = Vec::new();
        let flow = git::with_test_git_exe(&fake_git_path, || {
            visit_repo_files(&repo, &options, &mut stats, |stats, file| {
                stats.scanned_files = stats.scanned_files.saturating_add(1);
                visited.push(make_rel_path(&root, &file.abs_path));
                Ok(ControlFlow::Continue(()))
            })
        })?;

        assert_eq!(flow, ControlFlow::Continue(()));
        assert_eq!(visited.len(), FILES);
        let unique: HashSet<&str> = visited.iter().map(String::as_str).collect();
        assert_eq!(unique.len(), FILES);
        assert_eq!(stats.candidate_files, FILES as u64);
        assert_eq!(stats.skipped_walk_errors, 1);

        // 1st batch: check-ignore -> exit 1 (no ignores), 2nd batch: exit 2 (failure).
        // After the failure we should fall back to the walker (and avoid calling check-ignore again).
        let count: usize = fs::read_to_string(&count_path)
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);
        assert_eq!(count, 2);
    }

    Ok(())
}

#[test]
fn git_bin_override_validation_is_restrictive() -> io::Result<()> {
    assert_eq!(git::validate_git_bin_override(OsString::from("git")), None);
    assert_eq!(git::validate_git_bin_override(OsString::from("git/")), None);
    assert_eq!(
        git::validate_git_bin_override(OsString::from("bin/git")),
        None
    );
    assert_eq!(
        git::validate_git_bin_override(OsString::from("git --version")),
        None
    );
    assert_eq!(git::validate_git_bin_override(OsString::from("")), None);

    let root = temp_dir("git_bin_override_validation");
    fs::create_dir_all(&root)?;
    let missing = root.join("missing_git");
    assert_eq!(
        git::validate_git_bin_override(missing.as_os_str().to_os_string()),
        None
    );

    let existing = root.join("git");
    fs::write(&existing, "")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(&existing)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&existing, perms)?;
    }
    assert_eq!(
        git::validate_git_bin_override(existing.as_os_str().to_os_string()),
        Some(existing.as_os_str().to_os_string())
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let symlink_path = root.join("git_symlink");
        symlink(&existing, &symlink_path)?;
        assert_eq!(
            git::validate_git_bin_override(symlink_path.as_os_str().to_os_string()),
            None
        );

        let writable = root.join("git_writable");
        fs::write(&writable, "")?;
        let mut perms = fs::metadata(&writable)?.permissions();
        perms.set_mode(0o777);
        fs::set_permissions(&writable, perms)?;
        assert_eq!(
            git::validate_git_bin_override(writable.as_os_str().to_os_string()),
            None
        );
    }

    Ok(())
}

#[test]
fn git_bin_override_requires_opt_in() -> io::Result<()> {
    let root = temp_dir("git_bin_override_opt_in");
    fs::create_dir_all(&root)?;

    let existing = root.join("git");
    fs::write(&existing, "")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(&existing)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&existing, perms)?;
    }
    let existing = existing.as_os_str().to_os_string();

    // Without explicit opt-in, ignore the override even if it points to an existing file.
    assert_eq!(
        git::git_bin_override_from_env(false, Some(existing.clone())),
        None
    );

    // Opt-in is strict: only `DUP_CODE_CHECK_ALLOW_CUSTOM_GIT=1`.
    assert!(git::allow_custom_git_override(Some(OsStr::new("1"))));
    assert!(!git::allow_custom_git_override(Some(OsStr::new("true"))));
    assert!(!git::allow_custom_git_override(Some(OsStr::new("0"))));
    assert!(!git::allow_custom_git_override(None));

    // With opt-in, accept a valid absolute file path.
    assert_eq!(
        git::git_bin_override_from_env(true, Some(existing.clone())),
        Some(existing)
    );

    // With opt-in, still reject invalid overrides.
    assert_eq!(
        git::git_bin_override_from_env(true, Some(OsString::from("git"))),
        None
    );

    Ok(())
}

#[test]
fn git_streaming_non_utf8_path_falls_back_to_walker_before_scanning() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir("git_streaming_non_utf8_fallback");
        fs::create_dir_all(root.join(".git"))?;

        fs::write(root.join("a.txt"), "x")?;

        let marker_path = root.join(".git").join("git_called");
        let fake_git_path = root.join(".git").join("fake_git.sh");
        fs::write(
            &fake_git_path,
            fake_git_script_non_utf8(&root, &marker_path),
        )?;
        let mut perms = fs::metadata(&fake_git_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_git_path, perms)?;

        let repo = Repo {
            id: 0,
            root: root.clone(),
            label: "test".to_string(),
        };
        let options = ScanOptions {
            max_files: Some(10),
            ..ScanOptions::default()
        };

        let mut stats = ScanStats::default();
        let mut visited: Vec<String> = Vec::new();
        let flow = git::with_test_git_exe(&fake_git_path, || {
            visit_repo_files(&repo, &options, &mut stats, |_stats, file| {
                visited.push(make_rel_path(&root, &file.abs_path));
                Ok(ControlFlow::Continue(()))
            })
        })?;

        assert_eq!(flow, ControlFlow::Continue(()));
        assert!(visited.iter().any(|p| p == "a.txt"));
        assert!(marker_path.exists());
        assert_eq!(stats.skipped_walk_errors, 0);
    }

    Ok(())
}

#[test]
fn git_streaming_non_utf8_path_falls_back_to_walker_after_scanning_started() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        const FILES: usize = 600;

        let root = temp_dir("git_streaming_non_utf8_mid_fallback");
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir)?;

        for idx in 0..FILES {
            let name = format!("f{idx:04}.txt");
            fs::write(root.join(&name), "x")?;
        }

        let count_path = git_dir.join("check_ignore_count.txt");
        let fake_git_path = git_dir.join("fake_git.sh");
        fs::write(
            &fake_git_path,
            fake_git_script_non_utf8_after_started(&root, &count_path, FILES),
        )?;
        let mut perms = fs::metadata(&fake_git_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_git_path, perms)?;

        let repo = Repo {
            id: 0,
            root: root.clone(),
            label: "test".to_string(),
        };
        let options = ScanOptions {
            max_files: Some(FILES + 10),
            ..ScanOptions::default()
        };

        let mut stats = ScanStats::default();
        let mut visited: Vec<String> = Vec::new();
        let flow = git::with_test_git_exe(&fake_git_path, || {
            visit_repo_files(&repo, &options, &mut stats, |stats, file| {
                stats.scanned_files = stats.scanned_files.saturating_add(1);
                visited.push(make_rel_path(&root, &file.abs_path));
                Ok(ControlFlow::Continue(()))
            })
        })?;

        assert_eq!(flow, ControlFlow::Continue(()));
        assert_eq!(stats.skipped_walk_errors, 1);
        assert_eq!(stats.candidate_files, FILES as u64);
        assert_eq!(visited.len(), FILES);

        let count: usize = fs::read_to_string(&count_path)
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);
        assert_eq!(count, 1);
    }

    Ok(())
}

#[test]
fn read_repo_file_bytes_counts_binary_reads_in_scan_stats() -> io::Result<()> {
    let root = temp_dir("read_repo_file_bytes_binary_counts");
    fs::create_dir_all(&root)?;
    let path = root.join("bin.dat");
    fs::write(&path, b"hello\0world")?;

    let repo_file = RepoFile {
        repo_id: 0,
        repo_label: "test".to_string(),
        root: root.clone(),
        abs_path: path,
    };

    let options = ScanOptions::default();
    let mut stats = ScanStats::default();
    let out = read_repo_file_bytes(&repo_file, None, &options, &mut stats)?;

    assert!(out.is_none());
    assert_eq!(stats.skipped_binary, 1);
    assert_eq!(stats.scanned_files, 1);
    assert!(stats.scanned_bytes > 0);

    Ok(())
}

fn fake_git_script(repo: &Path, list: &Path, count_file: &Path) -> String {
    let repo = sh_single_quote(repo.to_string_lossy().as_ref());
    let list = sh_single_quote(list.to_string_lossy().as_ref());
    let count_file = sh_single_quote(count_file.to_string_lossy().as_ref());
    format!(
        r#"#!/bin/sh
set -eu

repo=""
if [ "${{1:-}}" = "-C" ]; then
  repo="${{2:-}}"
  shift 2
fi

cmd="${{1:-}}"
if [ -z "$cmd" ]; then
  exit 2
fi
shift

target_repo={repo}
file_list={list}
count_file={count_file}

if [ "$repo" = "$target_repo" ]; then
  case "$cmd" in
    ls-files)
      while IFS= read -r line || [ -n "$line" ]; do
        printf '%s\0' "$line"
      done < "$file_list"
      exit 0
      ;;
    check-ignore)
      n=0
      if [ -f "$count_file" ]; then
        n="$(cat "$count_file" 2>/dev/null || true)"
      fi
      case "$n" in
        ''|*[!0-9]*) n=0 ;;
      esac
      n=$((n+1))
      echo "$n" > "$count_file"
      cat >/dev/null
      if [ "$n" -ge 2 ]; then
        exit 2
      fi
      exit 1
      ;;
  esac
fi

if [ -n "$repo" ]; then
  exec git -C "$repo" "$cmd" "$@"
fi
exec git "$cmd" "$@"
"#
    )
}

fn fake_git_script_non_utf8(repo: &Path, marker: &Path) -> String {
    let repo = sh_single_quote(repo.to_string_lossy().as_ref());
    let marker = sh_single_quote(marker.to_string_lossy().as_ref());
    format!(
        r#"#!/bin/sh
set -eu

repo=""
if [ "${{1:-}}" = "-C" ]; then
  repo="${{2:-}}"
  shift 2
fi

cmd="${{1:-}}"
if [ -z "$cmd" ]; then
  exit 2
fi
shift

target_repo={repo}
marker={marker}

if [ "$repo" = "$target_repo" ]; then
  case "$cmd" in
    ls-files)
      : > "$marker"
      # Output a non-UTF-8 path to force the streaming scanner to fall back to the walker.
      printf '\377\0'
      exit 0
      ;;
  esac
fi

exit 2
"#
    )
}

fn fake_git_script_non_utf8_after_started(repo: &Path, count_file: &Path, files: usize) -> String {
    let repo = sh_single_quote(repo.to_string_lossy().as_ref());
    let count_file = sh_single_quote(count_file.to_string_lossy().as_ref());
    format!(
        r#"#!/bin/sh
set -eu

repo=""
if [ "${{1:-}}" = "-C" ]; then
  repo="${{2:-}}"
  shift 2
fi

cmd="${{1:-}}"
if [ -z "$cmd" ]; then
  exit 2
fi
shift

target_repo={repo}
count_file={count_file}
files={files}

if [ "$repo" = "$target_repo" ]; then
  case "$cmd" in
    ls-files)
      i=0
      while [ "$i" -lt "$files" ]; do
        if [ "$i" -eq 300 ]; then
          printf '\377.txt\0'
        fi
        printf 'f%04d.txt\0' "$i"
        i=$((i+1))
      done
      exit 0
      ;;
    check-ignore)
      n=0
      if [ -f "$count_file" ]; then
        n="$(cat "$count_file" 2>/dev/null || true)"
      fi
      case "$n" in
        ''|*[!0-9]*) n=0 ;;
      esac
      n=$((n+1))
      echo "$n" > "$count_file"
      cat >/dev/null
      exit 1
      ;;
  esac
fi

exit 2
"#
    )
}

fn sh_single_quote(s: &str) -> String {
    let escaped = s.replace('\'', r#"'"'"'"#);
    format!("'{escaped}'")
}

fn temp_dir(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    std::env::temp_dir().join(format!("dup-code-check-core-{suffix}-{nanos}"))
}
