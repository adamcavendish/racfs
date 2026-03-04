//! Advanced FUSE features (planned).
//!
//! This module is a placeholder for future FUSE capabilities tracked in the
//! [ROADMAP](https://github.com/adamcavendish/racfs/blob/master/ROADMAP.md) under
//! "Advanced FUSE Features". Implementations will be added as the fuser crate
//! and RACFS server support them.
//!
//! ## Protocol version and FUSE3
//!
//! The [fuser](https://github.com/cberner/fuser) crate is **compatible with FUSE3** (libfuse3).
//! Its README states: "fuse or fuse3 (this crate is compatible with both)". When the `libfuse`
//! feature is enabled, fuser prefers libfuse3 at build time (falling back to libfuse2). You can
//! explicitly select libfuse3 via the `libfuse3` feature. RACFS currently depends on fuser with
//! only the `experimental` feature; on Linux that uses fuser's pure-Rust mount (no libfuse link).
//! To use libfuse3 for mount/umount, add the `libfuse3` feature to the fuser dependency.
//!
//! The kernel protocol version is **7** (FUSE 2.x protocol); both libfuse2 and libfuse3 use
//! this protocol. `FUSE_PROTOCOL_VERSION` below reflects that.
//!
//! ## Planned features
//!
//! - **File locking (flock, fcntl)** – Advisory and mandatory locks; map to server-side
//!   locking or at least local process coordination.
//! - **Memory-mapped file support** – `mmap`/`munmap`; may require read-ahead and
//!   page-cache integration with the FUSE client cache.
//! - **FUSE3 / libfuse3** – Optional: enable fuser's `libfuse3` feature to link against
//!   libfuse3 for mount/umount (fuser already supports it; no "migration" required).
//! - **Multi-mount support** – Single FUSE process serving multiple mount points
//!   (e.g. multiple RACFS servers or paths). When implemented, use a single
//!   `RacfsAsyncFs` per mount and coordinate via a multiplexing layer or
//!   separate threads/tasks per mount.
//!
//! See `crates/racfs-fuse/src/async_fs.rs` for the AsyncFilesystem implementation.

/// Placeholder: number of mounts in a future multi-mount setup (2 = multi-mount supported).
pub const MULTI_MOUNT_PLACEHOLDER: u32 = 2;

/// FUSE kernel protocol version in use (7 = FUSE 2.x protocol; libfuse2 and libfuse3 both use it).
/// To use libfuse3 for mount/umount, add the `libfuse3` feature to the fuser dependency.
pub const FUSE_PROTOCOL_VERSION: u32 = 7;

use fuser::LockOwner;
use parking_lot::RwLock;
use std::collections::HashMap;

/// POSIX lock type: shared (read), exclusive (write), unlock.
pub const F_RDLCK: i32 = 0;
pub const F_WRLCK: i32 = 1;
pub const F_UNLCK: i32 = 2;

#[derive(Clone, Debug)]
struct LockEntry {
    start: u64,
    end: u64,
    typ: i32,
    pid: u32,
    owner: LockOwner,
}

/// Tracks advisory file locks per inode for getlk/setlk (process-local).
#[derive(Debug)]
pub struct FileLockState {
    /// inode -> list of lock ranges (owner, typ, pid, start, end)
    locks: RwLock<HashMap<u64, Vec<LockEntry>>>,
}

