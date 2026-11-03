use git2::Repository;
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Serialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

/// A `git` command running in the working tree. Paths are taken literally, so
/// names containing `*`, `?` or `[` aren't treated as patterns.
fn git_command(repo: &Repository) -> Result<Command, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("Repository has no working directory"))?;
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir).env("GIT_LITERAL_PATHSPECS", "1");
    Ok(cmd)
}

fn run_git(mut cmd: Command, what: &str) -> Result<(), git2::Error> {
    let output = cmd
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run {what}: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(git2::Error::from_str(&format!(
        "{what} failed: {}",
        stderr.trim()
    )))
}

fn is_unborn(repo: &Repository) -> bool {
    matches!(repo.head(), Err(e) if e.code() == git2::ErrorCode::UnbornBranch)
}

pub fn get_status(repo: &Repository) -> Result<Vec<GitFileStatus>, git2::Error> {
    let mut files = Vec::new();
    let statuses = repo.statuses(None)?;

    for entry in statuses.iter() {
        let Ok(path) = entry.path() else {
            continue;
        };
        let status = entry.status();

        // Unmerged paths (merge, cherry-pick, stash apply...) carry no
        // index/worktree flags, so they have to be picked out first.
        if status.is_conflicted() {
            files.push(GitFileStatus {
                path: path.to_string(),
                status: "conflicted".to_string(),
                staged: false,
            });
            continue;
        }

        // Staged: the index differs from HEAD.
        if status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange()
        {
            files.push(GitFileStatus {
                path: path.to_string(),
                status: format_status(status, true).to_string(),
                staged: true,
            });
        }

        // Unstaged: the working tree differs from the index.
        if status.is_wt_new()
            || status.is_wt_modified()
            || status.is_wt_deleted()
            || status.is_wt_renamed()
            || status.is_wt_typechange()
        {
            files.push(GitFileStatus {
                path: path.to_string(),
                status: format_status(status, false).to_string(),
                staged: false,
            });
        }
    }

    Ok(files)
}

/// Label for one half of a file's status. A file can be changed in both the
/// index and the working tree (`MD`, `AM`...), so each half is labelled from
/// its own flags rather than from the combination.
fn format_status(status: git2::Status, staged: bool) -> &'static str {
    if staged {
        if status.is_index_new() {
            "added"
        } else if status.is_index_modified() {
            "modified"
        } else if status.is_index_deleted() {
            "deleted"
        } else if status.is_index_renamed() {
            "renamed"
        } else if status.is_index_typechange() {
            "typechange"
        } else {
            "unknown"
        }
    } else if status.is_wt_new() {
        "added"
    } else if status.is_wt_modified() {
        "modified"
    } else if status.is_wt_deleted() {
        "deleted"
    } else if status.is_wt_renamed() {
        "renamed"
    } else if status.is_wt_typechange() {
        "typechange"
    } else {
        "unknown"
    }
}

/// A `git diff`-style patch for a file that git doesn't know yet, complete
/// with the header `git apply` needs to stage it hunk by hunk.
fn untracked_file_diff(workdir: &Path, path: &str) -> Option<String> {
    let full_path = workdir.join(path);
    let bytes = std::fs::read(&full_path).ok()?;
    if bytes.contains(&0) {
        // Binary: there's no meaningful text diff.
        return Some(String::new());
    }
    let content = String::from_utf8_lossy(&bytes);

    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        let executable = std::fs::metadata(&full_path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if executable {
            "100755"
        } else {
            "100644"
        }
    };
    #[cfg(not(unix))]
    let mode = "100644";

    let mut diff_text = format!(
        "diff --git a/{path} b/{path}\nnew file mode {mode}\n--- /dev/null\n+++ b/{path}\n"
    );
    // Split on '\n' only, so CRLF files keep their '\r' when staged.
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    if lines.is_empty() {
        return Some(diff_text);
    }
    diff_text.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
    for line in &lines {
        diff_text.push('+');
        diff_text.push_str(line);
    }
    if !content.ends_with('\n') {
        diff_text.push_str("\n\\ No newline at end of file\n");
    }
    Some(diff_text)
}

pub fn get_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    // Untracked files have no diff in git's eyes, and a conflicted file's diff
    // is meaningless (a mode change). Show both as the file's current content;
    // for a conflict that means its `<<<<<<<` markers.
    if !staged {
        let needs_content_view = repo
            .status_file(Path::new(path))
            .map(|s| s.is_wt_new() || s.is_conflicted())
            .unwrap_or(false);
        if needs_content_view {
            let workdir = repo
                .workdir()
                .ok_or_else(|| git2::Error::from_str("Repository has no working directory"))?;
            if let Some(diff_text) = untracked_file_diff(workdir, path) {
                return Ok(diff_text);
            }
        }
    }

    let mut diff_options = git2::DiffOptions::new();
    diff_options.pathspec(path);

    let diff = if staged {
        // Before the first commit there is no HEAD tree: everything staged is new.
        let head_tree = match repo.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e),
        };
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut diff_options))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut diff_options))?
    };

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        let content = String::from_utf8_lossy(line.content());

        match origin {
            '+' | '-' | ' ' => {
                diff_text.push(origin);
                diff_text.push_str(&content);
            }
            _ => {
                // For other lines (headers, etc), include as-is
                diff_text.push_str(&content);
            }
        }
        true
    })?;

    Ok(diff_text)
}

