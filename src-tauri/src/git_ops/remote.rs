use git2::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
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

pub fn push_remote(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), git2::Error> {
    let workdir = repo.workdir().unwrap();

    let output = std::process::Command::new("git")
        .args(["push", "--set-upstream", remote_name, branch_name])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git push: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(git2::Error::from_str(stderr.as_ref()));
    }

    Ok(())
}
