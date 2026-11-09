//! The Gemini API key: where it is kept and how it is handled.
//!
//! It lives in Windows Credential Manager as a generic credential of the owner's account (the
//! target `Hub/gemini`), persisted on this computer only. It is read for each request and dropped
//! after; it travels only in the `x-goog-api-key` header; nothing returns it to the page, and its
//! `Debug` shows nothing. Hub's database, export, backups, and log never hold it.

use std::fmt;

/// An API key. Prints as `ApiKey(…)` so it cannot slip into a log by `{:?}`.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(…)")
    }
}

/// Why a typed key was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProblem {
    /// Empty, or too short or long to be a key.
    Length,
    /// Spaces or characters a key never has (a pasted sentence, a line break in the middle).
    Characters,
}

/// Shortest and longest key accepted. Google's keys are 39 characters today; the range leaves
/// room for other formats without accepting a pasted paragraph.
const KEY_MIN: usize = 20;
const KEY_MAX: usize = 200;

impl ApiKey {
    /// A key as typed or pasted: surrounding space trimmed, then checked for its shape only.
    /// Whether Google accepts it is for "Uji koneksi".
    pub fn parse(text: &str) -> Result<ApiKey, KeyProblem> {
        let key = text.trim();
        let count = key.chars().count();
        if !(KEY_MIN..=KEY_MAX).contains(&count) {
            return Err(KeyProblem::Length);
        }
        if !key.chars().all(|c| c.is_ascii_graphic()) {
            return Err(KeyProblem::Characters);
        }
        Ok(ApiKey(key.to_string()))
    }

    /// The key for the request header. The only way to read it.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// Where the key is kept. The app uses Credential Manager; tests use memory.
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Result<Option<ApiKey>, StoreError>;
    fn set(&self, key: &ApiKey) -> Result<(), StoreError>;
    /// Removing a key that is not there is not an error.
    fn delete(&self) -> Result<(), StoreError>;
}

/// The credential store failed (an operating system error code, never the key).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("credential store: {0}")]
pub struct StoreError(pub String);

/// A store in memory, for tests and the browser preview.
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<Option<ApiKey>>);

impl KeyStore for MemoryStore {
    fn get(&self) -> Result<Option<ApiKey>, StoreError> {
        Ok(self.0.lock().map_err(|_| StoreError("poisoned".into()))?.clone())
    }

