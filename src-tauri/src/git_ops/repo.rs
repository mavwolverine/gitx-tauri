use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use std::path::Path;

fn create_remote_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    callbacks.transfer_progress(|stats| {
        println!(
            "Received {}/{} objects ({} bytes)",
            stats.received_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
        true
    });
    callbacks
}

pub fn clone_repository(url: &str, path: &str) -> Result<(), git2::Error> {
    let callbacks = create_remote_callbacks();

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.clone(url, Path::new(path))?;
    Ok(())
}

pub fn is_git_repository(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

pub fn open_repository(path: &str) -> Result<Repository, git2::Error> {
    Repository::open(path)
}
