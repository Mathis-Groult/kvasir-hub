//! `SecretStore` as one JSON file readable by the owner only.
//!
//! Hides: where and how the credentials rest on the device. The upgrade path is an
//! OS-keyring or Android-Keystore implementation of the same trait.

use hub_core::{Credentials, HubError, SecretStore};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// File-backed store. The file lives in the app's private data directory.
pub struct FileSecretStore {
    path: PathBuf,
}

impl FileSecretStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
        }
    }
}

fn io_error(path: &Path, e: impl std::fmt::Display) -> HubError {
    HubError::SecretStore(format!("{}: {e}", path.display()))
}

impl SecretStore for FileSecretStore {
    fn load(&self) -> Result<Option<Credentials>, HubError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| io_error(&self.path, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(&self.path, e)),
        }
    }

    fn save(&self, credentials: &Credentials) -> Result<(), HubError> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
        }
        let json = serde_json::to_vec(credentials).map_err(|e| io_error(&self.path, e))?;
        // Write a sibling then rename: a crash never leaves a half-written secret.
        let temp = self.path.with_extension("tmp");
        write_private(&temp, &json).map_err(|e| io_error(&temp, e))?;
        fs::rename(&temp, &self.path).map_err(|e| io_error(&self.path, e))
    }

    fn clear(&self) -> Result<(), HubError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error(&self.path, e)),
        }
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::File::create(path)?.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hub-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("secret.json")
    }

    #[test]
    fn empty_store_loads_none() {
        assert_eq!(FileSecretStore::new(scratch("empty")).load().unwrap(), None);
    }

    #[test]
    fn saved_credentials_load_back() {
        let store = FileSecretStore::new(scratch("roundtrip"));
        store.save(&Credentials::new("alice", "pw")).unwrap();
        assert_eq!(store.load().unwrap(), Some(Credentials::new("alice", "pw")));
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = scratch("mode");
        FileSecretStore::new(&path)
            .save(&Credentials::new("a", "b"))
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn clear_removes_and_is_idempotent() {
        let store = FileSecretStore::new(scratch("clear"));
        store.save(&Credentials::new("a", "b")).unwrap();
        store.clear().unwrap();
        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn corrupt_file_is_an_error_not_silent_signout() {
        let path = scratch("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not json").unwrap();
        assert!(matches!(
            FileSecretStore::new(&path).load(),
            Err(HubError::SecretStore(_))
        ));
    }
}
