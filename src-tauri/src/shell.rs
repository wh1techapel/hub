//! Windows' own shell for files Hub knows: open with the default app, show in Explorer with the
//! file selected, and the standard Open dialog. Paths come from hub-core by file id, never from
//! the page; programs are shown in Explorer instead of being started (files::is_program).

use std::path::{Path, PathBuf};

/// COM for this thread (single-threaded apartment, as the shell expects), undone on drop.
pub struct ComThread(bool);

impl ComThread {
    pub fn join() -> Self {
        #[cfg(windows)]
        {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
            // SAFETY: plain COM initialisation of this thread; undone in Drop when it succeeded.
            let joined = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }.is_ok();
            return ComThread(joined);
        }
        #[allow(unreachable_code)]
        ComThread(false)
    }
}

impl Drop for ComThread {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx of this same thread.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Run `work` on a fresh thread that has joined COM, and wait for it.
pub fn on_com_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    std::thread::spawn(move || {
        let _com = ComThread::join();
        work()
    })
    .join()
    .ok()
}

#[cfg(windows)]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Open with the app Windows uses for it. False when Windows refused.
#[cfg(windows)]
pub fn open(path: &Path) -> bool {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let file = wide(path);
    // SAFETY: NUL-terminated UTF-16 strings that outlive the call. A result above 32 is success.
    let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(file.as_ptr()), None, None, SW_SHOWNORMAL) };
    result.0 as usize > 32
}

/// Open a web address in the owner's browser. Only http and https: the address was checked when
/// it was stored (hub-core learning.rs), and again here.
pub fn open_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) || url.chars().any(char::is_whitespace) {
        return false;
    }
    open(Path::new(url))
}

/// Open Explorer on the file's folder with the file selected. False when the path is gone.
#[cfg(windows)]
pub fn reveal(path: &Path) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{ILFree, SHOpenFolderAndSelectItems, SHParseDisplayName};
    let file = wide(path);
    let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
    // SAFETY: COM is joined by the caller (on_com_thread); the id list is freed before return.
    unsafe {
        if SHParseDisplayName(PCWSTR(file.as_ptr()), None, &mut item, 0, None).is_err() || item.is_null() {
            return false;
        }
        let shown = SHOpenFolderAndSelectItems(item, None, 0).is_ok();
        ILFree(Some(item));
        shown
    }
}

/// The standard Open dialog over `owner`. Empty when the owner cancelled.
#[cfg(windows)]
pub fn pick_files(owner: Option<isize>, many: bool, title: &str) -> Vec<PathBuf> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH,
    };

    // SAFETY: COM is joined by the caller; every string the dialog hands back is freed.
    unsafe {
        let Ok(dialog) = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) else {
            return Vec::new();
        };
        let mut options = dialog.GetOptions().unwrap_or_default() | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST;
        if many {
            options |= FOS_ALLOWMULTISELECT;
        }
        let _ = dialog.SetOptions(options);
        let _ = dialog.SetTitle(&HSTRING::from(title));
        let parent = owner.map(|hwnd| HWND(hwnd as *mut core::ffi::c_void));
        if dialog.Show(parent).is_err() {
            return Vec::new();
        }
        let Ok(items) = dialog.GetResults() else { return Vec::new() };
        let count = items.GetCount().unwrap_or(0);
        let mut paths = Vec::new();
        for index in 0..count {
            let Ok(item) = items.GetItemAt(index) else { continue };
            let Ok(name): windows::core::Result<PWSTR> = item.GetDisplayName(SIGDN_FILESYSPATH) else { continue };
            if let Ok(text) = name.to_string() {
                paths.push(PathBuf::from(text));
            }
            CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
        }
        paths
    }
}

/// The standard folder dialog over `owner`. None when the owner cancelled.
#[cfg(windows)]
pub fn pick_folder(owner: Option<isize>, title: &str) -> Option<PathBuf> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH};

    // SAFETY: COM is joined by the caller; the string the dialog hands back is freed.
    unsafe {
        let dialog = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().unwrap_or_default() | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
        let _ = dialog.SetOptions(options);
        let _ = dialog.SetTitle(&HSTRING::from(title));
        let parent = owner.map(|hwnd| HWND(hwnd as *mut core::ffi::c_void));
        dialog.Show(parent).ok()?;
        let item = dialog.GetResult().ok()?;
        let chosen: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = chosen.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(chosen.0 as *const core::ffi::c_void));
        path
    }
}

#[cfg(not(windows))]
pub fn pick_folder(_owner: Option<isize>, _title: &str) -> Option<PathBuf> {
    None
}

/// The standard Save dialog over `owner`, offering `name`. None when the owner cancelled.
#[cfg(windows)]
pub fn save_file(owner: Option<isize>, title: &str, name: &str, extension: &str) -> Option<PathBuf> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{FileSaveDialog, IFileSaveDialog, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, SIGDN_FILESYSPATH};

    // SAFETY: COM is joined by the caller; the string the dialog hands back is freed.
    unsafe {
        let dialog = CoCreateInstance::<_, IFileSaveDialog>(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().unwrap_or_default() | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT;
        let _ = dialog.SetOptions(options);
        let _ = dialog.SetTitle(&HSTRING::from(title));
        let _ = dialog.SetFileName(&HSTRING::from(name));
        let _ = dialog.SetDefaultExtension(&HSTRING::from(extension));
        let parent = owner.map(|hwnd| HWND(hwnd as *mut core::ffi::c_void));
        dialog.Show(parent).ok()?;
        let item = dialog.GetResult().ok()?;
        let chosen: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = chosen.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(chosen.0 as *const core::ffi::c_void));
        path
    }
}

#[cfg(not(windows))]
pub fn save_file(_owner: Option<isize>, _title: &str, _name: &str, _extension: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(windows))]
pub fn open(_path: &Path) -> bool {
    false
}

#[cfg(not(windows))]
pub fn reveal(_path: &Path) -> bool {
    false
}

#[cfg(not(windows))]
pub fn pick_files(_owner: Option<isize>, _many: bool, _title: &str) -> Vec<PathBuf> {
    Vec::new()
}
