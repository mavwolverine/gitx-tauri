use super::commit::GitAuthor;
use git2::Repository;
use serde::Serialize;
use std::path::Path;

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
