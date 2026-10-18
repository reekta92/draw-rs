use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Core atomic write: write data to a temp file in the same directory,
/// sync it (unix only), optionally set permissions (unix only), then
/// atomically rename over `path`.
fn atomic_write_impl(path: &Path, data: &[u8], mode: Option<u32>, replace: bool) -> Result<()> {
    #[cfg(unix)]
    let mut mode = mode;
    if replace {
        match fs::metadata(path) {
            Ok(metadata) => {
                if metadata.permissions().readonly() {
                    anyhow::bail!("file is read-only: {}", path.display());
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    mode = mode.or(Some(metadata.permissions().mode()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to inspect replacement destination"),
        }
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));
    let _guard = TempFileGuard::new(tmp.clone());
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode.unwrap_or(0o666));
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = options
        .open(&tmp)
        .with_context(|| format!("failed to create {}", tmp.display()))?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .context("failed to set permissions on temp file")?;
    }
    std::io::Write::write_all(&mut file, data)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    file.sync_all().context("failed to sync temp file")?;
    drop(file);

    if replace {
        fs::rename(&tmp, path)
    } else {
        // A hard link publishes complete bytes without overwriting a competing file.
        fs::hard_link(&tmp, path)
    }
    .with_context(|| format!("failed to publish {}", path.display()))?;
    #[cfg(unix)]
    fs::File::open(parent)?
        .sync_all()
        .context("failed to sync publication directory")?;
    Ok(())
}

/// Write `data` to `path` atomically using a temp file + rename.
/// On unix the temp file is synced before rename; permissions are umask-default.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    atomic_write_impl(path, data, None, true)
}

/// Publish complete bytes as a new file; never replace an existing destination.
pub fn atomic_create(path: &Path, data: &[u8]) -> Result<()> {
    atomic_write_impl(path, data, None, false)
}

struct TempFileGuard(PathBuf);
impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }
}
impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
