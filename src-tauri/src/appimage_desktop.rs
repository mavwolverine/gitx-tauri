//! Desktop integration for the AppImage build on Linux.
//!
//! GNOME on Wayland ignores window icons and takes the dock/app-bar icon from
//! a `.desktop` file named after the app id (`gitx-tauri`). An AppImage carries
//! one internally, but the desktop can't see it, so on each launch we write a
//! user-level entry and icon that point at the `.AppImage` file.

use std::path::{Path, PathBuf};

const APP_ID: &str = "gitx-tauri";
/// Marks entries we wrote, so we never overwrite someone else's file.
const MARKER: &str = "X-GitX-Tauri-AppImage=true";
const ICON: &[u8] = include_bytes!("../icons/128x128@2x.png");

/// Writes/refreshes the desktop entry when running from an AppImage.
/// Best effort: failures are logged and otherwise ignored.
pub fn integrate() {
    let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) else {
        return;
    };
    if let Err(e) = write_entry(&appimage) {
        eprintln!("gitx-tauri: AppImage desktop integration failed: {e}");
    }
}

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
}

fn write_entry(appimage: &Path) -> std::io::Result<()> {
    // A .deb/.rpm install already provides the entry; a user-level one with
    // the same name would shadow it and redirect launches to the AppImage.
    if Path::new("/usr/share/applications")
        .join(format!("{APP_ID}.desktop"))
        .exists()
    {
        return Ok(());
    }
    let Some(data) = data_home() else {
        return Ok(());
    };

    let entry_path = data.join("applications").join(format!("{APP_ID}.desktop"));
    let existing = std::fs::read_to_string(&entry_path).ok();
    if existing.as_deref().is_some_and(|e| !e.contains(MARKER)) {
        return Ok(());
    }

    let icon_path = data
        .join("icons/hicolor/256x256/apps")
        .join(format!("{APP_ID}.png"));
    if std::fs::read(&icon_path).ok().as_deref() != Some(ICON) {
        std::fs::create_dir_all(icon_path.parent().unwrap())?;
        std::fs::write(&icon_path, ICON)?;
    }

    let entry = desktop_entry(appimage);
    // Only rewrite when something changed, e.g. the AppImage was moved.
    if existing.as_deref() != Some(entry.as_str()) {
        std::fs::create_dir_all(entry_path.parent().unwrap())?;
        std::fs::write(&entry_path, entry)?;
    }
    Ok(())
}

/// Quotes a path for the desktop entry `Exec` key (freedesktop spec).
fn exec_quote(path: &Path) -> String {
    let mut quoted = String::from("\"");
    for c in path.to_string_lossy().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    // `%` starts a field code, so a literal one is written as `%%`.
    quoted.replace('%', "%%")
}

fn desktop_entry(appimage: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=GitX-Tauri\n\
         Comment=Git GUI\n\
         Exec={} %f\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Development;RevisionControl;\n\
         StartupWMClass={APP_ID}\n\
         {MARKER}\n",
        exec_quote(appimage)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_quote_escapes_reserved_chars() {
        assert_eq!(
            exec_quote(Path::new("/opt/My Apps/a\"b$c`d\\e%f.AppImage")),
            r#""/opt/My Apps/a\"b\$c\`d\\e%%f.AppImage""#
        );
    }

    #[test]
    fn writes_entry_and_icon_and_respects_foreign_files() {
        let dir = std::env::temp_dir().join(format!("gitx-desktop-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("XDG_DATA_HOME", &dir);

        write_entry(Path::new("/tmp/GitX Tauri.AppImage")).unwrap();
        let entry_path = dir.join("applications/gitx-tauri.desktop");
        let entry = std::fs::read_to_string(&entry_path).unwrap();
        assert!(entry.contains("Exec=\"/tmp/GitX Tauri.AppImage\" %f\n"));
        assert!(entry.contains(MARKER));
        assert_eq!(
            std::fs::read(dir.join("icons/hicolor/256x256/apps/gitx-tauri.png")).unwrap(),
            ICON
        );

        // Moving the AppImage updates our entry.
        write_entry(Path::new("/new/place.AppImage")).unwrap();
        assert!(std::fs::read_to_string(&entry_path)
            .unwrap()
            .contains("Exec=\"/new/place.AppImage\" %f"));

        // An entry we didn't write is left alone.
        std::fs::write(&entry_path, "[Desktop Entry]\nName=Mine\n").unwrap();
        write_entry(Path::new("/tmp/other.AppImage")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&entry_path).unwrap(),
            "[Desktop Entry]\nName=Mine\n"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
