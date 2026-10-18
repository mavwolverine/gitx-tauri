//! "Install Command-Line Tool": puts a small `gitx-tauri` launcher script on
//! PATH so `gitx-tauri [path]` works from a terminal.

use serde::Serialize;
use std::path::{Path, PathBuf};

const CLI_NAME: &str = "gitx-tauri";
/// Marks scripts we wrote, so we never overwrite someone else's file.
const SCRIPT_MARKER: &str = "# Installed by GitX-Tauri";

#[derive(Serialize)]
pub struct CliStatus {
    pub supported: bool,
    /// Where Install writes the launcher script.
    pub install_path: Option<String>,
    /// Where `gitx-tauri` is already available, if anywhere.
    pub installed_at: Option<String>,
    /// Our script is installed but launches a different binary (e.g. an old
    /// debug or relocated build).
    pub outdated: bool,
}

fn install_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(PathBuf::from("/usr/local/bin").join(CLI_NAME))
    } else if cfg!(target_os = "linux") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin").join(CLI_NAME))
    } else {
        None
    }
}

fn app_executable() -> Result<PathBuf, String> {
    // Inside an AppImage, current_exe() points into a temporary mount; the
    // stable path is the .AppImage file itself.
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(appimage));
    }
    std::env::current_exe().map_err(|e| e.to_string())
}

fn find_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(CLI_NAME))
        .find(|p| p.is_file())
}

fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

fn launcher_script(exe: &Path) -> String {
    // Detach so the terminal is free; a second run is handed to the running
    // instance by the single-instance plugin.
    format!(
        "#!/bin/sh\n{SCRIPT_MARKER}\nnohup {} \"$@\" >/dev/null 2>&1 &\n",
        sh_quote(exe)
    )
}

#[tauri::command]
pub fn cli_install_status() -> CliStatus {
    let target = install_path();
    let ours = target
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .filter(|script| script.contains(SCRIPT_MARKER));
    let outdated = match (&ours, app_executable()) {
        // Debug builds can't install (see install_cli), so don't offer it.
        (Some(script), Ok(exe)) if !cfg!(debug_assertions) => *script != launcher_script(&exe),
        _ => false,
    };
    let installed_at = target
        .clone()
        .filter(|p| p.is_file())
        .or_else(find_on_path)
        .map(|p| p.to_string_lossy().into_owned());
    CliStatus {
        supported: target.is_some(),
        install_path: target.map(|p| p.to_string_lossy().into_owned()),
        installed_at,
        outdated,
    }
}

#[tauri::command]
pub fn install_cli() -> Result<String, String> {
    // A debug binary loads its UI from the dev server (devUrl), so a CLI
    // pointing at it shows "connection refused" whenever `pnpm dev` isn't up.
    if cfg!(debug_assertions) {
        return Err(
            "This is a debug build. Install the command-line tool from a release build (`pnpm tauri build`)."
                .into(),
        );
    }

    let target = install_path()
        .ok_or("Installing the command-line tool isn't supported on this platform yet.")?;

    if let Ok(existing) = std::fs::read_to_string(&target) {
        if !existing.contains(SCRIPT_MARKER) {
            return Err(format!(
                "{} already exists and wasn't installed by GitX-Tauri; leaving it alone.",
                target.display()
            ));
        }
    }

    let script = launcher_script(&app_executable()?);
    write_script(&target, &script)?;

    Ok(format!(
        "Installed {}. Open a new terminal and run `{CLI_NAME}` inside a repo.",
        target.display()
    ))
}

#[cfg(target_os = "linux")]
fn write_script(target: &Path, script: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(target, script).map_err(|e| e.to_string())?;
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn write_script(target: &Path, script: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    // /usr/local/bin is often user-writable (Homebrew); try that first.
    let direct = std::fs::write(target, script)
        .and_then(|_| std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755)));
    if direct.is_ok() {
        return Ok(());
    }

    // Otherwise stage the script and install it with an admin prompt.
    let staged = std::env::temp_dir().join(format!("{CLI_NAME}-cli"));
    std::fs::write(&staged, script).map_err(|e| e.to_string())?;
    let dir = target.parent().unwrap_or(Path::new("/usr/local/bin"));
    let shell = format!(
        "mkdir -p {} && install -m 755 {} {}",
        sh_quote(dir),
        sh_quote(&staged),
        sh_quote(target)
    );
    let applescript = format!(
        "do shell script \"{}\" with administrator privileges",
        shell.replace('\\', r"\\").replace('"', "\\\"")
    );
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(applescript)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&staged);

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") {
            Err("Installation cancelled.".into())
        } else {
            Err(stderr.trim().to_string())
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn write_script(_target: &Path, _script: &str) -> Result<(), String> {
    Err("Installing the command-line tool isn't supported on this platform yet.".into())
}
