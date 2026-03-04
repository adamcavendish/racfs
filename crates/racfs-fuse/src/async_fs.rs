//! Async FUSE filesystem implementation using experimental AsyncFilesystem trait

use async_trait::async_trait;
use fuser::Errno;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::instrument;
use tracing::{debug, error, warn};

use crate::cache::{DEFAULT_WRITE_BACK_FLUSH_SIZE, DEFAULT_WRITE_BACK_MAX_CHUNK, FuseCache};
use crate::error::Error;
use crate::inode_manager::InodeManager;
use bytes::Bytes;
use racfs_client::Client;

const TTL: Duration = Duration::from_secs(1);

/// Merge overlapping or adjacent write segments into contiguous (offset, data) runs.
fn merge_write_segments(mut segments: Vec<(u64, Vec<u8>)>) -> Vec<(u64, Vec<u8>)> {
    if segments.is_empty() {
        return segments;
    }
    segments.sort_by_key(|s| s.0);
    let mut result: Vec<(u64, Vec<u8>)> = Vec::new();
    for (off, data) in segments {
        if result.is_empty() {
            result.push((off, data));
            continue;
        }
        let last = match result.last_mut() {
            Some(l) => l,
            None => {
                result.push((off, data));
                continue;
            }
        };
        let last_end = last.0 + last.1.len() as u64;
        if off <= last_end {
            let start = (off.saturating_sub(last.0)) as usize;
            let end = start + data.len();
            if end > last.1.len() {
                last.1.resize(end, 0);
            }
            last.1[start..end].copy_from_slice(&data);
        } else {
            result.push((off, data));
        }
    }
    result
}

/// RACFS Async FUSE filesystem
pub struct RacfsAsyncFs {
    /// HTTP client for RACFS server
    client: Arc<Client>,
    /// Inode manager
    inodes: Arc<InodeManager>,
    /// Optional TTL cache for metadata and readdir
    cache: Arc<FuseCache>,
}

impl RacfsAsyncFs {
    /// Create a new RACFS async FUSE filesystem with default cache (1s TTL).
    pub fn new(server_url: &str) -> Result<Self, Error> {
        Ok(Self {
            client: Arc::new(Client::new(server_url)),
            inodes: Arc::new(InodeManager::new()),
            cache: Arc::new(FuseCache::with_default_ttl()),
        })
    }

    /// Create with a custom cache (e.g. different TTL or disabled by using zero capacity).
    pub fn with_cache(server_url: &str, cache: FuseCache) -> Result<Self, Error> {
        Ok(Self {
            client: Arc::new(Client::new(server_url)),
            inodes: Arc::new(InodeManager::new()),
            cache: Arc::new(cache),
        })
    }

