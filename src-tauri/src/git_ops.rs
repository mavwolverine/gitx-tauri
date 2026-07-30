use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
pub struct GitBranch {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
}

#[derive(Serialize)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct GitSubmodule {
    pub name: String,
    pub path: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Serialize)]
pub struct GitAuthor {
    pub name: String,
    pub email: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct GitCommit {
    pub id: String,
    pub message: String,
    pub author: GitAuthor,
    pub committer: GitAuthor,
    pub parents: Vec<String>,
    pub branches: Option<Vec<GitBranch>>,
    pub tags: Option<Vec<String>>,
    pub lane: usize,
    pub lines: Vec<GraphLine>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphLine {
    pub upper: bool,  // true = upper half, false = lower half
    pub from: usize,  // starting lane
    pub to: usize,    // ending lane
    pub color: usize, // color index
}

#[derive(Clone, Serialize)]
#[allow(dead_code)]
pub struct GraphInfo {
    pub lane: usize,
    pub color: usize,
}

#[derive(Serialize)]
pub struct CommitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize)]
pub struct DiffLine {
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub origin: char,
    pub content: String,
}

#[derive(Serialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub oid: String,
    pub children: Option<Vec<TreeNode>>,
}

fn create_remote_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    callbacks.transfer_progress(|stats| {
        println!(
            "Received {}/{} objects ({} bytes)",
            stats.received_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
        true
    });
    callbacks
}

pub fn clone_repository(url: &str, path: &str) -> Result<(), git2::Error> {
    let callbacks = create_remote_callbacks();

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.clone(url, Path::new(path))?;
    Ok(())
}

pub fn is_git_repository(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

pub fn open_repository(path: &str) -> Result<Repository, git2::Error> {
    Repository::open(path)
}

pub fn get_branches(repo: &Repository) -> Result<Vec<GitBranch>, git2::Error> {
    let mut branches = Vec::new();
    let is_detached = repo.head_detached()?;
    let head = repo.head()?;
    let head_name = head.shorthand();

    // Add detached HEAD as a special branch if applicable
    if is_detached {
        branches.push(GitBranch {
            name: "HEAD (detached)".to_string(),
            is_head: true,
            is_remote: false,
        });
    }

    for branch in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            branches.push(GitBranch {
                name: name.to_string(),
                is_head: !is_detached && Some(name) == head_name,
                is_remote: false,
            });
        }
    }

    Ok(branches)
}

pub fn get_branch_head(repo: &Repository, branch_name: &str) -> Result<String, git2::Error> {
    // Try local branch first
    if let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
        let commit = branch.get().peel_to_commit()?;
        return Ok(commit.id().to_string());
    }

    // Try remote branch
    if let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Remote) {
        let commit = branch.get().peel_to_commit()?;
        return Ok(commit.id().to_string());
    }

    // Try as a reference (for remote branches like origin/branch)
    let ref_name = format!("refs/remotes/{}", branch_name);
    if let Ok(reference) = repo.find_reference(&ref_name) {
        let commit = reference.peel_to_commit()?;
        return Ok(commit.id().to_string());
    }

    Err(git2::Error::from_str(&format!(
        "Branch '{}' not found",
        branch_name
    )))
}

pub fn get_remotes(repo: &Repository) -> Result<Vec<GitRemote>, git2::Error> {
    let mut remotes = Vec::new();

    for name in repo.remotes()?.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                remotes.push(GitRemote {
                    name: name.to_string(),
                    url: url.to_string(),
                });
            }
        }
    }

    Ok(remotes)
}

pub fn get_remote_branches(
    repo: &Repository,
    remote_name: &str,
) -> Result<Vec<String>, git2::Error> {
    let mut branches = Vec::new();
    let prefix = format!("{}/", remote_name);

    for branch in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            if name.starts_with(&prefix) {
                let branch_name = name.strip_prefix(&prefix).unwrap_or(name);
                if branch_name != "HEAD" {
                    branches.push(branch_name.to_string());
                }
            }
        }
    }

    Ok(branches)
}

pub fn get_tags(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    let mut tags = Vec::new();

    repo.tag_foreach(|_oid, name| {
        if let Ok(name_str) = std::str::from_utf8(name) {
            if let Some(tag_name) = name_str.strip_prefix("refs/tags/") {
                tags.push(tag_name.to_string());
            }
        }
        true
    })?;

    tags.sort();
    Ok(tags)
}

