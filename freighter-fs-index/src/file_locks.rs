use freighter_api_types::index::response::CrateVersion;
use freighter_api_types::index::{IndexError, IndexResult};
use freighter_api_types::storage::{Bytes, Metadata, MetadataStorageProvider};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use tempfile::NamedTempFile;
use tokio::sync::RwLock as AsyncRwLock;
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

/// Lowercase crate name -> lock for file access.
///
/// It holds a path to the json-lines metadata file to emphasise only locked access is allowed.
/// flock is not used here, because the directory is meant to be handled exclusively by
/// one index instance, and flock is meh anyway.
///
/// Strong `Arc` is held by current user of the path, so it's easier to clean up the hashmap
/// after last use of each lock.
pub(crate) struct AccessLocks<T> {
    locks: Mutex<HashMap<String, Weak<AsyncRwLock<T>>>>,
}

impl<T> AccessLocks<T> {
    pub fn new() -> Self {
        Self { locks: Mutex::default() }
    }

    pub fn rwlock_for_key(&self, key: &str, object: T) -> Arc<AsyncRwLock<T>> {
        match self.locks.lock().unwrap().entry(key.to_owned()) {
            Entry::Occupied(mut e) => if let Some(existing) = e.get().upgrade() {
                existing
            } else {
                let new_lock = Arc::new(AsyncRwLock::new(object));
                *e.get_mut() = Arc::downgrade(&new_lock);
                new_lock
            },
            Entry::Vacant(e) => {
                let new_lock = Arc::new(AsyncRwLock::new(object));
                e.insert(Arc::downgrade(&new_lock));
                new_lock
            },
        }
    }

    pub fn on_unlocked(&self, key: &str) {
        let mut locks = self.locks.lock().unwrap();
        // Can drop the entry in the shared hashtable after the last user is dropped
        if let Some(entry) = locks.get_mut(key) {
            if Weak::strong_count(entry) == 0 {
                locks.remove(key);
            }
        }
    }
}

/// A filesystem path that can be locked for access
pub(crate) struct CrateMetaPath<'a> {
    path: Option<Arc<AsyncRwLock<PathBuf>>>,
    key: String,
    locks: &'a AccessLocks<PathBuf>,
}

impl<'a> CrateMetaPath<'a> {
    pub fn new(locks: &'a AccessLocks<PathBuf>, key: String, file_path: PathBuf) -> Self {
        let lockable_path = locks.rwlock_for_key(&key, file_path);
        Self {
            path: Some(lockable_path),
            key,
            locks,
        }
    }

    pub async fn exclusive(&self) -> LockedMetaFile<RwLockWriteGuard<'_, PathBuf>> {
        LockedMetaFile {
            path: self.path.as_ref().unwrap().write().await,
        }
    }

    pub async fn shared(&self) -> LockedMetaFile<RwLockReadGuard<'_, PathBuf>> {
        LockedMetaFile {
            path: self.path.as_ref().unwrap().read().await,
        }
    }
}

impl Drop for CrateMetaPath<'_> {
    fn drop(&mut self) {
        let _ = self.path.take(); // drop refcount
        self.locks.on_unlocked(&self.key);
    }
}

pub(crate) struct LockedMetaFile<Guard> {
    path: Guard,
}

impl LockedMetaFile<RwLockReadGuard<'_, PathBuf>> {
    pub async fn deserialized(&self) -> IndexResult<Vec<CrateVersion>> {
        deserialize_data(&read_from_path(&self.path).await?)
    }
}

impl LockedMetaFile<RwLockWriteGuard<'_, PathBuf>> {
    pub async fn deserialized(&self) -> IndexResult<Vec<CrateVersion>> {
        deserialize_data(&read_from_path(&self.path).await?)
    }

    pub async fn replace(&self, data: &[CrateVersion]) -> IndexResult<()> {
        self.replace_file(&serialize_data(data)?).await
            .map_err(|e| IndexError::ServiceError(e.into()))
    }

    pub async fn create_or_append(&self, version: &CrateVersion) -> IndexResult<()> {
        self.create_or_append_file(&serialize_data(std::slice::from_ref(version))?).await
            .map_err(|e| IndexError::ServiceError(e.into()))
    }

    async fn create_or_append_file(&self, data: &[u8]) -> io::Result<()> {
        let path = &*self.path;
        let parent = path.parent().unwrap();
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        file.seek(io::SeekFrom::End(0))?;
        file.write_all(data)
    }

    async fn replace_file(&self, data: &[u8]) -> io::Result<()> {
        let path = &*self.path;
        let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(data)?;
        tmp.persist(path)?;
        Ok(())
    }
}

async fn read_from_path(path: &Path) -> IndexResult<Vec<u8>> {
    std::fs::read(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => IndexError::NotFound,
        _ => IndexError::ServiceError(e.into()),
    })
}

fn deserialize_data(json_lines: &[u8]) -> IndexResult<Vec<CrateVersion>> {
    json_lines
        .split(|&c| c == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<CrateVersion>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| IndexError::ServiceError(e.into()))
}

fn serialize_data(versions: &[CrateVersion]) -> IndexResult<Vec<u8>> {
    let mut json_lines = Vec::with_capacity(versions.len() * 128);
    for v in versions {
        serde_json::to_writer(&mut json_lines, v)
            .map_err(|e| IndexError::ServiceError(e.into()))?;
        json_lines.push(b'\n');
    }
    Ok(json_lines)
}
