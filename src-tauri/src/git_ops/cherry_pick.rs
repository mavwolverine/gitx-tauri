use super::common::resolve_author_signature;
use git2::Repository;

// Matches plain `git cherry-pick <commit>`: apply the commit's changes and
// immediately create a new commit reusing its message and author, with the
// current user as committer. On conflict, don't auto-commit — that would
// bake in unresolved conflict markers — surface an error instead and leave
// the index/working tree as-is for manual resolution via the terminal.
pub fn cherry_pick(repo: &Repository, commit_id: &str) -> Result<String, git2::Error> {
    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let commit = repo.find_commit(oid)?;

    repo.cherrypick(&commit, None)?;

    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Err(git2::Error::from_str(
            "Cherry-pick resulted in conflicts. Resolve them in a terminal, then run \
             `git cherry-pick --continue` (or `git cherry-pick --abort` to cancel).",
        ));
    }

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head_commit = repo.head()?.peel_to_commit()?;
    let committer = resolve_author_signature(repo)?;

    let new_oid = repo.commit(
        Some("HEAD"),
        &commit.author(),
        &committer,
        commit.message().unwrap_or(""),
        &tree,
        &[&head_commit],
    )?;

    repo.cleanup_state()?;

    Ok(new_oid.to_string())
}

// Whether cherry-picking this commit onto HEAD would be a no-op — i.e. it's
// HEAD itself or already one of its ancestors. Used to grey out the
// context-menu item; deliberately independent of whatever branch happens to
// be selected for viewing in the history panel, since cherry-pick always
// targets the actually-checked-out branch.
pub fn is_ancestor_of_head(repo: &Repository, commit_id: &str) -> Result<bool, git2::Error> {
    let oid = git2::Oid::from_str(commit_id)
        .map_err(|e| git2::Error::from_str(&format!("Invalid commit ID: {}", e)))?;
    let head_oid = repo.head()?.peel_to_commit()?.id();

    if head_oid == oid {
        return Ok(true);
    }
    repo.graph_descendant_of(head_oid, oid)
}