    /// Convert path to string for API calls
    fn path_to_str(&self, path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    /// Parse permissions string (e.g., "rwxr-xr-x") to mode
    fn parse_permissions(&self, perms: &str) -> u32 {
        let mut mode = 0o000;

        let chars: Vec<char> = perms.chars().collect();
        if chars.len() >= 9 {
            // Owner
            if chars[0] == 'r' {
                mode |= 0o400;
            }
            if chars[1] == 'w' {
                mode |= 0o200;
            }
            if chars[2] == 'x' {
                mode |= 0o100;
            }
            // Group
            if chars[3] == 'r' {
                mode |= 0o040;
            }
            if chars[4] == 'w' {
                mode |= 0o020;
            }
            if chars[5] == 'x' {
                mode |= 0o010;
            }
            // Other
            if chars[6] == 'r' {
                mode |= 0o004;
            }
            if chars[7] == 'w' {
                mode |= 0o002;
            }
            if chars[8] == 'x' {
                mode |= 0o001;
            }
        }

        mode
    }

    /// Parse ISO 8601 timestamp to SystemTime
    fn parse_timestamp(&self, _timestamp: &str) -> SystemTime {
        // Simple parsing - in production, use chrono
        UNIX_EPOCH + Duration::from_secs(0)
    }

    /// Convert metadata to FileAttr
    fn metadata_to_attr(
        &self,
        inode: u64,
        metadata: &racfs_client::types::FileMetadataResponse,
    ) -> fuser::FileAttr {
        use fuser::{FileType, INodeNo};

        let kind = if metadata.file_type == "directory" {
            FileType::Directory
        } else if metadata.file_type == "symlink" {
            FileType::Symlink
        } else {
            FileType::RegularFile
        };

        let perm = self.parse_permissions(&metadata.permissions);
        let mtime = metadata
            .modified
            .as_ref()
            .map(|s| self.parse_timestamp(s))
            .unwrap_or(UNIX_EPOCH);

        fuser::FileAttr {
            ino: INodeNo(inode),
            size: metadata.size,
            blocks: metadata.size.div_ceil(512),
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm: perm as u16,
            nlink: if kind == FileType::Directory { 2 } else { 1 },
            uid: 1000,
            gid: 1000,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    /// Flush write-back buffer for a path: merge pending segments and send to server.
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn flush_write_back(&self, path: &Path) -> Result<(), Errno> {
        let segments = self.cache.take_pending_writes(path);
        if segments.is_empty() {
            return Ok(());
        }
        let path_str = self.path_to_str(path);
        let merged = merge_write_segments(segments);
        for (offset, data) in merged {
            let data_str = String::from_utf8_lossy(&data).to_string();
            self.client
                .write_file(&path_str, &data_str, Some(offset as i64))
                .await
                .map_err(|e| {
                    error!("flush_write_back failed for {:?}: {}", path, e);
                    Errno::EIO
                })?;
        }
        self.cache.invalidate(path);
        Ok(())
    }

    /// Called on file release (close): flush any pending writes for this inode.
    #[instrument(skip(self), fields(ino = ino))]
    pub async fn release_async(&self, ino: u64) -> Result<(), Errno> {
        if let Some(path) = self.inodes.get_path(ino) {
            let _ = self.flush_write_back(&path).await;
        }
        Ok(())
    }
}

// Note: AsyncFilesystem is experimental and not yet in stable fuser
// For now, we'll keep the sync implementation but structure the code
// to make migration easy when AsyncFilesystem is stabilized
#[async_trait]
pub trait AsyncFilesystemCompat: Send + Sync + 'static {
    async fn lookup_async(
        &self,
        parent: u64,
        name: &OsStr,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno>;

    async fn getattr_async(&self, ino: u64) -> Result<(Duration, fuser::FileAttr), Errno>;

    async fn read_async(&self, ino: u64, offset: u64, size: u32) -> Result<Bytes, Errno>;

    async fn readdir_async(
        &self,
        ino: u64,
        offset: u64,
    ) -> Result<Vec<(u64, fuser::FileType, String)>, Errno>;

    /// Create a new regular file.
    async fn create_async(
        &self,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno>;

    /// Create a directory.
    async fn mkdir_async(
        &self,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno>;

    /// Write data to a file.
    async fn write_async(&self, ino: u64, offset: u64, data: &[u8]) -> Result<u32, Errno>;

    /// Remove a file.
    async fn unlink_async(&self, parent: u64, name: &OsStr) -> Result<(), Errno>;

    /// Remove a directory.
    async fn rmdir_async(&self, parent: u64, name: &OsStr) -> Result<(), Errno>;

    /// Rename a file or directory.
    async fn rename_async(
        &self,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
    ) -> Result<(), Errno>;

    /// Change file mode (for setattr).
    async fn chmod_async(&self, ino: u64, mode: u32) -> Result<(Duration, fuser::FileAttr), Errno>;

    /// Truncate file to size (for setattr).
    async fn truncate_async(
        &self,
        ino: u64,
        size: u64,
    ) -> Result<(Duration, fuser::FileAttr), Errno>;

    /// Read the target of a symbolic link.
    async fn readlink_async(&self, ino: u64) -> Result<Vec<u8>, Errno>;

    /// Create a symbolic link at (parent, name) pointing to target (target string as bytes).
    async fn symlink_async(
        &self,
        parent: u64,
        name: &OsStr,
        target: &[u8],
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno>;

    /// Get extended attribute value.
    async fn getxattr_async(&self, ino: u64, name: &OsStr, size: u32) -> Result<Vec<u8>, Errno>;

    /// Set extended attribute.
    async fn setxattr_async(
        &self,
        ino: u64,
        name: &OsStr,
        value: &[u8],
        _flags: i32,
        _position: u32,
    ) -> Result<(), Errno>;

    /// List extended attribute names (null-separated bytes).
    async fn listxattr_async(&self, ino: u64, size: u32) -> Result<Vec<u8>, Errno>;

    /// Remove extended attribute.
    async fn removexattr_async(&self, ino: u64, name: &OsStr) -> Result<(), Errno>;
}

#[async_trait]
impl AsyncFilesystemCompat for RacfsAsyncFs {
    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy()))]
    async fn lookup_async(
        &self,
        parent: u64,
        name: &OsStr,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno> {
        let name_str = name.to_string_lossy();
        debug!("lookup_async: parent={}, name={}", parent, name_str);

        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;

        let path = parent_path.join(name_str.as_ref());

        if let Some(metadata) = self.cache.get_stat(&path) {
            let inode = self.inodes.allocate(&path);
            let attr = self.metadata_to_attr(inode, &metadata);
            return Ok((TTL, attr, fuser::Generation(0)));
        }

        let path_str = self.path_to_str(&path);

        match self.client.stat(&path_str).await {
            Ok(metadata) => {
                self.cache.put_stat(&path, metadata.clone());
                let inode = self.inodes.allocate(&path);
                let attr = self.metadata_to_attr(inode, &metadata);
                Ok((TTL, attr, fuser::Generation(0)))
            }
            Err(e) => {
                if matches!(
                    e,
                    racfs_client::Error::CircuitOpen | racfs_client::Error::Http { .. }
                ) && let Some(metadata) = self.cache.get_stat_stale(&path)
                {
                    warn!("lookup_async: serving stale cache (backend unreachable)");
                    let inode = self.inodes.allocate(&path);
                    let attr = self.metadata_to_attr(inode, &metadata);
                    return Ok((TTL, attr, fuser::Generation(0)));
                }
                warn!("lookup_async failed for {:?}: {}", path, e);
                Err(Errno::ENOENT)
            }
        }
    }

    #[instrument(skip(self), fields(ino = ino))]
    async fn getattr_async(&self, ino: u64) -> Result<(Duration, fuser::FileAttr), Errno> {
        debug!("getattr_async: ino={}", ino);

        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;

        if let Some(metadata) = self.cache.get_stat(&path) {
            let attr = self.metadata_to_attr(ino, &metadata);
            return Ok((TTL, attr));
        }

        let path_str = self.path_to_str(&path);
        match self.client.stat(&path_str).await {
            Ok(metadata) => {
                self.cache.put_stat(&path, metadata.clone());
                let attr = self.metadata_to_attr(ino, &metadata);
                Ok((TTL, attr))
            }
            Err(e) => {
                if matches!(
                    e,
                    racfs_client::Error::CircuitOpen | racfs_client::Error::Http { .. }
                ) && let Some(metadata) = self.cache.get_stat_stale(&path)
                {
                    warn!("getattr_async: serving stale cache (backend unreachable)");
                    let attr = self.metadata_to_attr(ino, &metadata);
                    return Ok((TTL, attr));
                }
                error!("getattr_async failed: {}", e);
                Err(Errno::ENOENT)
            }
        }
    }

    #[instrument(skip(self), fields(ino = ino, offset = offset, size = size))]
    async fn read_async(&self, ino: u64, offset: u64, size: u32) -> Result<Bytes, Errno> {
        debug!("read_async: ino={}, offset={}, size={}", ino, offset, size);

        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;

        // Serve from read-ahead buffer (file content cache) if we have the range.
        if let Some(cached) = self.cache.get_file_content(&path) {
            let start = offset as usize;
            let end = std::cmp::min(start + size as usize, cached.len());
            if start < cached.len() {
                return Ok(cached.slice(start..end));
            }
            return Ok(Bytes::new());
        }

        let path_str = self.path_to_str(&path);

        match self.client.read_file(&path_str).await {
            Ok(content) => {
                let bytes = Bytes::from(content.into_bytes());
                self.cache.put_file_content(&path, bytes.clone());
                let start = offset as usize;
                let end = std::cmp::min(start + size as usize, bytes.len());

                if start < bytes.len() {
                    Ok(bytes.slice(start..end))
                } else {
                    Ok(Bytes::new())
                }
            }
            Err(e) => {
                error!("read_async failed for {:?}: {}", path, e);
                Err(Errno::EIO)
            }
        }
    }

    #[instrument(skip(self), fields(ino = ino, offset = offset))]
    async fn readdir_async(
        &self,
        ino: u64,
        offset: u64,
    ) -> Result<Vec<(u64, fuser::FileType, String)>, Errno> {
        use fuser::FileType;

        debug!("readdir_async: ino={}, offset={}", ino, offset);

        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;

        let path_str = self.path_to_str(&path);

        let dir_list = if let Some(list) = self.cache.get_readdir(&path) {
            list
        } else {
            match self.client.read_dir(&path_str).await {
                Ok(list) => {
                    self.cache.put_readdir(&path, list.clone());
                    list
                }
                Err(e) => {
                    if matches!(
                        e,
                        racfs_client::Error::CircuitOpen | racfs_client::Error::Http { .. }
                    ) {
                        if let Some(list) = self.cache.get_readdir_stale(&path) {
                            warn!("readdir_async: serving stale cache (backend unreachable)");
                            list
                        } else {
                            error!("readdir_async failed for {:?}: {}", path, e);
                            return Err(Errno::EIO);
                        }
                    } else {
                        error!("readdir_async failed for {:?}: {}", path, e);
                        return Err(Errno::EIO);
                    }
                }
            }
        };

        let mut entries = vec![
            (ino, FileType::Directory, ".".to_string()),
            (ino, FileType::Directory, "..".to_string()),
        ];

        for entry in dir_list.entries {
            let entry_path = PathBuf::from(&entry.path);
            let entry_name = entry_path
                .file_name()
                .unwrap_or(OsStr::new(""))
                .to_string_lossy()
                .to_string();

            let entry_inode = self.inodes.allocate(&entry_path);
            let kind = match entry.file_type.as_deref() {
                Some("directory") => FileType::Directory,
                Some("symlink") => FileType::Symlink,
                _ => {
                    if entry.path.ends_with('/') {
                        FileType::Directory
                    } else {
                        FileType::RegularFile
                    }
                }
            };

            entries.push((entry_inode, kind, entry_name));
        }

        let result = entries.into_iter().skip(offset as usize).collect();
        Ok(result)
    }

    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy()))]
    async fn create_async(
        &self,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno> {
        let name_str = name.to_string_lossy();
        debug!("create_async: parent={}, name={}", parent, name_str);

        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let path = parent_path.join(name_str.as_ref());
        let path_str = self.path_to_str(&path);

        self.client.create_file(&path_str).await.map_err(|e| {
            warn!("create_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;

        self.cache.invalidate(&path);

        let metadata = self.client.stat(&path_str).await.map_err(|e| {
            warn!("create_async stat after create failed: {}", e);
            Errno::EIO
        })?;

        let inode = self.inodes.allocate(&path);
        let attr = self.metadata_to_attr(inode, &metadata);
        Ok((TTL, attr, fuser::Generation(0)))
    }

    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy(), mode = mode))]
    async fn mkdir_async(
        &self,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno> {
        let name_str = name.to_string_lossy();
        debug!("mkdir_async: parent={}, name={}", parent, name_str);

        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let path = parent_path.join(name_str.as_ref());
        let path_str = self.path_to_str(&path);

        self.client
            .mkdir(&path_str, Some(mode))
            .await
            .map_err(|e| {
                warn!("mkdir_async failed for {:?}: {}", path, e);
                Errno::EIO
            })?;

        self.cache.invalidate(&path);

        let metadata = self.client.stat(&path_str).await.map_err(|e| {
            warn!("mkdir_async stat after mkdir failed: {}", e);
            Errno::EIO
        })?;

        let inode = self.inodes.allocate(&path);
        let attr = self.metadata_to_attr(inode, &metadata);
        Ok((TTL, attr, fuser::Generation(0)))
    }

    #[instrument(skip(self, data), fields(ino = ino, offset = offset, len = data.len()))]
    async fn write_async(&self, ino: u64, offset: u64, data: &[u8]) -> Result<u32, Errno> {
        debug!(
            "write_async: ino={}, offset={}, len={}",
            ino,
            offset,
            data.len()
        );

        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);
        let len = data.len() as u32;

        if data.len() <= DEFAULT_WRITE_BACK_MAX_CHUNK {
            let total = self.cache.push_write_back(&path, offset, data.to_vec());
            if total >= DEFAULT_WRITE_BACK_FLUSH_SIZE {
                self.flush_write_back(&path).await?;
            }
            return Ok(len);
        }

        self.flush_write_back(&path).await?;
        let data_str = String::from_utf8_lossy(data).to_string();
        self.client
            .write_file(&path_str, &data_str, Some(offset as i64))
            .await
            .map_err(|e| {
                error!("write_async failed for {:?}: {}", path, e);
                Errno::EIO
            })?;
        self.cache.invalidate(&path);
        Ok(len)
    }

    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy()))]
    async fn unlink_async(&self, parent: u64, name: &OsStr) -> Result<(), Errno> {
        let name_str = name.to_string_lossy();
        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let path = parent_path.join(name_str.as_ref());
        let path_str = self.path_to_str(&path);

        if let Some(ino) = self.inodes.get_inode(&path) {
            self.inodes.remove(ino);
        }
        self.cache.invalidate(&path);
        self.client.remove(&path_str).await.map_err(|e| {
            warn!("unlink_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;
        Ok(())
    }

    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy()))]
    async fn rmdir_async(&self, parent: u64, name: &OsStr) -> Result<(), Errno> {
        let name_str = name.to_string_lossy();
        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let path = parent_path.join(name_str.as_ref());
        let path_str = self.path_to_str(&path);

        if let Some(ino) = self.inodes.get_inode(&path) {
            self.inodes.remove(ino);
        }
        self.cache.invalidate(&path);
        self.client.remove(&path_str).await.map_err(|e| {
            warn!("rmdir_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;
        Ok(())
    }

    #[instrument(skip(self), fields(parent = parent, name = %name.to_string_lossy(), newparent = newparent, newname = %newname.to_string_lossy()))]
    async fn rename_async(
        &self,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
    ) -> Result<(), Errno> {
        let name_str = name.to_string_lossy();
        let newname_str = newname.to_string_lossy();
        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let newparent_path = self.inodes.get_path(newparent).ok_or(Errno::ENOENT)?;
        let old_path = parent_path.join(name_str.as_ref());
        let new_path = newparent_path.join(newname_str.as_ref());
        let old_str = self.path_to_str(&old_path);
        let new_str = self.path_to_str(&new_path);

        self.client.rename(&old_str, &new_str).await.map_err(|e| {
            warn!(
                "rename_async failed {:?} -> {:?}: {}",
                old_path, new_path, e
            );
            Errno::EIO
        })?;

        self.cache.invalidate_rename(&old_path, &new_path);

        if let Some(ino) = self.inodes.get_inode(&old_path) {
            self.inodes.remove(ino);
        }
        self.inodes.allocate(&new_path);
        Ok(())
    }

    #[instrument(skip(self), fields(ino = ino, mode = mode))]
    async fn chmod_async(&self, ino: u64, mode: u32) -> Result<(Duration, fuser::FileAttr), Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);

        self.client.chmod(&path_str, mode).await.map_err(|e| {
            warn!("chmod_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;

        let metadata = self.client.stat(&path_str).await.map_err(|e| {
            warn!("chmod_async stat failed: {}", e);
            Errno::EIO
        })?;
        let attr = self.metadata_to_attr(ino, &metadata);
        Ok((TTL, attr))
    }

    #[instrument(skip(self), fields(ino = ino, size = size))]
    async fn truncate_async(
        &self,
        ino: u64,
        size: u64,
    ) -> Result<(Duration, fuser::FileAttr), Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);

        self.client.truncate(&path_str, size).await.map_err(|e| {
            warn!("truncate_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;

        self.cache.invalidate(&path);

        let metadata = self.client.stat(&path_str).await.map_err(|e| {
            warn!("truncate_async stat failed: {}", e);
            Errno::EIO
        })?;
        let attr = self.metadata_to_attr(ino, &metadata);
        Ok((TTL, attr))
    }

    #[instrument(skip(self), fields(ino = ino))]
    async fn readlink_async(&self, ino: u64) -> Result<Vec<u8>, Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);

        let metadata = self.client.stat(&path_str).await.map_err(|e| {
            warn!("readlink_async stat failed for {:?}: {}", path, e);
            Errno::EIO
        })?;

        let target = metadata.symlink_target.as_ref().ok_or(Errno::EINVAL)?;
        Ok(target.as_bytes().to_vec())
    }

    #[instrument(skip(self, target), fields(parent = parent, name = %name.to_string_lossy()))]
    async fn symlink_async(
        &self,
        parent: u64,
        name: &OsStr,
        target: &[u8],
    ) -> Result<(Duration, fuser::FileAttr, fuser::Generation), Errno> {
        let target_str = String::from_utf8_lossy(target).to_string();
        let name_str = name.to_string_lossy();
        let parent_path = self.inodes.get_path(parent).ok_or(Errno::ENOENT)?;
        let link_path = parent_path.join(name_str.as_ref());
        let link_path_str = self.path_to_str(&link_path);

        self.client
            .symlink(&target_str, &link_path_str)
            .await
            .map_err(|e| {
                warn!(
                    "symlink_async failed for {:?} -> {}: {}",
                    link_path, target_str, e
                );
                Errno::EIO
            })?;

        self.cache.invalidate(&link_path);

        let metadata = self.client.stat(&link_path_str).await.map_err(|e| {
            warn!("symlink_async stat after create failed: {}", e);
            Errno::EIO
        })?;

        let inode = self.inodes.allocate(&link_path);
        let attr = self.metadata_to_attr(inode, &metadata);
        Ok((TTL, attr, fuser::Generation(0)))
    }

    #[instrument(skip(self), fields(ino = ino, name = %name.to_string_lossy()))]
    async fn getxattr_async(&self, ino: u64, name: &OsStr, _size: u32) -> Result<Vec<u8>, Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);
        let name_str = name.to_string_lossy();

        let resp = self
            .client
            .get_xattr(&path_str, &name_str)
            .await
            .map_err(|e| {
                warn!("getxattr_async failed for {:?} {}: {}", path, name_str, e);
                Errno::ENOATTR
            })?;
        let value = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &resp.value)
            .map_err(|_| Errno::EIO)?;
        Ok(value)
    }

    #[instrument(skip(self, value), fields(ino = ino, name = %name.to_string_lossy()))]
    async fn setxattr_async(
        &self,
        ino: u64,
        name: &OsStr,
        value: &[u8],
        _flags: i32,
        _position: u32,
    ) -> Result<(), Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);
        let name_str = name.to_string_lossy();
        let value_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, value);

        self.client
            .set_xattr(&path_str, &name_str, &value_b64)
            .await
            .map_err(|e| {
                warn!("setxattr_async failed for {:?} {}: {}", path, name_str, e);
                Errno::EIO
            })?;
        self.cache.invalidate(&path);
        Ok(())
    }

    #[instrument(skip(self), fields(ino = ino))]
    async fn listxattr_async(&self, ino: u64, _size: u32) -> Result<Vec<u8>, Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);

        let resp = self.client.list_xattr(&path_str).await.map_err(|e| {
            warn!("listxattr_async failed for {:?}: {}", path, e);
            Errno::EIO
        })?;
        // Null-separated list as required by FUSE
        let mut out = Vec::new();
        for name in &resp.names {
            out.extend(name.as_bytes());
            out.push(0);
        }
        Ok(out)
    }

    #[instrument(skip(self), fields(ino = ino, name = %name.to_string_lossy()))]
    async fn removexattr_async(&self, ino: u64, name: &OsStr) -> Result<(), Errno> {
        let path = self.inodes.get_path(ino).ok_or(Errno::ENOENT)?;
        let path_str = self.path_to_str(&path);
        let name_str = name.to_string_lossy();

        self.client
            .remove_xattr(&path_str, &name_str)
            .await
            .map_err(|e| {
                warn!(
                    "removexattr_async failed for {:?} {}: {}",
                    path, name_str, e
                );
                Errno::ENOATTR
            })?;
        self.cache.invalidate(&path);
        Ok(())
    }
}

