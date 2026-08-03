use super::commit::{diff_trees_to_commit_files, CommitFile};
use super::common::resolve_author_signature;
use git2::{Repository, StashFlags};
use serde::Serialize;

#[derive(Serialize)]
pub struct GitStash {
    pub index: usize,
    pub oid: String,
    pub message: String,
    pub timestamp: String,
}

pub fn save_stash(
    repo: &mut Repository,
    message: Option<&str>,
    include_untracked: bool,
) -> Result<String, git2::Error> {
    let signature = resolve_author_signature(repo)?;
    let flags = if include_untracked {
        StashFlags::INCLUDE_UNTRACKED
    } else {
        StashFlags::DEFAULT
    };
    let oid = repo.stash_save2(&signature, message, Some(flags))?;
    Ok(oid.to_string())
}

pub fn get_stashes(repo: &mut Repository) -> Result<Vec<GitStash>, git2::Error> {
    let mut stashes = Vec::new();

    repo.stash_foreach(|index, message, oid| {
        stashes.push((index, message.to_string(), *oid));
        true
    })?;

    stashes
        .into_iter()
        .map(|(index, message, oid)| {
            let timestamp = repo
                .find_commit(oid)
                .map(|c| c.author().when().seconds().to_string())
                .unwrap_or_default();
            Ok(GitStash {
                index,
                oid: oid.to_string(),
                message,
                timestamp,
            })
        })
        .collect()
}

pub fn apply_stash(repo: &mut Repository, index: usize) -> Result<(), git2::Error> {
    repo.stash_apply(index, None)
}

pub fn pop_stash(repo: &mut Repository, index: usize) -> Result<(), git2::Error> {
    repo.stash_pop(index, None)
}

pub fn drop_stash(repo: &mut Repository, index: usize) -> Result<(), git2::Error> {
    repo.stash_drop(index)
}

// A stash commit's own tree only holds tracked changes — untracked (and, if
// stashed with `--all`, ignored) files live in extra parent commits that are
// never merged into that tree. Matches `git stash show -u`: diff the tracked
// changes against parent 0, then show every other parent beyond the index
// snapshot (parent 1) as newly-added files.
pub fn get_stash_diff(repo: &Repository, oid_str: &str) -> Result<Vec<CommitFile>, git2::Error> {
    let oid = git2::Oid::from_str(oid_str)
        .map_err(|e| git2::Error::from_str(&format!("Invalid stash ID: {}", e)))?;
    let commit = repo.find_commit(oid)?;

    let commit_tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut files = diff_trees_to_commit_files(repo, parent_tree.as_ref(), &commit_tree)?;

    for i in 2..commit.parent_count() {
        let extra_tree = commit.parent(i)?.tree()?;
        files.extend(diff_trees_to_commit_files(repo, None, &extra_tree)?);
    }

    Ok(files)
}