/// The `.gitignore` line that ignores exactly `file_path`: anchored to the
/// repo root, with glob characters escaped.
fn gitignore_pattern(file_path: &str) -> String {
    let mut pattern = String::from("/");
    for c in file_path.chars() {
        if matches!(c, '*' | '?' | '[' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    // git drops unescaped trailing spaces.
    if pattern.ends_with(' ') {
        pattern.pop();
        pattern.push_str("\\ ");
    }
    pattern
}

pub fn ignore_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("Repository has no working directory"))?;

    // .gitignore has no effect on a file git already tracks.
    if repo.index()?.get_path(Path::new(file_path), 0).is_some() {
        return Err(git2::Error::from_str(&format!(
            "{file_path} is already tracked, so ignoring it has no effect. \
             Run `git rm --cached` on it first."
        )));
    }

    let gitignore_path = workdir.join(".gitignore");
    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)
            .map_err(|e| git2::Error::from_str(&format!("Failed to read .gitignore: {}", e)))?
    } else {
        String::new()
    };

    let pattern = gitignore_pattern(file_path);
    if content.lines().any(|line| line == pattern) {
        return Ok(());
    }

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&pattern);
    content.push('\n');

    std::fs::write(&gitignore_path, content)
        .map_err(|e| git2::Error::from_str(&format!("Failed to write .gitignore: {}", e)))?;

    Ok(())
}

pub fn stage_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let path = Path::new(file_path);
    let on_disk = repo
        .workdir()
        .map(|w| w.join(path).symlink_metadata().is_ok())
        .unwrap_or(true);
    if on_disk {
        index.add_path(path)?;
    } else {
        // Staging a deleted file means removing it from the index.
        index.remove_path(path)?;
    }
    index.write()?;
    Ok(())
}

/// Everything in the diff before its first hunk: the `diff --git`, mode,
/// `index` and `---`/`+++` lines. Their number varies (new, deleted and
/// mode-changed files have extra lines), so don't assume a fixed count.
fn diff_header(full_diff: &str) -> String {
    full_diff
        .split('\n')
        .take_while(|l| !l.starts_with("@@"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `file_path`'s current diff still contains exactly this hunk. The
/// diff shown in the UI can be older than the file on disk; applying a stale
/// hunk would stage or discard something other than what the user is looking at.
fn ensure_hunk_is_current(
    repo: &Repository,
    file_path: &str,
    staged: bool,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    let current = get_diff(repo, file_path, staged)?;
    // Split on '\n' only, as the UI does: `lines()` would drop the '\r' of CRLF files.
    let mut lines = current.split('\n');
    let found = lines.any(|l| l == hunk_header);
    let matches = found && {
        let block: Vec<&str> = lines.take_while(|l| !l.starts_with("@@")).collect();
        block.join("\n").trim_end_matches('\n') == hunk_lines.trim_end_matches('\n')
    };
    if matches {
        Ok(())
    } else {
        Err(git2::Error::from_str(
            "The file changed since this diff was loaded. The diff has been refreshed; \
             please review it and try again.",
        ))
    }
}

/// Applies one hunk with `git apply`. `args` decides where it goes (index,
/// working tree) and in which direction.
fn apply_hunk(
    repo: &Repository,
    args: &[&str],
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    // Create patch: header + hunk header + hunk lines
    let patch = format!(
        "{}\n{}\n{}\n",
        diff_header(full_diff),
        hunk_header,
        hunk_lines
    );

    let mut cmd = git_command(repo)?;
    let output = cmd
        .arg("apply")
        .args(["--unidiff-zero", "--ignore-whitespace"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(patch.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git apply: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(&format!(
            "git apply failed: {}",
            stderr
        )));
    }

    Ok(())
}

pub fn stage_hunk(
    repo: &Repository,
    file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    ensure_hunk_is_current(repo, file_path, false, hunk_header, hunk_lines)?;
    apply_hunk(repo, &["--cached"], full_diff, hunk_header, hunk_lines)
}

pub fn unstage_hunk(
    repo: &Repository,
    file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    ensure_hunk_is_current(repo, file_path, true, hunk_header, hunk_lines)?;
    apply_hunk(
        repo,
        &["--cached", "--reverse"],
        full_diff,
        hunk_header,
        hunk_lines,
    )
}

pub fn discard_hunk(
    repo: &Repository,
    file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    ensure_hunk_is_current(repo, file_path, false, hunk_header, hunk_lines)?;
    apply_hunk(repo, &["--reverse"], full_diff, hunk_header, hunk_lines)
}

pub fn unstage_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let mut cmd = git_command(repo)?;
    if is_unborn(repo) {
        // No HEAD to reset to yet: unstaging means dropping the index entry.
        cmd.args(["rm", "--cached", "-q", "--", file_path]);
    } else {
        cmd.args(["reset", "-q", "HEAD", "--", file_path]);
    }
    run_git(cmd, "git unstage")
}

pub fn discard_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let mut cmd = git_command(repo)?;
    // Restores the file from the index.
    cmd.args(["checkout", "--", file_path]);
    run_git(cmd, "git checkout")
}