// Implement fuser's experimental AsyncFilesystem for use with TokioAdapter (read-only ops).
#[async_trait]
impl fuser::experimental::AsyncFilesystem for RacfsAsyncFs {
    async fn lookup(
        &self,
        _context: &fuser::experimental::RequestContext,
        parent: fuser::INodeNo,
        name: &OsStr,
    ) -> fuser::experimental::Result<fuser::experimental::LookupResponse> {
        let (ttl, attr, generation) = self.lookup_async(parent.0, name).await?;
        Ok(fuser::experimental::LookupResponse::new(
            ttl, attr, generation,
        ))
    }

    async fn getattr(
        &self,
        _context: &fuser::experimental::RequestContext,
        ino: fuser::INodeNo,
        _file_handle: Option<fuser::FileHandle>,
    ) -> fuser::experimental::Result<fuser::experimental::GetAttrResponse> {
        let (ttl, attr) = self.getattr_async(ino.0).await?;
        Ok(fuser::experimental::GetAttrResponse::new(ttl, attr))
    }

    async fn read(
        &self,
        _context: &fuser::experimental::RequestContext,
        ino: fuser::INodeNo,
        _file_handle: fuser::FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock: Option<fuser::LockOwner>,
        out_data: &mut Vec<u8>,
    ) -> fuser::experimental::Result<()> {
        let data = self.read_async(ino.0, offset, size).await?;
        out_data.extend_from_slice(&data);
        Ok(())
    }

