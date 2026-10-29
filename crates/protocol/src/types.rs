use crate::{Json, ValidationError, wire};
use std::collections::BTreeMap;

pub type Frame = u64;
pub type Frequency = f64;
pub type Parameters = BTreeMap<String, String>;
pub type Object = BTreeMap<String, Json>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}
impl Version {
    pub fn validate(self) -> Result<(), ValidationError> {
        if self.major == 0 {
            return Err(ValidationError::Value("protocol.major".into()));
        }
        Ok(())
    }
    /// Accept every positive major through the caller's maximum, ignoring minor.
    /// Validity and implementation support are separate checks.
    pub fn check_supported(self, maximum_major: u32) -> Result<(), ValidationError> {
        self.validate()?;
        if self.major > maximum_major {
            return Err(ValidationError::UnsupportedMajor(self.major));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Speed {
    Numeric(f64),
    Max,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamFormat {
    Real,
    Iq,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub numerator: u64,
    pub denominator: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stream {
    pub format: StreamFormat,
    pub channels: u32,
    pub rate: Rate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: Frame,
    pub end: Frame,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Playing,
    Paused,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalReason {
    End,
    Stopped,
    Error,
}

/// Optional original-source frame count; never inferred from progress messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub frames: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    pub recording: String,
    pub module: String,
    pub params: Parameters,
    pub freq: Frequency,
    pub speed: Speed,
    pub paused: bool,
    pub output: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Start(Start),
    Pause,
    Resume,
    SetSpeed(Speed),
    Tune(Frequency),
    SetParams(Parameters),
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub state: State,
    pub frame: Frame,
    pub speed: Speed,
    pub freq: Frequency,
    pub params: Parameters,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub module: String,
    pub mask: Object,
    pub stream: Stream,
    pub frames: u64,
    pub snapshot: Snapshot,
}

/// Encoded integer pair. Decode does not quantize these components again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhasePoint {
    pub i: i16,
    pub q: i16,
}
impl PhasePoint {
    /// Quantize normalized source components, with ties away from zero.
    pub fn from_source(i: f64, q: f64) -> Result<Self, ValidationError> {
        Ok(Self {
            i: quantize(i)?,
            q: quantize(q)?,
        })
    }
}
fn quantize(value: f64) -> Result<i16, ValidationError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(ValidationError::Value("phase source component".into()));
    }
    Ok((value * 1000.0).round() as i16)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Descriptor {
    Phase {
        id: String,
        body: Json,
        points: Vec<PhasePoint>,
    },
    File {
        id: String,
        kind: String,
        body: Json,
        path: String,
        size: u64,
    },
}

/// All optional result payloads may coexist. `extra` retains producer keys such
/// as `record`; reserved/known result keys in `extra` are rejected on encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultEvent {
    pub samples: Span,
    pub bits: Option<String>,
    pub text: Option<String>,
    pub log: Option<String>,
    pub ale: Option<Json>,
    pub params: Option<Json>,
    pub binary: Option<Vec<Descriptor>>,
    pub extra: Object,
}
impl ResultEvent {
    pub fn new(samples: Span) -> Self {
        Self {
            samples,
            bits: None,
            text: None,
            log: None,
            ale: None,
            params: None,
            binary: None,
            extra: Object::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Hello(Version),
    Started(Started),
    State(Snapshot),
    Progress {
        frame: Frame,
    },
    Log {
        frame: Frame,
        level: LogLevel,
        text: String,
    },
    Result(ResultEvent),
    Error {
        fatal: bool,
        message: String,
    },
    Finished {
        reason: TerminalReason,
        frame: Frame,
    },
}

/// Unknown event names are ignorable, distinct from malformed known events.
#[derive(Clone, Debug, PartialEq)]
pub enum DecodedEvent {
    Known(Event),
    Unknown { name: String, fields: Object },
}

impl Command {
    pub fn decode(input: &str) -> Result<Self, ValidationError> {
        wire::command(Json::parse(input)?)
    }
    pub fn validate(&self) -> Result<(), ValidationError> {
        let json = wire::command_json(self)?;
        json.validate()?;
        wire::command(json).map(|_| ())
    }
    pub fn encode(&self) -> Result<String, ValidationError> {
        self.validate()?;
        wire::command_json(self)?.to_compact_string()
    }
}
impl Event {
    pub fn decode(input: &str, context: Context) -> Result<DecodedEvent, ValidationError> {
        wire::event(Json::parse(input)?, context)
    }
    pub fn validate(&self, context: Context) -> Result<(), ValidationError> {
        let json = wire::event_json(self)?;
        json.validate()?;
        wire::event(json, context).map(|_| ())
    }
    pub fn encode(&self, context: Context) -> Result<String, ValidationError> {
        self.validate(context)?;
        wire::event_json(self)?.to_compact_string()
    }
}

/// Validate an output-relative reference without rewriting it. This does not
/// establish safe filesystem access, existence, regular-file status or size.
pub fn validate_result_path(path: &str) -> Result<(), ValidationError> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(ValidationError::Value("path".into()));
    }
    Ok(())
}
