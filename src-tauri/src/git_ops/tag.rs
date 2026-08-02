use super::common::{resolve_author_signature, resolve_commit_from_ref};
use git2::Repository;

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

pub fn create_tag(
    repo: &Repository,
    tag_name: &str,
    from_ref: &str,
    message: Option<&str>,
) -> Result<(), git2::Error> {
    let target_commit = resolve_commit_from_ref(repo, from_ref)?;

    match message {
        Some(msg) if !msg.trim().is_empty() => {
            let signature = resolve_author_signature(repo)?;
            repo.tag(tag_name, target_commit.as_object(), &signature, msg, false)?;
        }
        _ => {
            repo.tag_lightweight(tag_name, target_commit.as_object(), false)?;
        }
    }

    Ok(())
}

pub fn delete_tag(repo: &Repository, tag_name: &str) -> Result<(), git2::Error> {
    repo.tag_delete(tag_name)
}