pub fn get_tag_commit(repo: &Repository, tag_name: &str) -> Result<String, git2::Error> {
    let reference = repo.find_reference(&format!("refs/tags/{}", tag_name))?;
    let target = reference.peel_to_commit()?;
    Ok(target.id().to_string())
}

pub fn get_submodules(repo: &Repository) -> Result<Vec<GitSubmodule>, git2::Error> {
    let mut submodules = Vec::new();

    for submodule in repo.submodules()? {
        if let (Some(name), Some(url)) = (submodule.name(), submodule.url()) {
            let path = submodule.path();
            submodules.push(GitSubmodule {
                name: name.to_string(),
                path: path.to_str().unwrap_or("").to_string(),
                url: url.to_string(),
            });
        }
    }

    Ok(submodules)
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

pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    // Use git checkout with error handling
    let output = std::process::Command::new("git")
        .args(["checkout", branch_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git checkout: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}

pub fn fetch_remote(repo: &Repository, remote_name: &str) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    let output = std::process::Command::new("git")
        .args(["fetch", remote_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git fetch: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}

pub fn pull_remote(repo: &Repository, remote_name: &str) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    let output = std::process::Command::new("git")
        .args(["pull", remote_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git pull: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}

pub fn get_commits(
    repo: &Repository,
    limit: usize,
    local_only: bool,
    branch_name: Option<&str>,
) -> Result<Vec<GitCommit>, git2::Error> {
    let mut revwalk = repo.revwalk()?;

    // If specific branch requested, only walk from that branch
    if let Some(branch) = branch_name {
        // Try local branch first
        if let Ok(b) = repo.find_branch(branch, git2::BranchType::Local) {
            if let Some(target) = b.get().target() {
                revwalk.push(target)?;
            }
        } else if let Ok(b) = repo.find_branch(branch, git2::BranchType::Remote) {
            // Try remote branch
            if let Some(target) = b.get().target() {
                revwalk.push(target)?;
            }
        } else {
            // Try as reference
            let ref_name = format!("refs/remotes/{}", branch);
            if let Ok(reference) = repo.find_reference(&ref_name) {
                if let Some(target) = reference.target() {
                    revwalk.push(target)?;
                }
            }
        }
    } else {
        // Push branches based on filter
        let filter_branch_type = if local_only {
            Some(git2::BranchType::Local)
        } else {
            None // All branches
        };

        for branch in repo.branches(filter_branch_type)? {
            let (branch, _) = branch?;
            if let Some(target) = branch.get().target() {
                revwalk.push(target)?;
            }
        }
    }

    revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

    let mut branch_map: std::collections::HashMap<git2::Oid, Vec<GitBranch>> =
        std::collections::HashMap::new();
    let is_detached = repo.head_detached().unwrap_or(false);
    let head_name = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    for branch in repo.branches(None)? {
        let (branch, branch_type) = branch?;
        if let Some(name) = branch.name()? {
            if let Some(target) = branch.get().target() {
                branch_map.entry(target).or_default().push(GitBranch {
                    name: name.to_string(),
                    is_head: !is_detached && Some(name.to_string()) == head_name,
                    is_remote: branch_type == git2::BranchType::Remote,
                });
            }
        }
    }

    let mut tag_map: std::collections::HashMap<git2::Oid, Vec<String>> =
        std::collections::HashMap::new();

    repo.tag_foreach(|oid, name| {
        if let Ok(name_str) = std::str::from_utf8(name) {
            if let Some(tag_name) = name_str.strip_prefix("refs/tags/") {
                // Peel to the commit OID (handles both lightweight and annotated tags)
                let target_oid = repo
                    .find_object(oid, None)
                    .and_then(|obj| obj.peel(git2::ObjectType::Commit))
                    .map(|obj| obj.id())
                    .unwrap_or(oid);
                tag_map
                    .entry(target_oid)
                    .or_default()
                    .push(tag_name.to_string());
            }
        }
        true
    })?;

    let mut commits = Vec::new();
    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let parents: Vec<String> = commit.parents().map(|p| p.id().to_string()).collect();
        let branches = branch_map.get(&oid).cloned();
        let tags = tag_map.get(&oid).cloned();

        let commit_author = commit.author();
        let commit_committer = commit.committer();

        let author = GitAuthor {
            name: commit_author.name().unwrap_or("").to_string(),
            email: commit_author.email().unwrap_or("").to_string(),
            timestamp: commit_author.when().seconds().to_string(),
        };

        let committer = GitAuthor {
            name: commit_committer.name().unwrap_or("").to_string(),
            email: commit_committer.email().unwrap_or("").to_string(),
            timestamp: commit_committer.when().seconds().to_string(),
        };

        commits.push(GitCommit {
            id: commit.id().to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author,
            committer,
            parents,
            branches,
            tags,
            lane: 0,
            lines: Vec::new(),
        });
    }

    // Calculate lanes and lines
    calculate_lanes(&mut commits);

    Ok(commits)
}

fn calculate_lanes(commits: &mut [GitCommit]) {
    struct Lane {
        sha: Option<String>,
        color_index: usize,
    }

    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut color_counter = 0;

    for commit in commits.iter_mut() {
        let commit_id = commit.id.clone();
        let mut new_lanes: Vec<Option<Lane>> = Vec::new();
        let mut current_lane: Option<usize> = None;
        let mut current_color = 0;
        let mut found_first = false;
        let mut lines: Vec<GraphLine> = Vec::new();

        // First, iterate over existing lanes and pass through any that don't want this commit
        for (i, lane) in lanes.iter().enumerate() {
            if let Some(lane_data) = lane {
                // This lane is expecting our commit
                if lane_data.sha.as_ref() == Some(&commit_id) {
                    if !found_first {
                        found_first = true;
                        current_lane = Some(new_lanes.len());
                        current_color = lane_data.color_index;
                        new_lanes.push(Some(Lane {
                            sha: None, // Will be set to first parent
                            color_index: lane_data.color_index,
                        }));
                        // Upper line from previous lane to current position
                        lines.push(GraphLine {
                            upper: true,
                            from: i,
                            to: new_lanes.len() - 1,
                            color: lane_data.color_index,
                        });
                        // Lower line at current position
                        if !commit.parents.is_empty() {
                            lines.push(GraphLine {
                                upper: false,
                                from: new_lanes.len() - 1,
                                to: new_lanes.len() - 1,
                                color: lane_data.color_index,
                            });
                        }
                    } else {
                        // Merge - this lane converges to current_lane
                        if let Some(cur_lane) = current_lane {
                            lines.push(GraphLine {
                                upper: true,
                                from: i,
                                to: cur_lane,
                                color: lane_data.color_index,
                            });
                        }
                    }
                } else {
                    // Not our commit, pass through
                    new_lanes.push(Some(Lane {
                        sha: lane_data.sha.clone(),
                        color_index: lane_data.color_index,
                    }));
                    // Pass-through lines
                    lines.push(GraphLine {
                        upper: true,
                        from: i,
                        to: new_lanes.len() - 1,
                        color: lane_data.color_index,
                    });
                    lines.push(GraphLine {
                        upper: false,
                        from: new_lanes.len() - 1,
                        to: new_lanes.len() - 1,
                        color: lane_data.color_index,
                    });
                }
            } else {
                // Empty lane
                new_lanes.push(None);
            }
        }

        // If we didn't find a lane expecting us, create new one
        if !found_first && !commit.parents.is_empty() {
            current_lane = Some(new_lanes.len());
            current_color = color_counter;
            new_lanes.push(Some(Lane {
                sha: None,
                color_index: color_counter,
            }));
            // Lower line for new commit
            lines.push(GraphLine {
                upper: false,
                from: new_lanes.len() - 1,
                to: new_lanes.len() - 1,
                color: color_counter,
            });
            color_counter += 1;
        }

        // Set the lane for this commit
        commit.lane = current_lane.unwrap_or(0);
        commit.lines = lines;

        // Update current lane to point to first parent
        if let Some(lane_idx) = current_lane {
            if let Some(first_parent) = commit.parents.first() {
                if let Some(Some(lane)) = new_lanes.get_mut(lane_idx) {
                    lane.sha = Some(first_parent.clone());
                }
            } else {
                // No parents - clear the lane
                new_lanes[lane_idx] = None;
            }
        }

        // Add other parents to new lanes
        for parent_id in commit.parents.iter().skip(1) {
            // Check if parent already in a lane
            let mut found_lane_idx = None;
            for (idx, lane) in new_lanes.iter().enumerate() {
                if let Some(lane_data) = lane {
                    if lane_data.sha.as_ref() == Some(parent_id) {
                        found_lane_idx = Some(idx);
                        break;
                    }
                }
            }

            if let Some(parent_lane_idx) = found_lane_idx {
                // Parent already has a lane, draw line to it
                if let Some(cur_lane) = current_lane {
                    // Use the parent lane's color
                    let parent_color = new_lanes[parent_lane_idx]
                        .as_ref()
                        .map(|l| l.color_index)
                        .unwrap_or(current_color);
                    commit.lines.push(GraphLine {
                        upper: false,
                        from: cur_lane,
                        to: parent_lane_idx,
                        color: parent_color,
                    });
                }
            } else {
                // Find empty lane or create new one
                let empty_idx = new_lanes.iter().position(|l| l.is_none());
                let new_lane_idx = if let Some(idx) = empty_idx {
                    new_lanes[idx] = Some(Lane {
                        sha: Some(parent_id.clone()),
                        color_index: color_counter,
                    });
                    idx
                } else {
                    new_lanes.push(Some(Lane {
                        sha: Some(parent_id.clone()),
                        color_index: color_counter,
                    }));
                    new_lanes.len() - 1
                };

                // Draw line from current commit to new parent lane
                if let Some(cur_lane) = current_lane {
                    commit.lines.push(GraphLine {
                        upper: false,
                        from: cur_lane,
                        to: new_lane_idx,
                        color: color_counter,
                    });
                }
                color_counter += 1;
            }
        }

        lanes = new_lanes;
    }
}

pub fn get_commit_diff(repo: &Repository, commit_id: &str) -> Result<Vec<CommitFile>, git2::Error> {
    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let commit = repo.find_commit(oid)?;

    let commit_tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)?;

    let mut files = Vec::new();

    for (delta_idx, delta) in diff.deltas().enumerate() {
        let old_file = delta.old_file();
        let new_file = delta.new_file();

        let path = new_file
            .path()
            .or(old_file.path())
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        let old_path = if old_file.path() != new_file.path() {
            old_file
                .path()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let status = match delta.status() {
            git2::Delta::Added => "added",
            git2::Delta::Deleted => "deleted",
            git2::Delta::Modified => "modified",
            git2::Delta::Renamed => "renamed",
            git2::Delta::Copied => "copied",
            _ => "unknown",
        }
        .to_string();

        let mut lines = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        let patch = git2::Patch::from_diff(&diff, delta_idx)?;
        if let Some(patch) = patch {
            for hunk_idx in 0..patch.num_hunks() {
                let (hunk, _) = patch.hunk(hunk_idx)?;

                // Add hunk header
                lines.push(DiffLine {
                    old_lineno: None,
                    new_lineno: None,
                    origin: '@',
                    content: format!(
                        "@@ -{},{} +{},{} @@",
                        hunk.old_start(),
                        hunk.old_lines(),
                        hunk.new_start(),
                        hunk.new_lines()
                    ),
                });

                for line_idx in 0..patch.num_lines_in_hunk(hunk_idx)? {
                    let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                    let origin = line.origin();
                    let content = std::str::from_utf8(line.content())
                        .unwrap_or("")
                        .to_string();

                    let old_lineno = line.old_lineno();
                    let new_lineno = line.new_lineno();

                    match origin {
                        '+' => additions += 1,
                        '-' => deletions += 1,
                        _ => {}
                    }

                    lines.push(DiffLine {
                        old_lineno,
                        new_lineno,
                        origin,
                        content,
                    });
                }
            }
        }

        files.push(CommitFile {
            path,
            old_path,
            status,
            additions,
            deletions,
            lines,
        });
    }

    Ok(files)
}

pub fn get_commit_tree(repo: &Repository, commit_id: &str) -> Result<Vec<TreeNode>, git2::Error> {
    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    build_tree_nodes(repo, &tree, "")
}

fn build_tree_nodes(
    repo: &Repository,
    tree: &git2::Tree,
    parent_path: &str,
) -> Result<Vec<TreeNode>, git2::Error> {
    let mut nodes = Vec::new();

    for entry in tree.iter() {
        let name = entry.name().unwrap_or("").to_string();
        let oid = entry.id();
        let path = if parent_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", parent_path, name)
        };

        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let subtree = repo.find_tree(oid)?;
                let children = build_tree_nodes(repo, &subtree, &path)?;
                nodes.push(TreeNode {
                    name,
                    path,
                    is_dir: true,
                    oid: oid.to_string(),
                    children: Some(children),
                });
            }
            Some(git2::ObjectType::Blob) => {
                nodes.push(TreeNode {
                    name,
                    path,
                    is_dir: false,
                    oid: oid.to_string(),
                    children: None,
                });
            }
            _ => {
                // Skip submodules (commit entries) and other object kinds for now
            }
        }
    }

    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(nodes)
}

