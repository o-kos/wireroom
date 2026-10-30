use dmd_source::Source;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub struct Hashing {
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<anyhow::Result<String>>>,
    unavailable: Option<String>,
    expected: String,
}
impl Hashing {
    pub fn start(source: Source, expected: String) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let result = thread::Builder::new()
            .name("signal-verification".into())
            .spawn(move || verify(source, &flag));
        let (worker, unavailable) = match result {
            Ok(worker) => (Some(worker), None),
            Err(error) => (
                None,
                Some(format!("Signal verification could not start: {error}")),
            ),
        };
        Self {
            cancel,
            worker,
            unavailable,
            expected,
        }
    }
    pub fn finish(&mut self) -> Option<String> {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(message) = self.unavailable.take() {
            return Some(message);
        }
        outcome(self.worker.take()?.join(), &self.expected)
    }
}
fn verify(source: Source, cancel: &AtomicBool) -> anyhow::Result<String> {
    let mut cursor = source.cursor()?;
    let before = cursor.file_metadata()?;
    let (actual, length) = digest(&mut cursor, cancel)?;
    let after = cursor.file_metadata()?;
    anyhow::ensure!(
        length == source.metadata().byte_size && stable(&before, &after)?,
        "Signal changed during verification"
    );
    Ok(actual)
}
fn stable(before: &std::fs::Metadata, after: &std::fs::Metadata) -> anyhow::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (before.ctime(), before.ctime_nsec()) != (after.ctime(), after.ctime_nsec()) {
            return Ok(false);
        }
    }
    Ok(before.len() == after.len() && before.modified()? == after.modified()?)
}
fn digest(reader: &mut impl Read, cancel: &AtomicBool) -> anyhow::Result<(String, u64)> {
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    let mut length = 0u64;
    loop {
        anyhow::ensure!(
            !cancel.load(Ordering::Relaxed),
            "Signal verification cancelled before completion"
        );
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        length = length
            .checked_add(n as u64)
            .ok_or_else(|| anyhow::anyhow!("Signal length overflow"))?;
        hash.update(&buffer[..n]);
    }
    Ok((format!("{:x}", hash.finalize()), length))
}
fn outcome(result: thread::Result<anyhow::Result<String>>, expected: &str) -> Option<String> {
    match result {
        Ok(Ok(actual)) if actual == expected => None,
        Ok(Ok(_)) => Some("Signal SHA-256 differs from the recording".into()),
        Ok(Err(error)) => Some(format!("Signal verification unavailable: {error}")),
        Err(_) => Some("Signal verification worker failed".into()),
    }
}
impl Drop for Hashing {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_outcomes_and_cancel_are_deterministic() {
        let flag = AtomicBool::new(false);
        let (hash, length) = digest(&mut &b"abc"[..], &flag).unwrap();
        assert_eq!(length, 3);
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(outcome(Ok(Ok(hash.clone())), &hash).is_none());
        assert!(
            outcome(Ok(Ok(hash)), "different")
                .unwrap()
                .contains("differs")
        );
        flag.store(true, Ordering::Relaxed);
        assert!(digest(&mut &b"abc"[..], &flag).is_err());
        assert!(
            outcome(Ok(Err(anyhow::anyhow!("read failed"))), "")
                .unwrap()
                .contains("read failed")
        );
        assert!(outcome(Err(Box::new("panic")), "").is_some());
    }
    #[test]
    fn changed_retained_metadata_cannot_certify_digest() {
        let path = std::env::temp_dir().join(format!("dmd-hash-metadata-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let before = file.metadata().unwrap();
        file.set_len(7).unwrap();
        assert!(!stable(&before, &file.metadata().unwrap()).unwrap());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn read_errors_are_diagnostics_and_workers_join_once() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected hash read failure"))
            }
        }
        assert!(
            digest(&mut Broken, &AtomicBool::new(false))
                .unwrap_err()
                .to_string()
                .contains("injected")
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let mut hashing = Hashing {
            cancel,
            worker: Some(thread::spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    thread::yield_now();
                }
                anyhow::bail!("cancelled")
            })),
            unavailable: None,
            expected: String::new(),
        };
        assert!(hashing.finish().unwrap().contains("cancelled"));
        assert!(hashing.finish().is_none());
    }
}
