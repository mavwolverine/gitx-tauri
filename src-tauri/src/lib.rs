mod cli_install;
mod git_ops;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Mutex;
use std::time::Duration;
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

fn is_watch_noise(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| WATCH_IGNORED_DIRS.contains(&s))
            .unwrap_or(false)
    })
}

/// One active watcher per window label. Dropping a watcher closes its event
/// channel, which ends the matching emitter thread.
#[derive(Default)]
struct RepoWatchers(Mutex<HashMap<String, RecommendedWatcher>>);

#[tauri::command]
fn watch_repo(
    window: tauri::Window,
    watchers: tauri::State<RepoWatchers>,
    repo_path: String,
) -> Result<(), String> {
    let (tx, rx) = channel::<notify::Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if event.paths.iter().any(|p| !is_watch_noise(p)) {
                    let _ = tx.send(event);
                } else {
                    println!(
                        "[watch_repo] ignored (noise dir): {:?} {:?}",
                        event.kind, event.paths
                    );
                }
            }
            Err(e) => println!("[watch_repo] watcher error: {e}"),
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )
    .map_err(|e| e.to_string())?;

    watcher
        .watch(Path::new(&repo_path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    let label = window.label().to_string();

    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            // Coalesce bursts of events (e.g. a checkout touching many files)
            // into a single refresh instead of one per fs event.
            let mut batch = vec![first];
            while let Ok(event) = rx.recv_timeout(Duration::from_millis(300)) {
                batch.push(event);
            }
            println!(
                "[watch_repo] repo-changed: {} event(s) triggered it, e.g. {:?} {:?}",
                batch.len(),
                batch[0].kind,
                batch[0].paths
            );
            // Window::emit broadcasts to every window; target only the one
            // that owns this watcher.
            let _ = window.emit_to(window.label(), "repo-changed", ());
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
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            if let Ok(cwd) = std::env::current_dir() {
                open_cli_repo(app.handle(), &args, &cwd);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                window
                    .state::<RepoWatchers>()
                    .0
                    .lock()
                    .unwrap()
                    .remove(window.label());
            }
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
