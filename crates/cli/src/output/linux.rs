use anyhow::{Context, bail, ensure};
use dmd_protocol::{Descriptor, ResultEvent};
use dmd_replay::Recording;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const FILE_BUDGET: u64 = 1 << 36;

pub struct Output {
    root: File,
    published: BTreeMap<String, File>,
    bytes: u64,
    mount: u64,
}
fn cstr(name: &std::ffi::OsStr) -> anyhow::Result<CString> {
    Ok(CString::new(name.as_bytes())?)
}
fn open_at(parent: &File, name: &std::ffi::OsStr, flags: i32) -> anyhow::Result<File> {
    let name = cstr(name)?;
    // SAFETY: name is NUL terminated; parent is a live descriptor; mode is supplied.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    ensure!(
        fd >= 0,
        "Open output component: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: successful openat returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn protected(dir: &File, sticky_allowed: bool) -> anyhow::Result<()> {
    let meta = dir.metadata()?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    ensure!(
        meta.is_dir() && (meta.uid() == uid || meta.uid() == 0),
        "Output ancestry has an untrusted owner"
    );
    ensure!(
        meta.mode() & 0o022 == 0
            || (sticky_allowed && meta.uid() == 0 && meta.mode() & 0o1000 != 0),
        "Output ancestry permits relocation by other users"
    );
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: stat points to writable storage and fd is live.
    ensure!(
        // SAFETY: stat points to writable storage and fd is live.
        unsafe { libc::fstatfs(dir.as_raw_fd(), stat.as_mut_ptr()) } == 0,
        "Cannot inspect output filesystem"
    );
    // SAFETY: successful fstatfs initialized stat.
    let kind = unsafe { stat.assume_init() }.f_type;
    ensure!(
        matches!(kind, 0xef53 | 0x01021994 | 0x9123683e),
        "Output requires a supported local Linux filesystem"
    );
    let mut flags: libc::c_uint = 0;
    // SAFETY: FS_IOC_GETFLAGS writes one c_uint to valid storage for a live directory.
    let result = unsafe { libc::ioctl(dir.as_raw_fd(), libc::FS_IOC_GETFLAGS, &mut flags) };
    ensure!(
        result == 0 && case_sensitive(flags),
        "Cannot establish case-sensitive output names"
    );
    Ok(())
}
fn case_sensitive(flags: libc::c_uint) -> bool {
    flags & 0x40000000 == 0
}
fn private(dir: &File) -> anyhow::Result<()> {
    let metadata = dir.metadata()?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    ensure!(
        metadata.uid() == uid && metadata.mode() & 0o077 == 0,
        "Output directories must be private to the caller UID"
    );
    Ok(())
}
fn mount_id(file: &File) -> anyhow::Result<u64> {
    let mut stat = std::mem::MaybeUninit::<libc::statx>::uninit();
    // SAFETY: empty C string selects the live fd; stat is writable output storage.
    let result = unsafe {
        libc::statx(
            file.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            stat.as_mut_ptr(),
        )
    };
    ensure!(result == 0, "Output requires statx mount identity support");
    // SAFETY: successful statx initialized the output.
    let stat = unsafe { stat.assume_init() };
    ensure!(
        stat.stx_mask & libc::STATX_MNT_ID != 0,
        "Output mount identity is unavailable"
    );
    Ok(stat.stx_mnt_id)
}
fn directory(parent: &File, name: &std::ffi::OsStr, create: bool) -> anyhow::Result<File> {
    if create {
        let name = cstr(name)?;
        // SAFETY: valid descriptor and NUL terminated component.
        let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
        if result != 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    let child = open_at(parent, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
    protected(&child, true)?;
    Ok(child)
}
impl Output {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let absolute = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let mut root = File::open("/")?;
        protected(&root, true)?;
        for component in absolute.components() {
            match component {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => root = directory(&root, name, true)?,
                _ => bail!("Output path must not contain parent components"),
            }
        }
        protected(&root, false)?;
        private(&root)?;
        let mount = mount_id(&root)?;
        Ok(Self {
            root,
            published: BTreeMap::new(),
            bytes: 0,
            mount,
        })
    }
    pub fn temporary(&self) -> anyhow::Result<File> {
        open_at(
            &self.root,
            std::ffi::OsStr::new("."),
            libc::O_RDWR | libc::O_TMPFILE,
        )
        .context("Output filesystem must support anonymous spooling")
    }
    fn parent(&self, path: &str) -> anyhow::Result<(File, String)> {
        dmd_protocol::validate_result_path(path)?;
        let mut parts: Vec<_> = path.split('/').collect();
        let name = parts.pop().context("Empty result path")?.to_owned();
        let mut parent = self.root.try_clone()?;
        for part in parts {
            parent = directory(&parent, std::ffi::OsStr::new(part), true)?;
            private(&parent)?;
            ensure!(
                mount_id(&parent)? == self.mount,
                "Output descendant crosses a mount point"
            );
        }
        protected(&parent, false)?;
        Ok((parent, name))
    }
    pub fn materialize(
        &mut self,
        recording: &Recording,
        result: &ResultEvent,
    ) -> anyhow::Result<()> {
        for descriptor in result.binary.iter().flatten() {
            if let Descriptor::File { path, size, .. } = descriptor {
                self.file(recording, descriptor, path, *size)?;
            }
        }
        Ok(())
    }
    fn file(
        &mut self,
        recording: &Recording,
        descriptor: &Descriptor,
        path: &str,
        size: u64,
    ) -> anyhow::Result<()> {
        let (parent, name) = self.parent(path)?;
        if let Some(retained) = self.published.get(path) {
            let current = open_at(
                &parent,
                std::ffi::OsStr::new(&name),
                libc::O_RDONLY | libc::O_NONBLOCK,
            )?;
            ensure!(
                mount_id(&current)? == self.mount,
                "Published asset crosses a mount point"
            );
            let a = current.metadata()?;
            let b = retained.metadata()?;
            ensure!(
                a.is_file()
                    && a.dev() == b.dev()
                    && a.ino() == b.ino()
                    && a.len() == size
                    && a.nlink() == 1,
                "Published asset replaced or changed"
            );
            return Ok(());
        }
        ensure!(
            self.published.len() < 65536,
            "Published file budget exhausted"
        );
        self.bytes = self
            .bytes
            .checked_add(size)
            .context("Output size overflow")?;
        ensure!(self.bytes <= FILE_BUDGET, "Output exceeds 64 GiB budget");
        let mut stage = Stage::new(parent)?;
        let mut asset = recording.asset(descriptor)?;
        let mut buffer = [0; 8192];
        loop {
            let n = asset.read_chunk(&mut buffer)?;
            if n == 0 {
                break;
            }
            stage
                .file
                .as_mut()
                .context("Closed staging file")?
                .write_all(&buffer[..n])?;
        }
        let retained = stage.publish(&name)?;
        self.published.insert(path.to_owned(), retained);
        Ok(())
    }
}
struct Stage {
    parent: File,
    name: CString,
    unpublished: Option<File>,
    file: Option<File>,
}
impl Stage {
    fn new(parent: File) -> anyhow::Result<Self> {
        let name = CString::new(format!(
            ".dmd-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))?;
        let file = open_at(
            &parent,
            std::ffi::OsStr::from_bytes(name.as_bytes()),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        let mut stage = Self {
            parent,
            name,
            unpublished: None,
            file: Some(file),
        };
        stage.unpublished = Some(open_at(
            &stage.parent,
            std::ffi::OsStr::from_bytes(stage.name.as_bytes()),
            libc::O_PATH,
        )?);
        Ok(stage)
    }
    fn publish(&mut self, target: &str) -> anyhow::Result<File> {
        let file = self.file.take().context("Closed staging file")?;
        file.sync_all()?;
        let fd = file.into_raw_fd();
        // SAFETY: fd is owned and consumed here. Never retry close after an error.
        ensure!(unsafe { libc::close(fd) } == 0, "Closing payload failed");
        let retained = open_at(
            &self.parent,
            std::ffi::OsStr::from_bytes(self.name.as_bytes()),
            libc::O_RDONLY,
        )?;
        let target = CString::new(target)?;
        // SAFETY: both names are valid components under a live protected parent.
        ensure!(
            // SAFETY: valid component names and live protected parent descriptor.
            unsafe {
                libc::renameat2(
                    self.parent.as_raw_fd(),
                    self.name.as_ptr(),
                    self.parent.as_raw_fd(),
                    target.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            } == 0,
            "Exclusive payload publication failed: {}",
            std::io::Error::last_os_error()
        );
        self.unpublished = None;
        Ok(retained)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let Some(owned) = self.unpublished.as_ref().or(self.file.as_ref()) else {
            return;
        };
        let mut current = std::mem::MaybeUninit::<libc::statx>::uninit();
        // SAFETY: parent is live, name is NUL terminated and current is writable.
        let result = unsafe {
            libc::statx(
                self.parent.as_raw_fd(),
                self.name.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_INO,
                current.as_mut_ptr(),
            )
        };
        if result != 0 {
            return;
        }
        // SAFETY: successful statx initialized current without allocating a descriptor.
        let current = unsafe { current.assume_init() };
        let same = current.stx_mask & libc::STATX_INO != 0
            && owned.metadata().is_ok_and(|owned| {
                libc::makedev(current.stx_dev_major, current.stx_dev_minor) == owned.dev()
                    && current.stx_ino == owned.ino()
            });
        if !same {
            return;
        }
        // Protected ancestry excludes other-user substitution between this check and unlink.
        // Same-credential mutation remains outside the threat boundary.
        // SAFETY: removes only the retained unpublished staging identity, never a tree.
        unsafe {
            libc::unlinkat(self.parent.as_raw_fd(), self.name.as_ptr(), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inode_flags_use_kernel_output_width_and_mask() {
        let flags: libc::c_uint = libc::c_uint::from_ne_bytes(0x40000000u32.to_ne_bytes());
        assert_eq!(std::mem::size_of_val(&flags), 4);
        assert!(!case_sensitive(flags));
        assert!(!case_sensitive(flags | 0x80));
        assert!(case_sensitive(0x80));
    }
    #[test]
    fn stage_cleanup_under_fd_exhaustion_runs_in_isolated_process() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "output::linux::tests::stage_cleanup_under_fd_exhaustion_helper",
                "--test-threads=1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated helper failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
    fn exhaust_descriptors() -> Vec<File> {
        let mut files = Vec::new();
        loop {
            match File::open("/dev/null") {
                Ok(file) => files.push(file),
                Err(error) => {
                    assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                    return files;
                }
            }
        }
    }
    #[test]
    #[ignore = "runs only in a dedicated subprocess because FD limits are process-wide"]
    fn stage_cleanup_under_fd_exhaustion_helper() {
        let root = std::env::temp_dir().join(format!("dmd-stage-emfile-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let mut published = Stage::new(File::open(&root).unwrap()).unwrap();
        let reused = root.join(std::ffi::OsStr::from_bytes(published.name.as_bytes()));
        let retained = published.publish("result").unwrap();
        std::fs::write(&reused, b"unrelated").unwrap();
        let mut closed = Stage::new(File::open(&root).unwrap()).unwrap();
        let closed_name = root.join(std::ffi::OsStr::from_bytes(closed.name.as_bytes()));
        drop(closed.file.take());
        let replaced = Stage::new(File::open(&root).unwrap()).unwrap();
        let replaced_name = root.join(std::ffi::OsStr::from_bytes(replaced.name.as_bytes()));
        std::fs::rename(&replaced_name, root.join("original")).unwrap();
        std::fs::write(&replaced_name, b"replacement").unwrap();
        let parent = File::open(&root).unwrap();
        let next_name = root.join(format!(
            ".dmd-{}-{}",
            std::process::id(),
            NEXT.load(Ordering::Relaxed)
        ));
        let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: limit points to writable storage for the process resource limit.
        let result = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) };
        assert_eq!(result, 0);
        // SAFETY: successful getrlimit initialized limit.
        let mut limit = unsafe { limit.assume_init() };
        limit.rlim_cur = limit.rlim_cur.min(64);
        // SAFETY: valid resource and initialized limit; this process runs only this helper.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
        let mut files = exhaust_descriptors();
        drop(files.pop().unwrap());
        let error = Stage::new(parent).err().expect("second open must fail");
        assert!(
            error
                .to_string()
                .contains(&std::io::Error::from_raw_os_error(libc::EMFILE).to_string())
        );
        assert!(!next_name.exists());
        files.extend(exhaust_descriptors());
        drop(closed);
        drop(replaced);
        drop(published);
        assert!(!closed_name.exists());
        drop(files);
        assert_eq!(std::fs::read(&reused).unwrap(), b"unrelated");
        assert_eq!(std::fs::read(&replaced_name).unwrap(), b"replacement");
        assert_eq!(retained.metadata().unwrap().len(), 0);
        assert_eq!(std::fs::read(root.join("result")).unwrap(), b"");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn published_stage_never_cleans_reused_temporary_name() {
        let root = std::env::temp_dir().join(format!(
            "dmd-stage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let mut stage = Stage::new(File::open(&root).unwrap()).unwrap();
        let name = root.join(std::ffi::OsStr::from_bytes(stage.name.as_bytes()));
        stage
            .file
            .as_mut()
            .unwrap()
            .write_all(b"published")
            .unwrap();
        let retained = stage.publish("result").unwrap();
        assert!(stage.unpublished.is_none());
        std::fs::write(&name, b"unrelated").unwrap();
        drop(stage);
        assert_eq!(std::fs::read(&name).unwrap(), b"unrelated");
        assert_eq!(retained.metadata().unwrap().len(), 9);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unpublished_stage_cleans_only_retained_identity() {
        let root = std::env::temp_dir().join(format!(
            "dmd-stage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        for replace in [false, true] {
            let stage = Stage::new(File::open(&root).unwrap()).unwrap();
            let name = root.join(std::ffi::OsStr::from_bytes(stage.name.as_bytes()));
            if replace {
                std::fs::rename(&name, root.join("retained")).unwrap();
                std::fs::write(&name, b"unrelated").unwrap();
            }
            drop(stage);
            assert_eq!(name.exists(), replace);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
