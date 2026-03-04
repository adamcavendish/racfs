//! Blocking adapter: implements fuser's sync Filesystem by delegating to RacfsAsyncFs via block_on.
//! Used by mount() for read-write; all logic lives in RacfsAsyncFs (AsyncFilesystemCompat).

use crate::advanced::{F_UNLCK, FileLockState};
use fuser::{
    FileHandle, Filesystem, FopenFlags, INodeNo, LockOwner, OpenFlags, ReplyAttr, ReplyCreate,
    ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyLock, ReplyWrite, ReplyXattr, Request,
    WriteFlags,
};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use tracing::debug;

use crate::async_fs::{AsyncFilesystemCompat, RacfsAsyncFs};
use crate::error::Error;

/// Blocking bridge: implements fuser::Filesystem by block_on'ing RacfsAsyncFs.
/// Kept crate-internal; mount() uses this for read-write.
pub(crate) struct BlockingAdapter {
    async_fs: Arc<RacfsAsyncFs>,
    runtime: Arc<tokio::runtime::Runtime>,
    lock_state: Arc<FileLockState>,
}

impl BlockingAdapter {
    pub fn new(server_url: &str) -> Result<Self, Error> {
        let async_fs = Arc::new(RacfsAsyncFs::new(server_url)?);
        let runtime = Arc::new(
            tokio::runtime::Runtime::new().map_err(|e| crate::error::Error::Io { source: e })?,
        );
        let lock_state = Arc::new(FileLockState::new());
        Ok(Self {
            async_fs,
            runtime,
            lock_state,
        })
    }
}

