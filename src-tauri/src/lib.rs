#[cfg(target_os = "linux")]
mod appimage_desktop;
mod cli_install;
mod git_ops;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
async fn clone_repository(url: String, path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        git_ops::clone_repository(&url, &path).map_err(|e| e.to_string())?;
        Ok(format!("Repository cloned to: {}", path))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn is_git_repository(path: String) -> bool {
    git_ops::is_git_repository(&path)
}

#[tauri::command]
fn open_repository(path: String) -> Result<String, String> {
    match git_ops::open_repository(&path) {
        Ok(_) => Ok(format!("Repository opened: {}", path)),
        Err(e) => Err(format!("Failed to open repository: {}", e)),
    }
}

#[tauri::command]
fn get_branches(path: String) -> Result<Vec<git_ops::GitBranch>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_branches(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_remotes(path: String) -> Result<Vec<git_ops::GitRemote>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_remotes(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_remote_branches(path: String, remote_name: String) -> Result<Vec<String>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_remote_branches(&repo, &remote_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_tags(path: String) -> Result<Vec<String>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_tags(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_tag_commit(path: String, tag_name: String) -> Result<String, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_tag_commit(&repo, &tag_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_submodules(path: String) -> Result<Vec<git_ops::GitSubmodule>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_submodules(&repo).map_err(|e| e.to_string())
}

const WATCH_IGNORED_DIRS: [&str; 6] = [
    "target",
    "node_modules",
    "dist",
    "build",
    ".venv",
    "__pycache__",
];

/// Quiet period that ends a burst of fs events.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);
/// A burst that never goes quiet (a build, an editor autosaving) still
/// refreshes at least this often.
const WATCH_MAX_WAIT: Duration = Duration::from_secs(2);
/// Minimum gap between refreshes triggered by the window gaining focus.
const FOCUS_REFRESH_GAP: Duration = Duration::from_secs(3);

/// A directory tree watched for one repo window.
#[derive(Clone, Debug)]
struct WatchRoot {
    path: PathBuf,
    /// A git directory rather than the working tree.
    is_git_dir: bool,
}

/// The directory holding the refs and objects shared by a repo's worktrees.
/// A linked worktree's git dir names it in a `commondir` file; otherwise it
/// is the git dir itself.
fn common_dir(git_dir: &Path) -> PathBuf {
    std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .and_then(|rel| git_dir.join(rel.trim()).canonicalize().ok())
        .unwrap_or_else(|| git_dir.to_path_buf())
}

/// The working tree plus any git directories that live outside it. A linked
/// worktree's `.git` is a file pointing into the main repo's `.git`, and a
/// submodule's points into `<super>/.git/modules`; refs, HEAD and the index
/// change there, so those need watching too.
fn watch_roots(repo_path: &Path) -> Vec<WatchRoot> {
    let mut roots = vec![WatchRoot {
        path: repo_path.to_path_buf(),
        is_git_dir: false,
    }];
    let canonical = repo_path.canonicalize().ok();
    if let Ok(repo) = git2::Repository::open(repo_path) {
        let git_dir = repo.path().to_path_buf();
        let common_dir = common_dir(&git_dir);
        // common dir first: for a worktree it contains the worktree's own git dir.
        for dir in [common_dir.as_path(), git_dir.as_path()] {
            let covered = dir.starts_with(repo_path)
                || canonical.as_deref().is_some_and(|c| dir.starts_with(c))
                || roots.iter().any(|r| dir.starts_with(&r.path));
            if !covered {
                roots.push(WatchRoot {
                    path: dir.to_path_buf(),
                    is_git_dir: true,
                });
            }
        }
    }
    roots
}

/// Whether an fs event at `path` can be ignored: build output and dependency
/// folders in the working tree, and git's object store. Only the part of the
/// path below the watched root counts, since the repo itself may live in a
/// folder called `build/`, and a branch may be called `build`.
fn is_watch_noise(path: &Path, roots: &[WatchRoot]) -> bool {
    let Some(root) = roots
        .iter()
        .filter(|r| path.starts_with(&r.path))
        .max_by_key(|r| r.path.components().count())
    else {
        return false;
    };
    let Ok(rel) = path.strip_prefix(&root.path) else {
        return false;
    };
    let mut parts = rel.components().filter_map(|c| c.as_os_str().to_str());
    if root.is_git_dir {
        return parts.next() == Some("objects");
    }
    match parts.next() {
        Some(".git") => parts.next() == Some("objects"),
        first => first
            .into_iter()
            .chain(parts)
            .any(|part| WATCH_IGNORED_DIRS.contains(&part)),
    }
}

/// Collects `first` plus every event that follows until things go quiet for
/// `debounce`, or `max_wait` has passed since `first`, whichever is sooner.
fn collect_batch(
    rx: &Receiver<notify::Event>,
    first: notify::Event,
    debounce: Duration,
    max_wait: Duration,
) -> Vec<notify::Event> {
    let started = Instant::now();
    let mut batch = vec![first];
    loop {
        let remaining = max_wait.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(debounce.min(remaining)) {
            Ok(event) => batch.push(event),
            Err(_) => break,
        }
    }
    batch
}

/// One active watcher per window label. Dropping a watcher closes its event
/// channel, which ends the matching emitter thread.
#[derive(Default)]
struct RepoWatchers(Mutex<HashMap<String, RecommendedWatcher>>);

/// When each window last got a `repo-changed`, so the focus-triggered refresh
/// doesn't pile on top of one that just happened.
#[derive(Default)]
struct LastRefresh(Mutex<HashMap<String, Instant>>);

impl LastRefresh {
    fn touch(&self, label: &str) {
        self.0
            .lock()
            .unwrap()
            .insert(label.to_string(), Instant::now());
    }

    fn due(&self, label: &str, gap: Duration) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(label)
            .is_none_or(|t| t.elapsed() >= gap)
    }

    fn forget(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }
}

#[tauri::command]
fn watch_repo(
    window: tauri::Window,
    watchers: tauri::State<RepoWatchers>,
    repo_path: String,
) -> Result<(), String> {
    let (tx, rx) = channel::<notify::Event>();
    let roots = watch_roots(Path::new(&repo_path));
    let filter_roots = roots.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                // Events without paths (e.g. a queue-overflow rescan) can't be
                // classified, so assume they matter.
                if event.paths.is_empty()
                    || event
                        .paths
                        .iter()
                        .any(|p| !is_watch_noise(p, &filter_roots))
                {
                    let _ = tx.send(event);
                } else {
                    println!(
                        "[watch_repo] ignored (noise dir): {:?} {:?}",
                        event.kind, event.paths
                    );
                }
            }
            Err(e) => {
                println!("[watch_repo] watcher error: {e}");
                // Events may have been lost; refresh to be safe.
                let _ = tx.send(notify::Event::new(notify::EventKind::Other));
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )
    .map_err(|e| e.to_string())?;

    for (i, root) in roots.iter().enumerate() {
        match watcher.watch(&root.path, RecursiveMode::Recursive) {
            Ok(()) => {}
            // The working tree is required; extra git dirs are best effort.
            Err(e) if i == 0 => return Err(e.to_string()),
            Err(e) => println!("[watch_repo] could not watch {}: {e}", root.path.display()),
        }
    }

    let label = window.label().to_string();
    window.state::<LastRefresh>().touch(&label);

    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            // Coalesce bursts of events (e.g. a checkout touching many files)
            // into a single refresh instead of one per fs event.
            let batch = collect_batch(&rx, first, WATCH_DEBOUNCE, WATCH_MAX_WAIT);
            println!(
                "[watch_repo] repo-changed: {} event(s) triggered it, e.g. {:?} {:?}",
                batch.len(),
                batch[0].kind,
                batch[0].paths
            );
            // Window::emit broadcasts to every window; target only the one
            // that owns this watcher.
            let _ = window.emit_to(window.label(), "repo-changed", ());
            window.state::<LastRefresh>().touch(window.label());
        }
    });

    // Replacing an existing entry drops the previous watcher (e.g. on remount).
    watchers.0.lock().unwrap().insert(label, watcher);

    Ok(())
}

