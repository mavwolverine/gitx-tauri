use super::branch::GitBranch;
use super::common::resolve_author_signature;
use git2::Repository;
use serde::Serialize;

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

pub fn create_commit(repo: &Repository, message: &str, amend: bool) -> Result<String, git2::Error> {
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = resolve_author_signature(repo)?;

    if amend {
        let head = repo.head()?;
        let head_commit = head.peel_to_commit()?;
        let new_oid = head_commit.amend(
            Some("HEAD"),
            Some(&signature),
            Some(&signature),
            None,
            Some(message),
            Some(&tree),
        )?;
        Ok(new_oid.to_string())
    } else {
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None,
        };
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        let oid = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;
        Ok(oid.to_string())
    }
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