    fn set(&self, key: &ApiKey) -> Result<(), StoreError> {
        *self.0.lock().map_err(|_| StoreError("poisoned".into()))? = Some(key.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), StoreError> {
        *self.0.lock().map_err(|_| StoreError("poisoned".into()))? = None;
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_store::CredentialStore;

#[cfg(windows)]
mod windows_store {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    use super::{ApiKey, KeyStore, StoreError};

    /// A generic credential in Windows Credential Manager, readable only by the owner's account.
    pub struct CredentialStore {
        target: Vec<u16>,
    }

    impl CredentialStore {
        /// `target` names the credential: `Hub/gemini` for the owner's Hub, a separate name for
        /// tests and smokes so they never touch the owner's key.
        pub fn new(target: &str) -> CredentialStore {
            CredentialStore { target: target.encode_utf16().chain(Some(0)).collect() }
        }
    }

    fn last_error(what: &str) -> StoreError {
        // SAFETY: reads the calling thread's last error code; no memory is touched.
        let code = unsafe { GetLastError() };
        StoreError(format!("{what} failed (Windows error {code})"))
    }

    impl KeyStore for CredentialStore {
        fn get(&self) -> Result<Option<ApiKey>, StoreError> {
            let mut found: *mut CREDENTIALW = std::ptr::null_mut();
            // SAFETY: `target` is a NUL-terminated UTF-16 string that outlives the call; `found`
            // receives a buffer Windows allocates, freed below with CredFree.
            let ok = unsafe { CredReadW(self.target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut found) };
            if ok == 0 {
                // SAFETY: as in last_error.
                let code = unsafe { GetLastError() };
                return if code == ERROR_NOT_FOUND { Ok(None) } else { Err(StoreError(format!("reading the key failed (Windows error {code})"))) };
            }
            // SAFETY: on success `found` points at a valid CREDENTIALW whose blob holds
            // `CredentialBlobSize` bytes; both are read before the buffer is freed.
            let bytes = unsafe {
                let credential = &*found;
                let blob = if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(credential.CredentialBlob, credential.CredentialBlobSize as usize).to_vec()
                };
                CredFree(found as *const c_void);
                blob
            };
            let text = String::from_utf8(bytes).map_err(|_| StoreError("the stored key is not text".into()))?;
            // A key that no longer parses (edited by hand in Windows) counts as no key.
            Ok(ApiKey::parse(&text).ok())
        }

        fn set(&self, key: &ApiKey) -> Result<(), StoreError> {
            let mut blob = key.expose().as_bytes().to_vec();
            let user: Vec<u16> = "Hub".encode_utf16().chain(Some(0)).collect();
            // SAFETY: an all-zero CREDENTIALW is valid (null pointers, zero sizes); the fields set
            // below point at buffers that live until CredWriteW returns, and Windows copies them.
            let ok = unsafe {
                let mut credential: CREDENTIALW = std::mem::zeroed();
                credential.Type = CRED_TYPE_GENERIC;
                credential.TargetName = self.target.as_ptr() as *mut u16;
                credential.CredentialBlobSize = blob.len() as u32;
                credential.CredentialBlob = blob.as_mut_ptr();
                credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
                credential.UserName = user.as_ptr() as *mut u16;
                CredWriteW(&credential, 0)
            };
            blob.fill(0);
            if ok == 0 { Err(last_error("saving the key")) } else { Ok(()) }
        }

        fn delete(&self) -> Result<(), StoreError> {
            // SAFETY: `target` is a NUL-terminated UTF-16 string that outlives the call.
            let ok = unsafe { CredDeleteW(self.target.as_ptr(), CRED_TYPE_GENERIC, 0) };
            if ok == 0 {
                // SAFETY: as in last_error.
                let code = unsafe { GetLastError() };
                if code != ERROR_NOT_FOUND {
                    return Err(StoreError(format!("removing the key failed (Windows error {code})")));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AIzaTestKeyForHubStudio0123456789abcd";

    #[test]
    fn keys_are_trimmed_and_shaped() {
        assert_eq!(ApiKey::parse(&format!("  {KEY}\n")).unwrap().expose(), KEY);
        assert_eq!(ApiKey::parse(""), Err(KeyProblem::Length));
        assert_eq!(ApiKey::parse("short"), Err(KeyProblem::Length));
        assert_eq!(ApiKey::parse(&"x".repeat(201)), Err(KeyProblem::Length));
        assert_eq!(ApiKey::parse("AIza key with spaces inside it 1234"), Err(KeyProblem::Characters));
        assert_eq!(ApiKey::parse("AIzaKeyWith\u{00e9}Accent0123456789"), Err(KeyProblem::Characters));
    }

    #[test]
    fn a_key_never_prints() {
        let key = ApiKey::parse(KEY).unwrap();
        assert_eq!(format!("{key:?}"), "ApiKey(…)");
        assert!(!format!("{:?}", Some(key)).contains("AIza"));
    }

    #[test]
    fn the_memory_store_keeps_replaces_and_forgets() {
        let store = MemoryStore::default();
        assert_eq!(store.get().unwrap(), None);
        store.set(&ApiKey::parse(KEY).unwrap()).unwrap();
        assert_eq!(store.get().unwrap().unwrap().expose(), KEY);
        store.delete().unwrap();
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    /// The real Credential Manager, under a name of its own that is removed at the end.
    #[cfg(windows)]
    #[test]
    fn credential_manager_keeps_replaces_and_forgets() {
        let target = format!("Hub-test/{}/gemini", uuid::Uuid::now_v7());
        let store = CredentialStore::new(&target);
        assert_eq!(store.get().unwrap(), None);
        store.set(&ApiKey::parse(KEY).unwrap()).unwrap();
        assert_eq!(store.get().unwrap().unwrap().expose(), KEY);
        let other = format!("{KEY}XYZ");
        store.set(&ApiKey::parse(&other).unwrap()).unwrap();
        assert_eq!(store.get().unwrap().unwrap().expose(), other, "a new key replaces the old");
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
        store.delete().unwrap();
    }
}
