use super::common::resolve_commit_from_ref;
use git2::Repository;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct GitBranch {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
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

pub fn create_branch(
    repo: &Repository,
    branch_name: &str,
    from_branch: &str,
) -> Result<(), git2::Error> {
    let target_commit = resolve_commit_from_ref(repo, from_branch)?;
    repo.branch(branch_name, &target_commit, false)?;
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

#[derive(Serialize)]
pub struct BranchDeleteInfo {
    pub has_local: bool,
    pub remote_names: Vec<String>,
}

pub fn get_branch_delete_info(
    repo: &Repository,
    branch_name: &str,
    remote_context: Option<&str>,
) -> Result<BranchDeleteInfo, git2::Error> {
    // Invoked from a specific remote's branch list: existence of that remote
    // ref is a given, we only need to check for a same-named local branch.
    if let Some(remote) = remote_context {
        let has_local = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .is_ok();
        return Ok(BranchDeleteInfo {
            has_local,
            remote_names: vec![remote.to_string()],
        });
    }

    // Invoked from the local branches list. A branch can be pushed to (and
    // exist on) more than one remote even though git only tracks a single
    // upstream, so check every configured remote rather than just upstream().
    repo.find_branch(branch_name, git2::BranchType::Local)?;

    let mut remote_names = Vec::new();
    for remote in repo.remotes()?.iter().flatten() {
        let remote_branch = format!("{}/{}", remote, branch_name);
        if repo
            .find_branch(&remote_branch, git2::BranchType::Remote)
            .is_ok()
        {
            remote_names.push(remote.to_string());
        }
    }

    Ok(BranchDeleteInfo {
        has_local: true,
        remote_names,
    })
}

pub fn delete_local_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("No working directory"))?;

    let output = std::process::Command::new("git")
        .args(["branch", "-D", branch_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git branch -D: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}

pub fn delete_remote_branch(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("No working directory"))?;

    let output = std::process::Command::new("git")
        .args(["push", remote_name, "--delete", branch_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git push --delete: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}
