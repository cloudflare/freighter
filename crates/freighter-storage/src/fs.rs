use async_trait::async_trait;
use freighter_api_types::storage::{
    Bytes, Metadata, MetadataStorageProvider, StorageError, StorageResult,
};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub struct FsStorageProvider {
    root: PathBuf,
}

impl FsStorageProvider {
    pub fn new(root: PathBuf) -> StorageResult<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn abs_path(&self, path: &str) -> StorageResult<PathBuf> {
        let path = self.root.join(path);
        if !path.starts_with(&self.root) {
            return Err(StorageError::ServiceError(
                io::Error::from(io::ErrorKind::InvalidInput).into(),
            ));
        }
        Ok(path)
    }
}

#[async_trait]
impl MetadataStorageProvider for FsStorageProvider {
    async fn pull_file(&self, path: &str) -> StorageResult<Bytes> {
        let data = std::fs::read(self.root.join(path))?;
        Ok(data.into())
    }

    async fn put_file(&self, path: &str, file_bytes: Bytes, _meta: Metadata) -> StorageResult<()> {
        let path = self.abs_path(path)?;
        let parent = path.parent().unwrap();
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(&file_bytes)?;
        tmp.persist(path)
            .map_err(|e| StorageError::ServiceError(e.into()))?;
        Ok(())
    }

    async fn list_prefix(&self, path: &str) -> StorageResult<Vec<String>> {
        let start = self.abs_path(path)?;
        let mut out = Vec::new();
        append_dir(&start, &mut out)?;
        Ok(out)
    }

    async fn delete_file(&self, path: &str) -> StorageResult<()> {
        let path = self.abs_path(path)?;
        std::fs::remove_file(path)?;
        Ok(())
    }

    async fn healthcheck(&self) -> anyhow::Result<()> {
        if self.root.is_dir() {
            Ok(())
        } else {
            anyhow::bail!("root not a dir")
        }
    }
}

fn append_dir(path: &Path, out: &mut Vec<String>) -> StorageResult<()> {
    for e in std::fs::read_dir(path)? {
        let e = e?;
        let ty = e.file_type()?;
        if ty.is_dir() {
            append_dir(&e.path(), out)?;
        }
        if let Ok(p) = e.path().into_os_string().into_string() {
            out.push(p);
        }
    }
    Ok(())
}
