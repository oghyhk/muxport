use fs2::FileExt;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InstanceLockError {
    #[error("another Muxport connector already owns instance lock {0}")]
    AlreadyRunning(PathBuf),
    #[error("connector instance lock I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// An OS-owned advisory lock that prevents two connector processes from
/// mutating the same local stores. The lock file is deliberately retained:
/// ownership is represented by the live OS lock, not file presence, so a
/// crash never requires deleting a potentially live process's marker.
pub struct InstanceLock {
    file: File,
    path: PathBuf,
}

impl InstanceLock {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, InstanceLockError> {
        let path = path.as_ref().to_path_buf();
        let parent = path
            .parent()
            .filter(|candidate| !candidate.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|source| InstanceLockError::Io {
            path: path.clone(),
            source,
        })?;

        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|source| InstanceLockError::Io {
            path: path.clone(),
            source,
        })?;

        if let Err(source) = file.try_lock_exclusive() {
            if source.kind() == std::io::ErrorKind::WouldBlock {
                return Err(InstanceLockError::AlreadyRunning(path));
            }
            return Err(InstanceLockError::Io { path, source });
        }

        if let Err(source) = write_owner_marker(&mut file) {
            let _ = FileExt::unlock(&file);
            return Err(InstanceLockError::Io {
                path: path.clone(),
                source,
            });
        }

        Ok(Self { file, path })
    }

    pub fn default_path_for(state_db: impl AsRef<Path>) -> PathBuf {
        let mut value = OsString::from(state_db.as_ref().as_os_str());
        value.push(".lock");
        PathBuf::from(value)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn write_owner_marker(file: &mut File) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(
        file,
        "pid={}\nstarted_at_ms={}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    )?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "muxport-instance-{}-{}.lock",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ))
    }

    #[test]
    fn rejects_a_second_owner_and_recovers_after_drop() {
        let path = unique_path();
        let first = InstanceLock::acquire(&path).unwrap();
        assert_eq!(first.path(), path.as_path());
        assert!(matches!(
            InstanceLock::acquire(&path),
            Err(InstanceLockError::AlreadyRunning(contended)) if contended == path
        ));

        drop(first);
        let reacquired = InstanceLock::acquire(&path).unwrap();
        let marker = std::fs::read_to_string(&path).unwrap();
        assert!(marker.contains(&format!("pid={}", std::process::id())));
        drop(reacquired);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn derives_lock_path_without_replacing_the_database_extension() {
        assert_eq!(
            InstanceLock::default_path_for("/var/lib/muxport/state.db"),
            PathBuf::from("/var/lib/muxport/state.db.lock")
        );
    }
}