impl Filesystem for BlockingAdapter {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.lookup_async(parent, &name).await });

        match result {
            Ok((ttl, attr, generation)) => reply.entry(&ttl, &attr, generation),
            Err(e) => reply.error(e),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();

        let result = self
            .runtime
            .block_on(async move { async_fs.getattr_async(ino).await });

        match result {
            Ok((ttl, attr)) => reply.attr(&ttl, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();

        let result = self
            .runtime
            .block_on(async move { async_fs.readdir_async(ino, offset).await });

        match result {
            Ok(entries) => {
                for (i, (inode, kind, name)) in entries.iter().enumerate() {
                    let next_offset = offset + i as u64 + 1;
                    if reply.add(INodeNo(*inode), next_offset, *kind, name) {
                        break;
                    }
                }
                reply.ok();
            }
            Err(e) => reply.error(e),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();

        debug!("read: ino={}, offset={}, size={}", ino, offset, size);

        let result = self
            .runtime
            .block_on(async move { async_fs.read_async(ino, offset, size).await });

        match result {
            Ok(data) => reply.data(data.as_ref()),
            Err(e) => reply.error(e),
        }
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.create_async(parent, &name, 0o644, 0o022).await });

        match result {
            Ok((ttl, attr, generation)) => {
                reply.created(&ttl, &attr, generation, FileHandle(0), FopenFlags::empty());
            }
            Err(e) => reply.error(e),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        reply: ReplyEntry,
    ) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.mkdir_async(parent, &name, mode, umask).await });

        match result {
            Ok((ttl, attr, generation)) => reply.entry(&ttl, &attr, generation),
            Err(e) => reply.error(e),
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();
        let data = data.to_vec();

        let result = self
            .runtime
            .block_on(async move { async_fs.write_async(ino, offset, &data).await });

        match result {
            Ok(written) => reply.written(written),
            Err(e) => reply.error(e),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.unlink_async(parent, &name).await });

        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.rmdir_async(parent, &name).await });

        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        let parent = parent.0;
        let newparent = newparent.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();
        let newname = newname.to_os_string();

        let result = self.runtime.block_on(async move {
            async_fs
                .rename_async(parent, &name, newparent, &newname)
                .await
        });

        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _flags: OpenFlags,
        lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        if let Some(owner) = lock_owner {
            self.lock_state.remove_owner(owner);
        }
        let ino = ino.0;
        let async_fs = self.async_fs.clone();
        let result = self
            .runtime
            .block_on(async move { async_fs.release_async(ino).await });
        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        if let Some(size) = size {
            let ino = ino.0;
            let async_fs = self.async_fs.clone();

            let result = self
                .runtime
                .block_on(async move { async_fs.truncate_async(ino, size).await });

            match result {
                Ok((ttl, attr)) => reply.attr(&ttl, &attr),
                Err(e) => reply.error(e),
            }
            return;
        }

        if let Some(mode) = mode {
            let ino = ino.0;
            let async_fs = self.async_fs.clone();

            let result = self
                .runtime
                .block_on(async move { async_fs.chmod_async(ino, mode).await });

            match result {
                Ok((ttl, attr)) => reply.attr(&ttl, &attr),
                Err(e) => reply.error(e),
            }
        } else {
            let ino = ino.0;
            let async_fs = self.async_fs.clone();

            let result = self
                .runtime
                .block_on(async move { async_fs.getattr_async(ino).await });

            match result {
                Ok((ttl, attr)) => reply.attr(&ttl, &attr),
                Err(e) => reply.error(e),
            }
        }
    }

    fn readlink(&self, _req: &Request, ino: INodeNo, reply: ReplyData) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();

        let result = self
            .runtime
            .block_on(async move { async_fs.readlink_async(ino).await });

        match result {
            Ok(data) => reply.data(&data),
            Err(e) => reply.error(e),
        }
    }

    fn symlink(
        &self,
        _req: &Request,
        parent: INodeNo,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        let parent = parent.0;
        let async_fs = self.async_fs.clone();
        let name = link_name.to_os_string();
        let target_bytes = target.to_string_lossy().as_bytes().to_vec();

        let result = self
            .runtime
            .block_on(async move { async_fs.symlink_async(parent, &name, &target_bytes).await });

        match result {
            Ok((ttl, attr, generation)) => reply.entry(&ttl, &attr, generation),
            Err(e) => reply.error(e),
        }
    }

    fn getxattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, size: u32, reply: ReplyXattr) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.getxattr_async(ino, &name, size).await });

        match result {
            Ok(data) => {
                if size == 0 {
                    reply.size(data.len() as u32);
                } else if data.len() <= size as usize {
                    reply.data(&data);
                } else {
                    reply.error(fuser::Errno::ERANGE);
                }
            }
            Err(e) => reply.error(e),
        }
    }

    fn setxattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        name: &OsStr,
        value: &[u8],
        flags: i32,
        position: u32,
        reply: ReplyEmpty,
    ) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();
        let value = value.to_vec();

        let result = self.runtime.block_on(async move {
            async_fs
                .setxattr_async(ino, &name, &value, flags, position)
                .await
        });

        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();

        let result = self
            .runtime
            .block_on(async move { async_fs.listxattr_async(ino, size).await });

        match result {
            Ok(data) => {
                if size == 0 {
                    reply.size(data.len() as u32);
                } else if data.len() <= size as usize {
                    reply.data(&data);
                } else {
                    reply.error(fuser::Errno::ERANGE);
                }
            }
            Err(e) => reply.error(e),
        }
    }

    fn removexattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let ino = ino.0;
        let async_fs = self.async_fs.clone();
        let name = name.to_os_string();

        let result = self
            .runtime
            .block_on(async move { async_fs.removexattr_async(ino, &name).await });

        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn getlk(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        lock_owner: LockOwner,
        start: u64,
        end: u64,
        typ: i32,
        _pid: u32,
        reply: ReplyLock,
    ) {
        let conflict = self
            .lock_state
            .get_conflict(ino.0, start, end, typ, lock_owner);
        match conflict {
            Some((s, e, t, pid)) => reply.locked(s, e, t, pid),
            None => reply.locked(start, end, F_UNLCK, 0),
        }
    }

    fn setlk(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        lock_owner: LockOwner,
        start: u64,
        end: u64,
        typ: i32,
        pid: u32,
        sleep: bool,
        reply: ReplyEmpty,
    ) {
        match self
            .lock_state
            .set_lock(ino.0, lock_owner, start, end, typ, pid, sleep)
        {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }
}