#[tauri::command]
fn get_status(path: String) -> Result<Vec<git_ops::GitFileStatus>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_status(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_diff(path: String, file_path: String, staged: bool) -> Result<String, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_diff(&repo, &file_path, staged).map_err(|e| e.to_string())
}

#[tauri::command]
fn unstage_file(path: String, file_path: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::unstage_file(&repo, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn unstage_hunk(
    path: String,
    file_path: String,
    full_diff: String,
    hunk_header: String,
    hunk_lines: String,
) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::unstage_hunk(&repo, &file_path, &full_diff, &hunk_header, &hunk_lines)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stage_hunk(
    path: String,
    file_path: String,
    full_diff: String,
    hunk_header: String,
    hunk_lines: String,
) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::stage_hunk(&repo, &file_path, &full_diff, &hunk_header, &hunk_lines)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stage_file(path: String, file_path: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::stage_file(&repo, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn discard_file(path: String, file_path: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::discard_file(&repo, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn discard_hunk(
    path: String,
    file_path: String,
    full_diff: String,
    hunk_header: String,
    hunk_lines: String,
) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::discard_hunk(&repo, &file_path, &full_diff, &hunk_header, &hunk_lines)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn ignore_file(path: String, file_path: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::ignore_file(&repo, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_commit(path: String, message: String, amend: bool) -> Result<String, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::create_commit(&repo, &message, amend).map_err(|e| e.to_string())
}

#[tauri::command]
fn cherry_pick(path: String, commit_id: String) -> Result<String, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::cherry_pick(&repo, &commit_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn is_ancestor_of_head(path: String, commit_id: String) -> Result<bool, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::is_ancestor_of_head(&repo, &commit_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_branch(path: String, branch_name: String, from_branch: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::create_branch(&repo, &branch_name, &from_branch).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_tag(
    path: String,
    tag_name: String,
    from_ref: String,
    message: Option<String>,
) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::create_tag(&repo, &tag_name, &from_ref, message.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_tag(path: String, tag_name: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::delete_tag(&repo, &tag_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_stash(
    path: String,
    message: Option<String>,
    include_untracked: bool,
) -> Result<String, String> {
    let mut repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::save_stash(&mut repo, message.as_deref(), include_untracked).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stashes(path: String) -> Result<Vec<git_ops::GitStash>, String> {
    let mut repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_stashes(&mut repo).map_err(|e| e.to_string())
}

#[tauri::command]
fn apply_stash(path: String, index: usize) -> Result<(), String> {
    let mut repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::apply_stash(&mut repo, index).map_err(|e| e.to_string())
}

#[tauri::command]
fn pop_stash(path: String, index: usize) -> Result<(), String> {
    let mut repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::pop_stash(&mut repo, index).map_err(|e| e.to_string())
}

#[tauri::command]
fn drop_stash(path: String, index: usize) -> Result<(), String> {
    let mut repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::drop_stash(&mut repo, index).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stash_diff(path: String, oid: String) -> Result<Vec<git_ops::CommitFile>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_stash_diff(&repo, &oid).map_err(|e| e.to_string())
}

#[tauri::command]
fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::checkout_branch(&repo, &branch_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_branch_delete_info(
    path: String,
    branch_name: String,
    remote_context: Option<String>,
) -> Result<git_ops::BranchDeleteInfo, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_branch_delete_info(&repo, &branch_name, remote_context.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_local_branch(path: String, branch_name: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::delete_local_branch(&repo, &branch_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_remote_branch(
    path: String,
    remote_name: String,
    branch_name: String,
) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::delete_remote_branch(&repo, &remote_name, &branch_name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_remote(path: String, remote_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
        git_ops::fetch_remote(&repo, &remote_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pull_remote(path: String, remote_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
        git_ops::pull_remote(&repo, &remote_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn push_remote(path: String, remote_name: String, branch_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
        git_ops::push_remote(&repo, &remote_name, &branch_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_commits(
    path: String,
    limit: usize,
    local_only: Option<bool>,
    branch_name: Option<String>,
) -> Result<Vec<git_ops::GitCommit>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_commits(
        &repo,
        limit,
        local_only.unwrap_or(false),
        branch_name.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_branch_head(path: String, branch_name: String) -> Result<String, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_branch_head(&repo, &branch_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_commit_diff(path: String, commit_id: String) -> Result<Vec<git_ops::CommitFile>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_commit_diff(&repo, &commit_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_commit_tree(path: String, commit_id: String) -> Result<Vec<git_ops::TreeNode>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_commit_tree(&repo, &commit_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_file_content(
    path: String,
    commit_id: String,
    file_path: String,
) -> Result<git_ops::FileContent, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_file_content(&repo, &commit_id, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_file_blame(
    path: String,
    commit_id: String,
    file_path: String,
) -> Result<Vec<git_ops::BlameLine>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_file_blame(&repo, &commit_id, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_file_history(
    path: String,
    commit_id: String,
    file_path: String,
) -> Result<Vec<git_ops::FileHistoryEntry>, String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::get_file_history(&repo, &commit_id, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_repo_window(app: tauri::AppHandle, repo_path: String) -> Result<(), String> {
    let label = format!("repo-{}", repo_path.replace(['/', '\\', ':', ' '], "-"));

    if let Some(window) = app.get_webview_window(&label) {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let url = format!("/?path={}", urlencoding::encode(&repo_path));
    let repo_name = repo_path
        .split(['/', '\\'])
        .next_back()
        .unwrap_or(&repo_path);

    // Get current branch name
    let branch_name = match git_ops::open_repository(&repo_path) {
        Ok(repo) => {
            if repo.head_detached().unwrap_or(false) {
                git_ops::DETACHED_HEAD_NAME.to_string()
            } else if let Ok(head) = repo.head() {
                head.shorthand().unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            }
        }
        Err(_) => "unknown".to_string(),
    };

    let title = format!("GitX-Tauri - {} [{}]", repo_name, branch_name);

    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(&title)
        .inner_size(1200.0, 800.0)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Repo root to open from CLI args (`gitx-tauri [path]`), resolved against `cwd`.
/// With no path, the cwd itself is used unless it is `/` or the home dir, so a
/// desktop launch doesn't pick up e.g. a dotfiles repo in `$HOME`.
fn cli_repo_root(app: &tauri::AppHandle, args: &[String], cwd: &Path) -> Option<String> {
    let explicit = args.iter().skip(1).find(|a| !a.starts_with('-'));
    let start = match explicit {
        Some(arg) => cwd.join(arg),
        None => {
            let home = app.path().home_dir().ok();
            if cwd.parent().is_none() || home.as_deref() == Some(cwd) {
                return None;
            }
            cwd.to_path_buf()
        }
    };

    match git2::Repository::discover(&start) {
        Ok(repo) => {
            let root: PathBuf = repo.workdir()?.to_path_buf();
            let root = root.canonicalize().unwrap_or(root);
            // No trailing slash, so the window label matches the launcher's.
            Some(
                root.to_string_lossy()
                    .trim_end_matches(['/', '\\'])
                    .to_string(),
            )
        }
        Err(e) => {
            if explicit.is_some() {
                eprintln!(
                    "gitx-tauri: not a git repository: {} ({e})",
                    start.display()
                );
            }
            None
        }
    }
}

fn open_cli_repo(app: &tauri::AppHandle, args: &[String], cwd: &Path) {
    if let Some(root) = cli_repo_root(app, args, cwd) {
        if let Err(e) = open_repo_window(app.clone(), root) {
            eprintln!("gitx-tauri: failed to open repository window: {e}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first. A second `gitx-tauri [path]` hands its
        // args to the running instance instead of starting another app.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
            open_cli_repo(app, &args, Path::new(&cwd));
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(RepoWatchers::default())
        .manage(LastRefresh::default())
        .setup(|app| {
            #[cfg(target_os = "linux")]
            appimage_desktop::integrate();
            let args: Vec<String> = std::env::args().collect();
            if let Ok(cwd) = std::env::current_dir() {
                open_cli_repo(app.handle(), &args, &cwd);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Destroyed => {
                window
                    .state::<RepoWatchers>()
                    .0
                    .lock()
                    .unwrap()
                    .remove(window.label());
                window.state::<LastRefresh>().forget(window.label());
            }
            // Safety net for changes the watcher can't see (filesystems
            // without inotify, an exhausted watch limit).
            tauri::WindowEvent::Focused(true) => {
                let watching = window
                    .state::<RepoWatchers>()
                    .0
                    .lock()
                    .unwrap()
                    .contains_key(window.label());
                let last = window.state::<LastRefresh>();
                if watching && last.due(window.label(), FOCUS_REFRESH_GAP) {
                    last.touch(window.label());
                    let _ = window.emit_to(window.label(), "repo-changed", ());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            clone_repository,
            is_git_repository,
            open_repository,
            open_repo_window,
            cli_install::cli_install_status,
            cli_install::install_cli,
            get_branches,
            get_remotes,
            get_remote_branches,
            get_tags,
            get_tag_commit,
            get_submodules,
            watch_repo,
            get_status,
            get_diff,
            stage_file,
            unstage_file,
            stage_hunk,
            unstage_hunk,
            discard_file,
            discard_hunk,
            ignore_file,
            create_commit,
            cherry_pick,
            is_ancestor_of_head,
            create_branch,
            create_tag,
            delete_tag,
            save_stash,
            get_stashes,
            apply_stash,
            pop_stash,
            drop_stash,
            get_stash_diff,
            checkout_branch,
            get_branch_delete_info,
            delete_local_branch,
            delete_remote_branch,
            fetch_remote,
            pull_remote,
            push_remote,
            get_commits,
            get_branch_head,
            get_commit_diff,
            get_commit_tree,
            get_file_content,
            get_file_blame,
            get_file_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitX-Tauri");
}

#[cfg(test)]
mod watch_tests {
    use super::*;
    use std::process::Command;

    fn root(path: &str, is_git_dir: bool) -> WatchRoot {
        WatchRoot {
            path: PathBuf::from(path),
            is_git_dir,
        }
    }

    fn noise(path: &str, roots: &[WatchRoot]) -> bool {
        is_watch_noise(Path::new(path), roots)
    }

    #[test]
    fn noise_only_counts_the_path_below_the_root() {
        // The repo itself lives under a folder called `build`.
        let roots = [root("/srv/build/repo", false)];
        assert!(!noise("/srv/build/repo/src/main.rs", &roots));
        assert!(!noise("/srv/build/repo/a.txt", &roots));
        assert!(noise("/srv/build/repo/target/debug/x", &roots));
        assert!(noise("/srv/build/repo/web/node_modules/a/b.js", &roots));
        // A file merely named like an ignored folder is not noise.
        assert!(!noise("/srv/build/repo/src/dist.rs", &roots));
    }

    #[test]
    fn git_dir_events_are_relevant_except_the_object_store() {
        let roots = [root("/srv/repo", false)];
        assert!(!noise("/srv/repo/.git/HEAD", &roots));
        assert!(!noise("/srv/repo/.git/index", &roots));
        assert!(!noise("/srv/repo/.git/refs/heads/main.lock", &roots));
        // A branch or tag named like an ignored folder must still refresh.
        assert!(!noise("/srv/repo/.git/refs/heads/build", &roots));
        assert!(!noise("/srv/repo/.git/refs/tags/dist", &roots));
        assert!(noise("/srv/repo/.git/objects/ab/cdef", &roots));
    }

    #[test]
    fn external_git_dirs_use_their_own_root() {
        let roots = [root("/w/linked", false), root("/m/.git", true)];
        assert!(!noise("/m/.git/refs/heads/build", &roots));
        assert!(!noise("/m/.git/worktrees/linked/index", &roots));
        assert!(noise("/m/.git/objects/ab/cdef", &roots));
    }

    #[test]
    fn events_outside_every_root_are_kept() {
        let roots = [root("/srv/repo", false)];
        assert!(!noise("/somewhere/else/target/x", &roots));
    }

    #[test]
    fn collect_batch_stops_at_the_max_wait() {
        let (tx, rx) = channel::<notify::Event>();
        let sender = std::thread::spawn(move || {
            for _ in 0..40 {
                if tx
                    .send(notify::Event::new(notify::EventKind::Other))
                    .is_err()
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        let first = rx.recv().unwrap();
        let started = Instant::now();
        let batch = collect_batch(
            &rx,
            first,
            Duration::from_millis(100),
            Duration::from_millis(300),
        );
        // Events arrive faster than the debounce, so only the cap ends the batch.
        assert!(started.elapsed() < Duration::from_millis(600));
        assert!(batch.len() >= 5 && batch.len() < 40, "got {}", batch.len());
        drop(rx);
        sender.join().unwrap();
    }

    #[test]
    fn collect_batch_returns_after_a_quiet_period() {
        let (_tx, rx) = channel::<notify::Event>();
        let started = Instant::now();
        let batch = collect_batch(
            &rx,
            notify::Event::new(notify::EventKind::Other),
            Duration::from_millis(50),
            Duration::from_secs(5),
        );
        assert_eq!(batch.len(), 1);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn focus_refresh_is_throttled() {
        let last = LastRefresh::default();
        let gap = Duration::from_secs(60);
        assert!(last.due("w", gap), "never refreshed yet");
        last.touch("w");
        assert!(!last.due("w", gap));
        assert!(last.due("other", gap));
        last.forget("w");
        assert!(last.due("w", gap));
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.txt"), "x\n").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-qm", "init"]);
    }

    #[test]
    fn watch_roots_cover_worktrees_and_submodules() {
        let base = std::env::temp_dir().join(format!("gitx-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let canon = |p: PathBuf| p.canonicalize().unwrap();

        // A normal repo needs only the working tree: its .git is inside it.
        let main = base.join("main");
        init_repo(&main);
        assert_eq!(watch_roots(&main).len(), 1);

        // A linked worktree also needs the main repo's .git.
        let linked = base.join("linked");
        git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);
        let roots = watch_roots(&linked);
        assert_eq!(roots.len(), 2, "{roots:?}");
        assert!(roots[1].is_git_dir);
        assert_eq!(canon(roots[1].path.clone()), canon(main.join(".git")));

        // A submodule also needs <super>/.git/modules/<name>.
        let lib = base.join("lib");
        init_repo(&lib);
        let sup = base.join("super");
        init_repo(&sup);
        git(
            &sup,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                lib.to_str().unwrap(),
                "sub",
            ],
        );
        let roots = watch_roots(&sup.join("sub"));
        assert_eq!(roots.len(), 2, "{roots:?}");
        assert!(roots[1].is_git_dir);
        assert_eq!(
            canon(roots[1].path.clone()),
            canon(sup.join(".git/modules/sub"))
        );

        // Events in those git dirs count; their object stores do not.
        let main_git = canon(main.join(".git"));
        let roots = watch_roots(&linked);
        assert!(!is_watch_noise(&main_git.join("refs/heads/build"), &roots));
        assert!(is_watch_noise(&main_git.join("objects/ab/cd"), &roots));

        let _ = std::fs::remove_dir_all(&base);
    }
}
