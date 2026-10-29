use crate::{Command, Context, DecodedEvent, Event, ValidationError};
use std::io::{self, BufRead, Write};

pub const MAX_LINE_BYTES: usize = 16_777_216;

/// A complete rejected command line occupies one FIFO rejection position. Only
/// `Invalid` and `Utf8` are recoverable at the next line; size, I/O and incomplete EOF end
/// this reader. Event consumers treat *all* invalid data as protocol failure.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("invalid complete message: {0}")]
    Invalid(#[from] ValidationError),
    #[error("invalid UTF-8 in complete message")]
    Utf8(#[source] std::str::Utf8Error),
    #[error("line exceeds {MAX_LINE_BYTES} bytes")]
    TooLong,
    #[error("unterminated final line")]
    Incomplete,
    #[error("input failed: {0}")]
    Io(#[source] io::Error),
    #[error("reader already ended after a fatal failure")]
    Ended,
}
impl DecodeError {
    /// UTF-8 and JSON/shape failures on complete command lines are recoverable.
    pub fn is_complete_line_rejection(&self) -> bool {
        matches!(self, Self::Invalid(_) | Self::Utf8(_))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("invalid outgoing message: {0}")]
    Invalid(#[from] ValidationError),
    #[error("encoded line exceeds {MAX_LINE_BYTES} bytes")]
    TooLong,
    #[error("output failed: {0}")]
    Io(#[source] io::Error),
}

/// Pull-based incremental bounded NDJSON reader over a caller-owned buffer.
/// `Ok(None)` means clean EOF; no runtime action is implied. A fatal failure
/// poisons this reader; it never drains an oversized line to seek recovery.
pub struct Reader<R> {
    inner: R,
    pending: Vec<u8>,
    ended: bool,
}
impl<R: BufRead> Reader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            ended: false,
        }
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
    pub fn read_command(&mut self) -> Result<Option<Command>, DecodeError> {
        self.line()?
            .map(|line| {
                let text = std::str::from_utf8(&line).map_err(DecodeError::Utf8)?;
                Command::decode(text).map_err(DecodeError::Invalid)
            })
            .transpose()
    }
    pub fn read_event(&mut self, context: Context) -> Result<Option<DecodedEvent>, DecodeError> {
        self.line()?
            .map(|line| {
                let text = std::str::from_utf8(&line).map_err(DecodeError::Utf8)?;
                Event::decode(text, context).map_err(DecodeError::Invalid)
            })
            .transpose()
    }
    fn line(&mut self) -> Result<Option<Vec<u8>>, DecodeError> {
        if self.ended {
            return Err(DecodeError::Ended);
        }
        let result = self.next_line();
        if result.is_err() {
            self.ended = true;
        }
        result
    }
    fn next_line(&mut self) -> Result<Option<Vec<u8>>, DecodeError> {
        loop {
            let bytes = match self.inner.fill_buf() {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => result.map_err(DecodeError::Io)?,
            };
            if bytes.is_empty() {
                if self.pending.len() > MAX_LINE_BYTES {
                    return Err(DecodeError::TooLong);
                }
                return if self.pending.is_empty() {
                    Ok(None)
                } else {
                    Err(DecodeError::Incomplete)
                };
            }
            // One provisional CR is permitted past the payload limit, only
            // until the next byte establishes whether it belongs to CRLF.
            let room = MAX_LINE_BYTES + 1 - self.pending.len();
            let inspected = &bytes[..bytes.len().min(room + 1)];
            let newline = inspected.iter().position(|byte| *byte == b'\n');
            let length = newline.unwrap_or(bytes.len());
            let copied = length.min(room);
            self.pending.extend_from_slice(&bytes[..copied]);
            self.inner.consume(copied);
            let complete = newline == Some(copied);
            self.check_length()?;
            if copied < length {
                return Err(DecodeError::TooLong);
            }
            if complete {
                self.inner.consume(1);
                if self.pending.last() == Some(&b'\r') {
                    self.pending.pop();
                }
                return Ok(Some(std::mem::take(&mut self.pending)));
            }
        }
    }
    fn check_length(&self) -> Result<(), DecodeError> {
        let length = self.pending.len();
        if length <= MAX_LINE_BYTES {
            return Ok(());
        }
        if length == MAX_LINE_BYTES + 1 && self.pending.last() == Some(&b'\r') {
            return Ok(());
        }
        Err(DecodeError::TooLong)
    }
}

/// Validates and buffers the whole compact object before any output is written.
/// Every successful message ends in LF and is flushed. After an I/O error the
/// caller must treat the stream as unusable: a prefix may already be on the wire.
pub struct Writer<W> {
    inner: W,
}
impl<W: Write> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
    pub fn into_inner(self) -> W {
        self.inner
    }
    pub fn write_command(&mut self, command: &Command) -> Result<(), EncodeError> {
        self.message(command.encode()?)
    }
    pub fn write_event(&mut self, event: &Event, context: Context) -> Result<(), EncodeError> {
        self.message(event.encode(context)?)
    }
    fn message(&mut self, mut encoded: String) -> Result<(), EncodeError> {
        if encoded.len() > MAX_LINE_BYTES {
            return Err(EncodeError::TooLong);
        }
        encoded.push('\n');
        self.inner
            .write_all(encoded.as_bytes())
            .map_err(EncodeError::Io)?;
        self.inner.flush().map_err(EncodeError::Io)
    }
}
