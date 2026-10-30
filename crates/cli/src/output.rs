use anyhow::{Context, ensure};
use dmd_protocol::{Descriptor, ResultEvent};
use dmd_replay::Recording;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

const FILE_BUDGET: u64 = 1 << 36;

pub struct Output {
    root: PathBuf,
    published: BTreeMap<String, u64>,
    bytes: u64,
}

fn directory(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(metadata.is_dir(), "Output ancestor is not a directory"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(path)?,
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

impl Output {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let root = PathBuf::from(path);
        // The caller controls root ancestry; only archive-controlled descendants are traversed.
        fs::create_dir_all(&root)?;
        directory(&root)?;
        Ok(Self {
            root,
            published: BTreeMap::new(),
            bytes: 0,
        })
    }

    pub fn temporary(&self) -> anyhow::Result<File> {
        Ok(tempfile::tempfile_in(&self.root)?)
    }

    fn destination(&self, path: &str) -> anyhow::Result<PathBuf> {
        dmd_protocol::validate_result_path(path)?;
        let mut destination = self.root.clone();
        let mut parts = path.split('/').peekable();
        while let Some(part) = parts.next() {
            destination.push(part);
            if parts.peek().is_some() {
                directory(&destination)?;
            }
        }
        Ok(destination)
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
        if let Some(previous) = self.published.get(path) {
            ensure!(*previous == size, "Conflicting published asset size");
            return Ok(());
        }
        // Asset validates lexical/platform names and exact archive size before filesystem use.
        let mut asset = recording.asset(descriptor)?;
        let destination = self.destination(path)?;
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => anyhow::bail!("Output destination already exists"),
        }
        ensure!(
            self.published.len() < 65536,
            "Published file budget exhausted"
        );
        let bytes = self
            .bytes
            .checked_add(size)
            .context("Output size overflow")?;
        ensure!(bytes <= FILE_BUDGET, "Output exceeds 64 GiB budget");
        let mut stage = tempfile::NamedTempFile::new_in(
            destination.parent().context("Missing output parent")?,
        )?;
        let mut buffer = [0; 8192];
        loop {
            let n = asset.read_chunk(&mut buffer)?;
            if n == 0 {
                break;
            }
            stage.write_all(&buffer[..n])?;
        }
        stage.as_file().sync_all()?;
        // into_temp_path drops the writing handle before publication.
        stage.into_temp_path().persist_noclobber(&destination)?;
        self.published.insert(path.to_owned(), size);
        self.bytes = bytes;
        Ok(())
    }
}