    async fn readdir(
        &self,
        _context: &fuser::experimental::RequestContext,
        ino: fuser::INodeNo,
        _file_handle: fuser::FileHandle,
        offset: u64,
        mut builder: fuser::experimental::DirEntListBuilder<'_>,
    ) -> fuser::experimental::Result<()> {
        let entries = self.readdir_async(ino.0, offset).await?;
        for (i, (inode, kind, name)) in entries.into_iter().enumerate() {
            let next_offset = offset + i as u64 + 1;
            if builder.add(fuser::INodeNo(inode), next_offset, kind, name) {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_write_segments_empty() {
        assert!(merge_write_segments(vec![]).is_empty());
    }

    #[test]
    fn test_merge_write_segments_single() {
        let segs = vec![(0u64, b"hello".to_vec())];
        let out = merge_write_segments(segs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 0);
        assert_eq!(out[0].1.as_slice(), b"hello");
    }

    #[test]
    fn test_merge_write_segments_non_overlapping() {
        let segs = vec![(0u64, b"ab".to_vec()), (10u64, b"cd".to_vec())];
        let out = merge_write_segments(segs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1.as_slice(), b"ab");
        assert_eq!(out[1].1.as_slice(), b"cd");
    }

    #[test]
    fn test_merge_write_segments_adjacent() {
        let segs = vec![(0u64, b"hello".to_vec()), (5u64, b" world".to_vec())];
        let out = merge_write_segments(segs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 0);
        assert_eq!(out[0].1.as_slice(), b"hello world");
    }

    #[test]
    fn test_merge_write_segments_overlapping() {
        let segs = vec![(0u64, b"abc".to_vec()), (2u64, b"XYZ".to_vec())];
        let out = merge_write_segments(segs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 0);
        assert_eq!(out[0].1.as_slice(), b"abXYZ");
    }
}
