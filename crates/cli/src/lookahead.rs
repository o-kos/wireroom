use anyhow::{Context, ensure};
use dmd_protocol::{Context as EventContext, Event, Writer};
use dmd_replay::{Journal, Record};
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

pub enum Ending {
    Boundary { from: u64, to: u64 },
    Marker,
    Eof,
    Suffix,
}
pub struct Unit {
    pub file: File,
    pub ending: Ending,
}
pub struct Lookahead {
    requests: Option<SyncSender<File>>,
    pub results: Receiver<anyhow::Result<Unit>>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Lookahead {
    pub fn new(journal: Journal, context: EventContext) -> anyhow::Result<Self> {
        Self::with_limit(journal, context, 256 * 1024 * 1024)
    }
    pub(crate) fn with_limit(
        mut journal: Journal,
        context: EventContext,
        limit: u64,
    ) -> anyhow::Result<Self> {
        let (requests, input) = mpsc::sync_channel::<File>(1);
        let (output, results) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        journal.set_cancellation(cancel.clone());
        let worker = thread::Builder::new()
            .name("replay-lookahead".into())
            .spawn(move || {
                let mut suffix = false;
                while let Ok(file) = input.recv() {
                    let result = spool(&mut journal, file, context, &mut suffix, limit);
                    let failed = result.is_err();
                    if output.send(result).is_err() || failed {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            results,
            cancel,
            worker: Some(worker),
        })
    }
    #[cfg(test)]
    pub(crate) fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
    #[cfg(test)]
    pub(crate) fn wait_finished(&self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            assert!(std::time::Instant::now() < deadline);
            thread::yield_now();
        }
    }
    pub fn finish(&mut self) -> anyhow::Result<()> {
        self.cancel.store(true, Ordering::Relaxed);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Lookahead worker failed"))?;
        }
        for result in self.results.try_iter() {
            if let Err(error) = result
                && !matches!(
                    error.downcast_ref::<dmd_replay::Error>(),
                    Some(dmd_replay::Error::Cancelled)
                )
            {
                return Err(error);
            }
        }
        Ok(())
    }
    pub fn request(&self, file: File) -> anyhow::Result<()> {
        self.requests
            .as_ref()
            .context("Lookahead closed")?
            .send(file)
            .map_err(|_| anyhow::anyhow!("Lookahead worker stopped"))
    }
}
impl Drop for Lookahead {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn spool(
    journal: &mut Journal,
    mut file: File,
    context: EventContext,
    suffix: &mut bool,
    limit: u64,
) -> anyhow::Result<Unit> {
    file.set_len(0)?;
    file.rewind()?;
    let mut writer = Writer::new(file);
    let mut bytes = 0u64;
    let ending = loop {
        match journal.next_record()? {
            Some(Record::Event(event)) => {
                bytes += event.encode(context)?.len() as u64 + 1;
                ensure!(
                    bytes <= limit,
                    "Processing block exceeds 256 MiB spool budget"
                );
                writer.write_event(&event, context)?;
                if *suffix {
                    break Ending::Suffix;
                }
            }
            Some(Record::Boundary { from, to }) => {
                let event = Event::Progress { frame: to };
                bytes += event.encode(context)?.len() as u64 + 1;
                ensure!(
                    bytes <= limit,
                    "Processing block exceeds 256 MiB spool budget"
                );
                writer.write_event(&event, context)?;
                break Ending::Boundary { from, to };
            }
            Some(Record::Finalization) => {
                *suffix = true;
                break Ending::Marker;
            }
            None => break Ending::Eof,
        }
    };
    let mut file = writer.into_inner();
    file.seek(SeekFrom::Start(0))?;
    Ok(Unit { file, ending })
}
