//! The few per-user registry values Hub writes: its notification id and its `hubapp://` link.
//! Nothing else, and only under `HKEY_CURRENT_USER\Software\Classes`.

#[cfg(windows)]
pub fn set_values(key: &str, values: &[(&str, String)]) -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    let wide = |text: &str| text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let path = wide(key);
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: plain Win32 registry calls with valid, NUL-terminated buffers that outlive them;
    // the key handle is closed below.
    unsafe {
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut handle,
            std::ptr::null_mut(),
        ) != ERROR_SUCCESS
        {
            return false;
        }
        let mut ok = true;
        for (name, value) in values {
            // An empty name sets the key's default value.
            let name = if name.is_empty() { None } else { Some(wide(name)) };
            let data = wide(value);
            ok &= RegSetValueExW(
                handle,
                name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            ) == ERROR_SUCCESS;
        }
        RegCloseKey(handle);
        ok
    }
}

#[cfg(not(windows))]
pub fn set_values(_key: &str, _values: &[(&str, String)]) -> bool {
    false
}
