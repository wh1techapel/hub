//! Where the bridge runs: the folder Claude Code works in, and whether files written into Hub's
//! data folder from this process would land somewhere else.
//!
//! The folder: Claude Code sets `CLAUDE_PROJECT_DIR` in the environment of every stdio MCP server
//! it starts (code.claude.com/docs/en/mcp), the CLI and the desktop app's Code tab alike. That is
//! the session's project folder even when the process starts somewhere else, so it wins over the
//! working directory, which stays the fallback (older versions, other clients).
//!
//! Redirected writes: processes started by Claude's desktop app (an MSIX app) have their writes to
//! `%LOCALAPPDATA%` moved into the package's private folder, without having package identity
//! themselves (both checked on the owner's machine, 2026-11-08). A Hub started from there would
//! keep its data where the owner's Hub never looks, so the bridge does not start Hub then; it asks
//! the owner to open Hub. The check writes one empty file into the data folder and asks Windows
//! where it really went (`canonicalize` reads the final path of the open file).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folder Claude Code works in: `CLAUDE_PROJECT_DIR` when it names an absolute folder that
/// exists, otherwise the working directory.
pub fn working_folder(project_dir: Option<OsString>, cwd: PathBuf) -> PathBuf {
    project_dir.map(PathBuf::from).filter(|dir| dir.is_absolute() && dir.is_dir()).unwrap_or(cwd)
}

/// The folder for this process, read from its environment.
pub fn this_folder() -> PathBuf {
    working_folder(std::env::var_os("CLAUDE_PROJECT_DIR"), std::env::current_dir().unwrap_or_default())
}

/// Whether a file written into `data_dir` (or, when it does not exist yet, its parent) ends up in
/// another folder. When nothing can be written there the answer is yes: not starting Hub is the
/// safe mistake.
pub fn writes_redirected(data_dir: &Path) -> bool {
    let folder = if data_dir.is_dir() { data_dir } else { data_dir.parent().unwrap_or(data_dir) };
    let probe = folder.join(format!(".hub-mcp-probe-{}", std::process::id()));
    if std::fs::write(&probe, b"").is_err() {
        return true;
    }
    let landed = std::fs::canonicalize(&probe).ok();
    let _ = std::fs::remove_file(&probe);
    match landed {
        Some(landed) => !same_path(&landed, &probe),
        None => true,
    }
}

/// Two paths as Windows compares them: no `\\?\` prefix, either slash, any case.
fn same_path(a: &Path, b: &Path) -> bool {
    let plain = |path: &Path| {
        let text = path.to_string_lossy().replace('/', "\\").to_lowercase();
        text.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(text)
    };
    plain(a) == plain(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_folder_from_claude_code_wins_over_the_working_directory() {
        let project = std::env::temp_dir();
        assert_eq!(working_folder(Some(project.clone().into_os_string()), PathBuf::from(r"C:\Users\someone")), project);
    }

    #[test]
    fn the_working_directory_stays_when_the_variable_is_missing_relative_or_not_a_folder() {
        let cwd = std::env::temp_dir();
        assert_eq!(working_folder(None, cwd.clone()), cwd);
        assert_eq!(working_folder(Some("".into()), cwd.clone()), cwd);
        assert_eq!(working_folder(Some("relative\\repo".into()), cwd.clone()), cwd);
        assert_eq!(working_folder(Some(cwd.join("hub-mcp-no-such-folder-0b7c").into_os_string()), cwd.clone()), cwd);
    }

    #[test]
    fn a_plain_folder_is_not_redirected_and_the_probe_is_gone() {
        // %TEMP% is never moved into a package's folder, inside Claude's desktop app or not.
        let dir = tempfile::tempdir().unwrap();
        assert!(!writes_redirected(dir.path()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        // A data folder that does not exist yet is judged by its parent.
        assert!(!writes_redirected(&dir.path().join("Hub")));
    }

    #[test]
    fn paths_compare_the_way_windows_does() {
        assert!(same_path(Path::new(r"\\?\C:\Users\A\AppData\Local\Hub\x"), Path::new("c:/users/a/appdata/local/hub/x")));
        assert!(!same_path(
            Path::new(r"\\?\C:\Users\A\AppData\Local\Packages\Claude_x\LocalCache\Local\Hub\x"),
            Path::new(r"C:\Users\A\AppData\Local\Hub\x"),
        ));
    }

    /// Run by hand inside and outside Claude's desktop app:
    /// `cargo test -p hub-mcp --lib local_app_data_redirection -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn local_app_data_redirection() {
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("Hub");
        println!("{} redirected: {}", local.display(), writes_redirected(&local));
    }
}
