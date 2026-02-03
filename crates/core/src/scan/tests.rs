use super::*;

use std::ffi::{OsStr, OsString};
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use std::fs;

#[test]
fn git_streaming_check_ignore_failure_degrades_without_double_scan() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::collections::HashSet;
        use std::os::unix::fs::PermissionsExt;

        const BATCH_SIZE: usize = 256;
        const FILES: usize = 600;

        let root = temp_dir("git_streaming_check_ignore_failure");
        fs::create_dir_all(root.join(".git"))?;

        let mut rels = Vec::with_capacity(FILES);
        for idx in 0..FILES {
            let name = format!("f{idx:04}.txt");
            fs::write(root.join(&name), "x")?;
            rels.push(name);
        }

        let list_path = root.join("filelist.txt");
        fs::write(&list_path, rels.join("\n"))?;
        let count_path = root.join("check_ignore_count.txt");

        let fake_git_path = root.join("fake_git.sh");
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

        assert_eq!(flow, ControlFlow::Break(()));
        assert_eq!(visited.len(), BATCH_SIZE);
        let unique: HashSet<&str> = visited.iter().map(String::as_str).collect();
        assert_eq!(unique.len(), BATCH_SIZE);
        assert_eq!(stats.candidate_files, BATCH_SIZE as u64);
        assert_eq!(stats.skipped_walk_errors, 1);

        // 1st batch: check-ignore -> exit 1 (no ignores), 2nd batch: exit 2 (failure).
        // After the failure we should stop scanning (and avoid calling check-ignore again).
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
    assert_eq!(
        git::validate_git_bin_override(existing.as_os_str().to_os_string()),
        Some(existing.as_os_str().to_os_string())
    );

    Ok(())
}

#[test]
fn git_bin_override_requires_opt_in() -> io::Result<()> {
    let root = temp_dir("git_bin_override_opt_in");
    fs::create_dir_all(&root)?;

    let existing = root.join("git");
    fs::write(&existing, "")?;
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
