use git2::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

pub fn get_status(repo: &Repository) -> Result<Vec<GitFileStatus>, git2::Error> {
    let mut files = Vec::new();
    let statuses = repo.statuses(None)?;

    for entry in statuses.iter() {
        if let Some(path) = entry.path() {
            let status = entry.status();

            // Check if file is staged (in index)
            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
            {
                files.push(GitFileStatus {
                    path: path.to_string(),
                    status: format_status(status),
                    staged: true,
                });
            }

            // Check if file is unstaged (in working tree)
            if status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_renamed()
            {
                files.push(GitFileStatus {
                    path: path.to_string(),
                    status: format_status(status),
                    staged: false,
                });
            }
        }
    }

    Ok(files)
}

fn format_status(status: git2::Status) -> String {
    if status.is_index_new() || status.is_wt_new() {
        "added".to_string()
    } else if status.is_index_modified() || status.is_wt_modified() {
        "modified".to_string()
    } else if status.is_index_deleted() || status.is_wt_deleted() {
        "deleted".to_string()
    } else if status.is_index_renamed() || status.is_wt_renamed() {
        "renamed".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn get_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    // Check if file is untracked
    let statuses = repo.statuses(None)?;
    let mut is_untracked = false;

    for entry in statuses.iter() {
        if entry.path() == Some(path) && entry.status().is_wt_new() {
            is_untracked = true;
            break;
        }
    }

    // For untracked files, generate diff manually
    if is_untracked && !staged {
        let workdir = repo
            .workdir()
            .ok_or_else(|| git2::Error::from_str("Repository has no working directory"))?;
        let file_path = workdir.join(path);

        if let Ok(content) = std::fs::read_to_string(&file_path) {
            let line_count = content.lines().count();
            let mut diff_text = format!("@@ -0,0 +1,{} @@\n", line_count);
            for line in content.lines() {
                diff_text.push('+');
                diff_text.push_str(line);
                diff_text.push('\n');
            }
            return Ok(diff_text);
        }
    }

    let mut diff_options = git2::DiffOptions::new();
    diff_options.pathspec(path);

    let diff = if staged {
        let head = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head), None, Some(&mut diff_options))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut diff_options))?
    };

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        let content = std::str::from_utf8(line.content()).unwrap_or("");

        match origin {
            '+' | '-' | ' ' => {
                diff_text.push(origin);
                diff_text.push_str(content);
            }
            _ => {
                // For other lines (headers, etc), include as-is
                diff_text.push_str(content);
            }
        }
        true
    })?;

    Ok(diff_text)
}

pub fn ignore_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let gitignore_path = repo.workdir().unwrap().join(".gitignore");

    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)
            .map_err(|e| git2::Error::from_str(&format!("Failed to read .gitignore: {}", e)))?
    } else {
        String::new()
    };

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    content.push_str(file_path);
    content.push('\n');

    std::fs::write(&gitignore_path, content)
        .map_err(|e| git2::Error::from_str(&format!("Failed to write .gitignore: {}", e)))?;

    Ok(())
}

pub fn stage_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    index.add_path(std::path::Path::new(file_path))?;
    index.write()?;
    Ok(())
}

pub fn stage_hunk(
    repo: &Repository,
    _file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    // Extract the first 4 lines as diff header (like GitX does)
    let diff_lines: Vec<&str> = full_diff.lines().collect();
    let diff_header = diff_lines
        .iter()
        .take(4)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    // Create patch: header + hunk header + hunk lines
    let patch = format!("{}\n{}\n{}\n", diff_header, hunk_header, hunk_lines);

    eprintln!("=== STAGE PATCH ===");
    eprintln!("{}", patch);
    eprintln!("=== END ===");

    // Use git apply command with --unidiff-zero and --cached
    let output = std::process::Command::new("git")
        .args(["apply", "--unidiff-zero", "--cached", "--ignore-whitespace"])
        .current_dir(workdir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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

pub fn unstage_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    // Use git reset HEAD <file> to unstage
    let output = std::process::Command::new("git")
        .args(["reset", "HEAD", file_path])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git reset: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(&format!(
            "git reset failed: {}",
            stderr
        )));
    }

    Ok(())
}

pub fn unstage_hunk(
    repo: &Repository,
    _file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    // Extract the first 4 lines as diff header (like GitX does)
    let diff_lines: Vec<&str> = full_diff.lines().collect();
    let diff_header = diff_lines
        .iter()
        .take(4)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    // Create patch: header + hunk header + hunk lines
    let patch = format!("{}\n{}\n{}\n", diff_header, hunk_header, hunk_lines);

    eprintln!("=== UNSTAGE PATCH ===");
    eprintln!("{}", patch);
    eprintln!("=== END ===");

    // Use git apply with --cached and --reverse to unstage
    let output = std::process::Command::new("git")
        .args([
            "apply",
            "--unidiff-zero",
            "--cached",
            "--reverse",
            "--ignore-whitespace",
        ])
        .current_dir(workdir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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

pub fn discard_file(repo: &Repository, file_path: &str) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    // Use git checkout -- <file> to discard changes
    let output = std::process::Command::new("git")
        .args(["checkout", "--", file_path])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git checkout: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(&format!(
            "git checkout failed: {}",
            stderr
        )));
    }

    Ok(())
}

pub fn discard_hunk(
    repo: &Repository,
    _file_path: &str,
    full_diff: &str,
    hunk_header: &str,
    hunk_lines: &str,
) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    let diff_lines: Vec<&str> = full_diff.lines().collect();
    let diff_header = diff_lines
        .iter()
        .take(4)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let patch = format!("{}\n{}\n{}\n", diff_header, hunk_header, hunk_lines);

    // Use git apply --reverse (without --cached) to discard working tree changes
    let output = std::process::Command::new("git")
        .args([
            "apply",
            "--unidiff-zero",
            "--reverse",
            "--ignore-whitespace",
        ])
        .current_dir(workdir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
