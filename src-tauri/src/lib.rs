mod git_ops;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
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

#[tauri::command]
fn watch_repo(window: tauri::Window, repo_path: String) -> Result<(), String> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )
    .map_err(|e| e.to_string())?;

    watcher
        .watch(Path::new(&repo_path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    std::thread::spawn(move || {
        let _watcher = watcher;
        while rx.recv().is_ok() {
            let _ = window.emit("repo-changed", ());
        }
    });

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
fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    let repo = git_ops::open_repository(&path).map_err(|e| e.to_string())?;
    git_ops::checkout_branch(&repo, &branch_name).map_err(|e| e.to_string())
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
                "HEAD (detached)".to_string()
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            clone_repository,
            is_git_repository,
            open_repository,
            open_repo_window,
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
            checkout_branch,
            fetch_remote,
            pull_remote,
            get_commits,
            get_branch_head,
            get_commit_diff
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitX-Tauri");
}
