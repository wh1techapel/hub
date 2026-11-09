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
//! where it really went (`canonicalize` reads the final path of the open file). That final path
//! spells every folder out in full, so it is compared with the probe's own path in full as well
//! (`GetLongPathNameW`, which follows no redirection): `C:\Users\RUNNER~1\...`, the 8.3 short
//! form GitHub's Windows runners give `%TEMP%`, is the same folder, not a redirection.

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
    let written = long_path(&probe);
    let _ = std::fs::remove_file(&probe);
    match landed {
        Some(landed) => !same_path(&landed, &written),
        None => true,
    }
}

/// `path` with every 8.3 short name (`RUNNER~1`) written out in full, or `path` as it is when
/// Windows cannot say. Unlike `canonicalize`, it keeps the path the process asked for: a
/// redirected write still shows up as a different folder.
#[cfg(windows)]
fn long_path(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 260];
    loop {
        let capacity = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: `wide` ends in a NUL, and Windows writes at most `capacity` units into `buffer`.
        let written = unsafe { GetLongPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), capacity) } as usize;
        match written {
            0 => return path.to_path_buf(),
            n if n < buffer.len() => return PathBuf::from(OsString::from_wide(&buffer[..n])),
            // Too small: `n` is the length the long path needs, its NUL included.
            n => buffer.resize(n, 0),
        }
    }
}

#[cfg(not(windows))]
fn long_path(path: &Path) -> PathBuf {
    path.to_path_buf()
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

    /// GitHub's Windows runners set `%TEMP%` to `C:\Users\RUNNER~1\...`. The folder's 8.3 short
    /// name is the same folder, so it is not a redirection either.
    #[cfg(windows)]
    #[test]
    fn a_folder_named_by_its_short_name_is_not_redirected() {
        let base = tempfile::tempdir().unwrap();
        let folder = base.path().join("a folder with a long name");
        std::fs::create_dir(&folder).unwrap();
        let short = short_path(&folder);
        assert!(!writes_redirected(&short));
        assert_eq!(long_path(&short), long_path(&folder));
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    }

    /// The 8.3 form of `path`, or `path` itself on a volume that makes no short names.
    #[cfg(windows)]
    fn short_path(path: &Path) -> PathBuf {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut buffer = vec![0u16; 1024];
        // SAFETY: `wide` ends in a NUL, and Windows writes at most the buffer's length into it.
        let written = unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), 1024) } as usize;
        assert!(written > 0 && written < buffer.len(), "GetShortPathNameW failed for {}", path.display());
        PathBuf::from(std::ffi::OsString::from_wide(&buffer[..written]))
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
