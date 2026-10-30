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
    let mut flags: libc::c_long = 0;
    // SAFETY: the ioctl writes one long to valid storage for a live directory.
    let result = unsafe { libc::ioctl(dir.as_raw_fd(), libc::FS_IOC_GETFLAGS, &mut flags) };
    ensure!(
        result == 0 && flags & 0x40000000 == 0,
        "Cannot establish case-sensitive output names"
    );
    Ok(())
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
        Ok(Self {
            parent,
            name,
            file: Some(file),
        })
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
        Ok(retained)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        // SAFETY: removes only our exclusive unpublished staging name, never a tree.
        unsafe {
            libc::unlinkat(self.parent.as_raw_fd(), self.name.as_ptr(), 0);
        }
    }
}