#[derive(Serialize)]
pub struct FileContent {
    pub is_binary: bool,
    pub size: u64,
    pub content: Option<String>,
}

pub fn get_file_content(
    repo: &Repository,
    commit_id: &str,
    file_path: &str,
) -> Result<FileContent, git2::Error> {
    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let entry = tree.get_path(Path::new(file_path))?;
    let object = entry.to_object(repo)?;
    let blob = object
        .as_blob()
        .ok_or_else(|| git2::Error::from_str("Path does not refer to a file"))?;

    if blob.is_binary() {
        Ok(FileContent {
            is_binary: true,
            size: blob.size() as u64,
            content: None,
        })
    } else {
        Ok(FileContent {
            is_binary: false,
            size: blob.size() as u64,
            content: Some(String::from_utf8_lossy(blob.content()).into_owned()),
        })
    }
}

#[derive(Serialize)]
pub struct BlameLine {
    pub line_no: usize,
    pub content: String,
    pub commit_id: String,
    pub author: String,
    pub timestamp: String,
}

pub fn get_file_blame(
    repo: &Repository,
    commit_id: &str,
    file_path: &str,
) -> Result<Vec<BlameLine>, git2::Error> {
    let file = get_file_content(repo, commit_id, file_path)?;
    let Some(content) = file.content else {
        return Ok(Vec::new());
    };

    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let mut opts = git2::BlameOptions::new();
    opts.newest_commit(oid);
    let blame = repo.blame_file(Path::new(file_path), Some(&mut opts))?;

    let mut commit_cache: std::collections::HashMap<git2::Oid, (String, String)> =
        std::collections::HashMap::new();

    let mut lines = Vec::new();
    for (i, line_content) in content.lines().enumerate() {
        let line_no = i + 1;
        let Some(hunk) = blame.get_line(line_no) else {
            continue;
        };
        let final_commit_id = hunk.final_commit_id();

        let (author, timestamp) = if let Some(cached) = commit_cache.get(&final_commit_id) {
            cached.clone()
        } else {
            let (author, timestamp) = match repo.find_commit(final_commit_id) {
                Ok(c) => {
                    let sig = c.author();
                    (
                        sig.name().unwrap_or("").to_string(),
                        sig.when().seconds().to_string(),
                    )
                }
                Err(_) => (String::new(), String::new()),
            };
            commit_cache.insert(final_commit_id, (author.clone(), timestamp.clone()));
            (author, timestamp)
        };

        lines.push(BlameLine {
            line_no,
            content: line_content.to_string(),
            commit_id: final_commit_id.to_string(),
            author,
            timestamp,
        });
    }

    Ok(lines)
}

#[derive(Serialize)]
pub struct FileHistoryEntry {
    pub id: String,
    pub message: String,
    pub author: GitAuthor,
}

pub fn get_file_history(
    repo: &Repository,
    commit_id: &str,
    file_path: &str,
) -> Result<Vec<FileHistoryEntry>, git2::Error> {
    let start_oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(start_oid)?;
    revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

    let path = Path::new(file_path);
    let mut entries = Vec::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= 5000 {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let changed = if commit.parent_count() == 0 {
            tree.get_path(path).is_ok()
        } else {
            let parent_tree = commit.parent(0)?.tree()?;
            let mut diff_opts = git2::DiffOptions::new();
            diff_opts.pathspec(file_path);
            let diff =
                repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut diff_opts))?;
            diff.deltas().len() > 0
        };

        if changed {
            let sig = commit.author();
            let author = GitAuthor {
                name: sig.name().unwrap_or("").to_string(),
                email: sig.email().unwrap_or("").to_string(),
                timestamp: sig.when().seconds().to_string(),
            };
            entries.push(FileHistoryEntry {
                id: commit.id().to_string(),
                message: commit.message().unwrap_or("").to_string(),
                author,
            });
        }
    }

    Ok(entries)
}