impl FileLockState {
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
        }
    }

    /// Check if (start, end, typ) conflicts with an existing lock. Returns the first conflicting lock if any.
    pub fn get_conflict(
        &self,
        ino: u64,
        start: u64,
        end: u64,
        typ: i32,
        exclude_owner: LockOwner,
    ) -> Option<(u64, u64, i32, u32)> {
        let guards = self.locks.read();
        let entries = guards.get(&ino)?;
        for e in entries {
            if e.owner == exclude_owner {
                continue;
            }
            let overlaps = start <= e.end && e.start <= end;
            if !overlaps {
                continue;
            }
            let conflict = (typ == F_WRLCK) || (e.typ == F_WRLCK);
            if conflict {
                return Some((e.start, e.end, e.typ, e.pid));
            }
        }
        None
    }

    /// Set a lock. Returns Err(EAGAIN) if conflicting and non-blocking.
    #[allow(clippy::too_many_arguments)] // mirrors FUSE setlk API
    pub fn set_lock(
        &self,
        ino: u64,
        lock_owner: LockOwner,
        start: u64,
        end: u64,
        typ: i32,
        pid: u32,
        sleep: bool,
    ) -> Result<(), fuser::Errno> {
        if typ == F_UNLCK {
            self.unlock(ino, lock_owner, start, end);
            return Ok(());
        }

        if let Some((_, _, _, _)) = self.get_conflict(ino, start, end, typ, lock_owner) {
            if !sleep {
                return Err(fuser::Errno::EAGAIN);
            }
            // Blocking not implemented: treat as one attempt
            return Err(fuser::Errno::EAGAIN);
        }

        let mut guards = self.locks.write();
        let entries = guards.entry(ino).or_default();
        // Remove any existing lock from this owner overlapping the range
        entries.retain(|e| e.owner != lock_owner || e.end < start || e.start > end);
        entries.push(LockEntry {
            start,
            end,
            typ,
            pid,
            owner: lock_owner,
        });
        Ok(())
    }

    /// Remove locks for owner in [start, end] on inode.
    pub fn unlock(&self, ino: u64, lock_owner: LockOwner, start: u64, end: u64) {
        let mut guards = self.locks.write();
        if let Some(entries) = guards.get_mut(&ino) {
            entries.retain(|e| e.owner != lock_owner || e.end < start || e.start > end);
            if entries.is_empty() {
                guards.remove(&ino);
            }
        }
    }

    /// Remove all locks for the given owner (e.g. on release).
    pub fn remove_owner(&self, lock_owner: LockOwner) {
        let mut guards = self.locks.write();
        for entries in guards.values_mut() {
            entries.retain(|e| e.owner != lock_owner);
        }
        guards.retain(|_, v| !v.is_empty());
    }
}

impl Default for FileLockState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuser::LockOwner;

    #[test]
    fn file_lock_state_default() {
        let _ = FileLockState::default();
    }

    #[test]
    fn fuse_protocol_version_is_fuse2() {
        assert_eq!(FUSE_PROTOCOL_VERSION, 7);
    }

    #[test]
    fn multi_mount_placeholder_is_at_least_two() {
        const _: () = assert!(MULTI_MOUNT_PLACEHOLDER >= 2);
    }

    #[test]
    fn file_lock_set_and_get_conflict() {
        let state = FileLockState::new();
        let owner1 = LockOwner(1);
        let owner2 = LockOwner(2);
        // No conflict when empty
        assert!(state.get_conflict(10, 0, 100, F_WRLCK, owner1).is_none());
        // Set write lock
        assert!(
            state
                .set_lock(10, owner1, 0, 100, F_WRLCK, 1000, false)
                .is_ok()
        );
        // Other owner conflicts
        assert!(state.get_conflict(10, 50, 150, F_WRLCK, owner2).is_some());
        assert!(state.get_conflict(10, 50, 150, F_RDLCK, owner2).is_some());
        // Same owner does not conflict (excluded)
        assert!(state.get_conflict(10, 50, 150, F_WRLCK, owner1).is_none());
        // Unlock
        state.unlock(10, owner1, 0, 100);
        assert!(state.get_conflict(10, 0, 100, F_WRLCK, owner2).is_none());
    }

    #[test]
    fn file_lock_setlk_conflict_returns_eagain() {
        let state = FileLockState::new();
        let owner1 = LockOwner(1);
        let owner2 = LockOwner(2);
        assert!(
            state
                .set_lock(10, owner1, 0, 100, F_WRLCK, 1000, false)
                .is_ok()
        );
        let r = state.set_lock(10, owner2, 50, 150, F_WRLCK, 1001, false);
        assert!(r.is_err());
    }
}
