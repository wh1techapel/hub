//! A file's identity on its volume, so a file moved or renamed on the same drive is found again
//! (docs/architecture.md, "Files").
//!
//! Windows gives every file a 128-bit id on its volume (`FILE_ID_INFO`, NTFS and ReFS). Hub keeps
//! it beside the path; when the path stops working, `locate` opens the file by that id and asks
//! Windows where it is now. Across drives there is no way to follow a file: it is missing.
//!
//! Only metadata is read: files are opened for attributes, never for their content, and never
//! written.

use std::path::{Path, PathBuf};

/// Where a file lives in the volume's own terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub volume: u64,
    /// The 128-bit file id as 32 lowercase hex digits.
    pub file: String,
}

/// What Hub needs to know about a path before it keeps it.
#[derive(Debug, Clone)]
pub struct Probe {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub identity: Option<Identity>,
    /// On a drive that can be unplugged (a flash disk).
    pub removable: bool,
}

/// Look at a path: it must exist.
pub fn probe(path: &Path) -> std::io::Result<Probe> {
    let meta = std::fs::metadata(path)?;
    Ok(Probe {
        path: path.to_path_buf(),
        is_dir: meta.is_dir(),
        size: meta.is_file().then_some(meta.len()),
        identity: identify(path),
        removable: is_removable(path),
    })
}

/// How the library checks and follows files. The real one asks Windows; tests use a fake.
pub trait Finder {
    fn identify(&self, path: &Path) -> Option<Identity>;
    fn locate(&self, identity: &Identity) -> Option<PathBuf>;
}

/// Windows' own answers.
pub struct System;

impl Finder for System {
    fn identify(&self, path: &Path) -> Option<Identity> {
        identify(path)
    }

    fn locate(&self, identity: &Identity) -> Option<PathBuf> {
        locate(identity)
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, ExtendedFileIdType, FileIdInfo, GetDriveTypeW, GetFileInformationByHandleEx,
        GetFinalPathNameByHandleW, GetLogicalDrives, OpenFileById, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_128,
        FILE_ID_DESCRIPTOR, FILE_ID_DESCRIPTOR_0, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, VOLUME_NAME_DOS,
    };
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOVABLE;

    use super::Identity;

