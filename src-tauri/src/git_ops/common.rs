use git2::Repository;

// Unlike the `git` CLI, libgit2's `Repository::signature()` won't fall back to
// an OS-derived name when `user.name` isn't set in git config. Shell out to
// `git var`, which is what the CLI itself uses, to match its behavior.
pub(crate) fn resolve_author_signature(
    repo: &Repository,
) -> Result<git2::Signature<'static>, git2::Error> {
    if let Ok(sig) = repo.signature() {
        return Ok(sig.to_owned());
    }

    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("No working directory"))?;

    let output = std::process::Command::new("git")
        .args(["var", "GIT_AUTHOR_IDENT"])
        .current_dir(workdir)
        .output()
        .map_err(|e| git2::Error::from_str(&format!("Failed to run git var: {}", e)))?;

    if !output.status.success() {
        return Err(git2::Error::from_str(
            "Git identity not set. Run `git config --global user.name \"Your Name\"` \
             and `git config --global user.email \"you@example.com\"`, then try again.",
        ));
    }

    let ident = String::from_utf8_lossy(&output.stdout);
    let ident = ident.trim();
    let (name, rest) = ident
        .split_once('<')
        .ok_or_else(|| git2::Error::from_str("Could not parse git identity"))?;
    let (email, _) = rest
        .split_once('>')
        .ok_or_else(|| git2::Error::from_str("Could not parse git identity"))?;

    git2::Signature::now(name.trim(), email.trim())
}

pub(crate) fn resolve_commit_from_ref<'repo>(
    repo: &'repo Repository,
    reference: &str,
) -> Result<git2::Commit<'repo>, git2::Error> {
    if let Ok(branch) = repo.find_branch(reference, git2::BranchType::Local) {
        return branch.get().peel_to_commit();
    }
    if let Ok(branch) = repo.find_branch(reference, git2::BranchType::Remote) {
        return branch.get().peel_to_commit();
    }
    repo.revparse_single(reference)?.peel_to_commit()
}
