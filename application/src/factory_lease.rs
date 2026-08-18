//! The smallest durable admission mechanism the local factory needs.
//!
//! Commands are already durable, idempotent, and crash recoverable through the
//! command journal; that machinery is the authority and is not duplicated here.
//! The only thing missing was a way to say "this Mac already has this request,
//! but the expensive local resource is occupied".
//!
//! One advisory lease file under the private machine data root answers that.
//! An unattended runner takes the lease before it invokes a coding harness,
//! Xcode, or the deterministic gates, and releases it whenever it stops using
//! them — most importantly while a verified candidate waits for the iPhone, so
//! a cable that is not plugged in never blocks unrelated source work.
//!
//! There is no queue, no scheduler, no new protocol record, and no new command
//! state: a waiting runner simply stays in its durable `queued` phase, which
//! every surface already presents as "Waiting to build…".

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const LEASE_FILE: &str = "factory.lease";
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// An exclusive claim on this Mac's expensive factory resources.
///
/// Released when dropped, when the process exits, and when the process is
/// killed — a crashed runner cannot strand the local factory.
#[derive(Debug)]
pub struct FactoryLease {
    file: File,
    path: PathBuf,
}

impl FactoryLease {
    pub fn path(machine_root: &Path) -> PathBuf {
        machine_root.join(LEASE_FILE)
    }

    /// Take the lease if it is free right now.
    ///
    /// Returns `Ok(None)` when another execution on this Mac holds it.
    pub fn try_acquire(machine_root: &Path) -> io::Result<Option<Self>> {
        let path = Self::path(machine_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // O_CLOEXEC matters: the harness, Xcode, and every other child of a
            // runner must not inherit the lease, or a long-lived descendant
            // would keep holding it after its runner finished.
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = options.open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "local factory lease is not a regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = io::Error::last_os_error();
                return match error.raw_os_error() {
                    Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => Ok(None),
                    _ => Err(error),
                };
            }
        }
        Ok(Some(Self { file, path }))
    }

    /// Wait until the lease is free, then take it.
    ///
    /// `on_first_wait` runs once, only if the lease was not immediately
    /// available, so a caller can durably publish that it is waiting before it
    /// starts polling. Returning an error from it abandons the wait.
    pub async fn acquire(
        machine_root: &Path,
        on_first_wait: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(lease) = Self::try_acquire(machine_root)? {
            return Ok(lease);
        }
        on_first_wait()?;
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            if let Some(lease) = Self::try_acquire(machine_root)? {
                return Ok(lease);
            }
        }
    }

    /// True when no execution on this Mac currently holds the lease.
    pub fn is_available(machine_root: &Path) -> io::Result<bool> {
        Ok(Self::try_acquire(machine_root)?.is_some())
    }
}

impl Drop for FactoryLease {
    fn drop(&mut self) {
        let _ = &self.path;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
        #[cfg(not(unix))]
        {
            let _ = &self.file;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_execution_holds_the_local_factory_at_a_time() {
        let root = tempfile::tempdir().unwrap();
        let held = FactoryLease::try_acquire(root.path()).unwrap();
        assert!(held.is_some());
        assert!(FactoryLease::try_acquire(root.path()).unwrap().is_none());
        assert!(!FactoryLease::is_available(root.path()).unwrap());
        drop(held);
        assert!(FactoryLease::try_acquire(root.path()).unwrap().is_some());
    }

    #[tokio::test]
    async fn a_waiting_runner_reports_once_and_then_starts_automatically() {
        let root = tempfile::tempdir().unwrap();
        let held = FactoryLease::try_acquire(root.path()).unwrap().unwrap();
        let announced = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = announced.clone();
        let path = root.path().to_path_buf();
        let waiter = tokio::spawn(async move {
            FactoryLease::acquire(&path, || {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
            .await
            .map(drop)
            .map_err(|error| error.to_string())
        });
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!waiter.is_finished(), "the second execution must wait");
        drop(held);
        waiter.await.unwrap().unwrap();
        assert_eq!(announced.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_immediately_available_lease_announces_nothing() {
        let root = tempfile::tempdir().unwrap();
        let lease = FactoryLease::acquire(root.path(), || {
            panic!("an available local factory must not announce waiting")
        })
        .await
        .unwrap();
        drop(lease);
    }
}