    /// A handle that closes itself.
    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW or OpenFileById and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    /// Open for attributes only (directories need backup semantics to be opened at all).
    fn open(path: &Path) -> Option<Handle> {
        let name = wide(path);
        // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call.
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                ptr::null_mut(),
            )
        };
        (handle != INVALID_HANDLE_VALUE && !handle.is_null()).then_some(Handle(handle))
    }

    fn id_info(handle: &Handle) -> Option<FILE_ID_INFO> {
        let mut info = FILE_ID_INFO::default();
        // SAFETY: `info` is a FILE_ID_INFO and the size passed is its size.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileIdInfo,
                (&mut info as *mut FILE_ID_INFO).cast(),
                size_of::<FILE_ID_INFO>() as u32,
            )
        };
        (ok != 0).then_some(info)
    }

    fn hex(id: &FILE_ID_128) -> String {
        id.Identifier.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn unhex(text: &str) -> Option<FILE_ID_128> {
        if text.len() != 32 {
            return None;
        }
        let mut identifier = [0u8; 16];
        for (index, byte) in identifier.iter_mut().enumerate() {
            *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
        }
        Some(FILE_ID_128 { Identifier: identifier })
    }

    pub fn identify(path: &Path) -> Option<Identity> {
        let handle = open(path)?;
        let info = id_info(&handle)?;
        Some(Identity { volume: info.VolumeSerialNumber, file: hex(&info.FileId) })
    }

    fn final_path(handle: &Handle) -> Option<PathBuf> {
        let mut buffer = vec![0u16; 1024];
        loop {
            // SAFETY: the buffer is writable for `len` UTF-16 units.
            let written = unsafe { GetFinalPathNameByHandleW(handle.0, buffer.as_mut_ptr(), buffer.len() as u32, VOLUME_NAME_DOS) };
            if written == 0 {
                return None;
            }
            if (written as usize) < buffer.len() {
                buffer.truncate(written as usize);
                break;
            }
            buffer.resize(written as usize + 1, 0);
        }
        let path = PathBuf::from(OsString::from_wide(&buffer));
        // "\\?\C:\..." back to "C:\..."; "\\?\UNC\server\share" back to "\\server\share".
        let text = path.to_string_lossy();
        Some(if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{rest}"))
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            PathBuf::from(rest)
        } else {
            path
        })
    }

    pub fn locate(identity: &Identity) -> Option<PathBuf> {
        let file_id = unhex(&identity.file)?;
        // SAFETY: GetLogicalDrives takes no arguments.
        let drives = unsafe { GetLogicalDrives() };
        for letter in 0..26u8 {
            if drives & (1 << letter) == 0 {
                continue;
            }
            let root = PathBuf::from(format!("{}:\\", char::from(b'A' + letter)));
            let Some(volume) = open(&root) else { continue };
            if id_info(&volume).map(|info| info.VolumeSerialNumber) != Some(identity.volume) {
                continue;
            }
            let descriptor = FILE_ID_DESCRIPTOR {
                dwSize: size_of::<FILE_ID_DESCRIPTOR>() as u32,
                Type: ExtendedFileIdType,
                Anonymous: FILE_ID_DESCRIPTOR_0 { ExtendedFileId: file_id },
            };
            // SAFETY: `descriptor` is a valid FILE_ID_DESCRIPTOR and the volume handle is open.
            let handle = unsafe {
                OpenFileById(
                    volume.0,
                    &descriptor,
                    FILE_READ_ATTRIBUTES,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    ptr::null(),
                    FILE_FLAG_BACKUP_SEMANTICS,
                )
            };
            if handle == INVALID_HANDLE_VALUE || handle.is_null() {
                return None;
            }
            return final_path(&Handle(handle));
        }
        None
    }

    pub fn is_removable(path: &Path) -> bool {
        let Some(root) = path.ancestors().last() else { return false };
        let mut root = root.as_os_str().to_os_string();
        if !root.to_string_lossy().ends_with('\\') {
            root.push("\\");
        }
        let name: Vec<u16> = root.encode_wide().chain(Some(0)).collect();
        // SAFETY: `name` is a NUL-terminated UTF-16 string.
        unsafe { GetDriveTypeW(name.as_ptr()) == DRIVE_REMOVABLE }
    }
}

#[cfg(windows)]
pub use win::{identify, is_removable, locate};

#[cfg(not(windows))]
pub fn identify(_path: &Path) -> Option<Identity> {
    None
}

#[cfg(not(windows))]
pub fn locate(_identity: &Identity) -> Option<PathBuf> {
    None
}

#[cfg(not(windows))]
pub fn is_removable(_path: &Path) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_renamed_file_keeps_its_identity_and_is_found_again() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("laporan.txt");
        std::fs::write(&first, b"vlan").unwrap();
        let identity = identify(&first).expect("an NTFS temp folder has file ids");
        assert_eq!(identity.file.len(), 32);

        std::fs::create_dir(dir.path().join("arsip")).unwrap();
        let moved = dir.path().join("arsip").join("laporan-final.txt");
        std::fs::rename(&first, &moved).unwrap();
        assert_eq!(identify(&moved), Some(identity.clone()));

        let found = locate(&identity).expect("found by id");
        assert_eq!(
            std::fs::canonicalize(found).unwrap(),
            std::fs::canonicalize(&moved).unwrap()
        );
    }

    #[test]
    fn folders_have_an_identity_and_missing_paths_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(identify(dir.path()).is_some());
        assert!(identify(&dir.path().join("nothing-here")).is_none());
        let gone = Identity { volume: 1, file: "0".repeat(32) };
        assert_eq!(locate(&gone), None);
        assert!(!is_removable(dir.path()));
    }
}
