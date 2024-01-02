use freighter_api_types::index::response::CrateVersion;
use freighter_api_types::index::{IndexError, IndexResult};
use freighter_api_types::storage::{Bytes, Metadata, MetadataStorageProvider};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::RwLock as AsyncRwLock;
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};
use freighter_api_types::storage::{MetadataStorageProvider, Bytes, Metadata};

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
    rel_path: Option<Arc<AsyncRwLock<String>>>,
    key: String,
    fs: &'a (dyn MetadataStorageProvider + Send + Sync),
    locks: &'a AccessLocks<String>,
}

impl<'a> CrateMetaPath<'a> {
    pub fn new(fs: &'a (dyn MetadataStorageProvider + Send + Sync), locks: &'a AccessLocks<String>, key: String, file_rel_path: String) -> Self {
        let lockable_path = locks.rwlock_for_key(&key, file_rel_path);
        Self {
            fs,
            rel_path: Some(lockable_path),
            key,
            locks,
        }
    }

    pub async fn exclusive(&self) -> LockedMetaFile<'a, RwLockWriteGuard<'_, String>> {
        LockedMetaFile {
            fs: self.fs,
            rel_path: self.rel_path.as_ref().unwrap().write().await,
        }
    }

    pub async fn shared(&self) -> LockedMetaFile<'a, RwLockReadGuard<'_, String>> {
        LockedMetaFile {
            fs: self.fs,
            rel_path: self.rel_path.as_ref().unwrap().read().await,
        }
    }
}

impl Drop for CrateMetaPath<'_> {
    fn drop(&mut self) {
        let _ = self.rel_path.take(); // drop refcount
        self.locks.on_unlocked(&self.key);
    }
}

pub(crate) struct LockedMetaFile<'a, Guard> {
    rel_path: Guard,
    fs: &'a (dyn MetadataStorageProvider + Send + Sync),
}

impl LockedMetaFile<'_, RwLockReadGuard<'_, String>> {
    pub async fn deserialized(&self) -> IndexResult<Vec<CrateVersion>> {
        deserialize_data(&self.fs.pull_file(&self.rel_path).await?)
    }
}

impl LockedMetaFile<'_, RwLockWriteGuard<'_, String>> {
    pub async fn deserialized(&self) -> IndexResult<Vec<CrateVersion>> {
        deserialize_data(&self.fs.pull_file(&self.rel_path).await?)
    }

    pub async fn replace(&self, data: &[CrateVersion]) -> IndexResult<()> {
        let bytes = serialize_data(data)?;
        let meta = Metadata {
            content_type: Some("application/json"),
            content_length: Some(bytes.len()),
            cache_control: None,
            content_encoding: None,
            sha256: None,
            kv: Default::default()
        };
        self.fs.put_file(&self.rel_path, bytes, meta).await
            .map_err(|e| IndexError::ServiceError(e.into()))
    }

    pub async fn create_or_append(&self, version: &CrateVersion) -> IndexResult<()> {
        let meta = Metadata {
            content_type: Some("application/json"),
            content_length: None,
            cache_control: None,
            content_encoding: None,
            sha256: None,
            kv: Default::default()
        };
        self.fs.create_or_append_file(&self.rel_path, serialize_data(std::slice::from_ref(version))?, meta).await
            .map_err(|e| IndexError::ServiceError(e.into()))
    }
}

fn deserialize_data(json_lines: &[u8]) -> IndexResult<Vec<CrateVersion>> {
    json_lines
        .split(|&c| c == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<CrateVersion>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| IndexError::ServiceError(e.into()))
}

fn serialize_data(versions: &[CrateVersion]) -> IndexResult<Bytes> {
    let mut json_lines = Vec::with_capacity(versions.len() * 128);
    for v in versions {
        serde_json::to_writer(&mut json_lines, v)
            .map_err(|e| IndexError::ServiceError(e.into()))?;
        json_lines.push(b'\n');
    }
    Ok(json_lines.into())
}
