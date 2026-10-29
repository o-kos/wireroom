use crate::{Asset, Error, profile};
use dmd_protocol::{Context, DecodedEvent, Descriptor, Event, Json};
use std::collections::BTreeMap;

/// Stored data in order. Prefix events remain provisional until their closing
/// boundary validates the block. A boundary is a stored progress event, including
/// repeated frontiers; reading ahead never means source submission.
#[derive(Debug, PartialEq)]
pub enum Record {
    Event(Event),
    Boundary { from: u64, to: u64 },
    Finalization,
}
/// Incremental structural validation, requiring at most one bounded JSON line.
/// Full success requires `next_record()` to return `Ok(None)`; a marker or the
/// last source frontier alone is insufficient. Dropping does not certify unread data.
pub struct Journal {
    asset: Asset,
    buffer: [u8; 8192],
    begin: usize,
    end: usize,
    frames: u64,
    frontier: u64,
    log: Option<u64>,
    pending: bool,
    finalization: bool,
    complete: bool,
    failed: bool,
    references: BTreeMap<String, u64>,
}
impl Journal {
    pub(crate) fn new(asset: Asset, frames: u64) -> Self {
        Self {
            asset,
            buffer: [0; 8192],
            begin: 0,
            end: 0,
            frames,
            frontier: 0,
            log: None,
            pending: false,
            finalization: false,
            complete: false,
            failed: false,
            references: BTreeMap::new(),
        }
    }
    /// Read the next meaningful record, skipping validated unknown kinds.
    pub fn next_record(&mut self) -> Result<Option<Record>, Error> {
        if self.failed {
            return Err(Error::Sequence("reader ended after failure"));
        }
        if self.complete {
            return Ok(None);
        }
        let result = self.next();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn next(&mut self) -> Result<Option<Record>, Error> {
        loop {
            let Some(line) = self.line()? else {
                if !self.finalization || self.pending {
                    return Err(Error::Sequence("missing marker or closing progress"));
                }
                self.complete = true;
                return Ok(None);
            };
            if let Some(record) = self.dispatch(&line)? {
                return Ok(Some(record));
            }
        }
    }
    fn dispatch(&mut self, line: &[u8]) -> Result<Option<Record>, Error> {
        let text = std::str::from_utf8(line).map_err(|_| Error::Framing("UTF-8"))?;
        let Json::Object(map) = profile::json(text)? else {
            return Err(Error::Framing("non-object"));
        };
        if let Some(value) = map.get("event") {
            let Json::String(name) = value else {
                return Err(Error::Framing("event discriminator"));
            };
            match name.as_str() {
                "hello" | "started" | "state" | "error" | "finished" => {
                    return Err(Error::Sequence("forbidden stored event"));
                }
                "result" | "log" | "progress" => {
                    let DecodedEvent::Known(event) = Event::decode(
                        text,
                        Context {
                            frames: Some(self.frames),
                        },
                    )?
                    else {
                        return Err(Error::Sequence("event dispatch"));
                    };
                    return self.event(event).map(Some);
                }
                _ => return Ok(None),
            }
        }
        let Some(Json::String(kind)) = map.get("record") else {
            return Err(Error::Framing("service discriminator"));
        };
        if kind != "finalization" {
            return Ok(None);
        }
        if self.finalization || self.pending || self.frontier != self.frames {
            return Err(Error::Sequence("invalid finalization marker"));
        }
        self.finalization = true;
        Ok(Some(Record::Finalization))
    }
    fn event(&mut self, event: Event) -> Result<Record, Error> {
        match &event {
            Event::Progress { frame } => {
                if *frame < self.frontier
                    || self.log.is_some_and(|log| log > *frame)
                    || (self.finalization && *frame != self.frames)
                {
                    return Err(Error::Sequence("progress frontier"));
                }
                let from = self.frontier;
                self.frontier = *frame;
                self.log = None;
                self.pending = false;
                if !self.finalization {
                    return Ok(Record::Boundary { from, to: *frame });
                }
            }
            Event::Log { frame, .. } => {
                if *frame < self.frontier
                    || self.log.is_some_and(|log| log > *frame)
                    || (self.finalization && *frame != self.frames)
                {
                    return Err(Error::Sequence("log frontier"));
                }
                self.log = Some(*frame);
                self.pending = !self.finalization;
            }
            Event::Result(result) => {
                for descriptor in result.binary.iter().flatten() {
                    self.reference(descriptor)?;
                }
                self.pending = !self.finalization;
            }
            _ => return Err(Error::Sequence("unexpected event")),
        }
        Ok(Record::Event(event))
    }
    fn reference(&mut self, descriptor: &Descriptor) -> Result<(), Error> {
        let Descriptor::File { path, size, .. } = descriptor else {
            return Ok(());
        };
        profile::path(path)?;
        profile::path(&format!("payloads/{path}"))?;
        profile::limit(*size, profile::ENTRY, "descriptor size")?;
        if self.references.get(path).is_some_and(|old| old != size) {
            return Err(Error::ConflictingAsset(path.clone()));
        }
        if !self.references.contains_key(path) && self.references.len() == 65536 {
            return Err(Error::Resource("distinct asset references"));
        }
        self.references.insert(path.clone(), *size);
        Ok(())
    }
    fn byte(&mut self) -> Result<Option<u8>, Error> {
        if self.begin == self.end {
            self.end = self.asset.read_chunk(&mut self.buffer)?;
            self.begin = 0;
        }
        if self.end == 0 {
            return Ok(None);
        }
        let byte = self.buffer[self.begin];
        self.begin += 1;
        Ok(Some(byte))
    }
    fn line(&mut self) -> Result<Option<Vec<u8>>, Error> {
        let mut line = Vec::new();
        loop {
            let Some(byte) = self.byte()? else {
                if !line.is_empty() {
                    return Err(Error::Framing("unterminated line"));
                }
                return Ok(None);
            };
            if byte == b'\n' {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.is_empty() {
                    return Err(Error::Framing("blank line"));
                }
                return Ok(Some(line));
            }
            line.push(byte);
            if line.len() > profile::JSON_BYTES
                && (line.len() != profile::JSON_BYTES + 1 || byte != b'\r')
            {
                return Err(Error::Resource("journal line bytes"));
            }
        }
    }
}
