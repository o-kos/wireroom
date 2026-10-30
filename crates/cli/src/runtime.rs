use crate::{
    hashing::Hashing,
    lookahead::{Ending, Lookahead, Unit},
    output::Output,
    pacing::Budget,
};
use anyhow::{Context as _, bail};
use dmd_protocol::{
    Command, Context, DecodeError, DecodedEvent, Event, LogLevel, Reader, Snapshot, Start, Started,
    State, TerminalReason, Writer,
};
use dmd_replay::Recording;
use dmd_source::Source;
use std::{
    io::{self, BufReader, Write},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

type Input = Result<Option<Command>, DecodeError>;
const TICK: Duration = Duration::from_millis(5);

pub fn run() -> anyhow::Result<()> {
    let (tx, rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("protocol-input".into())
        .spawn(move || {
            let mut reader = Reader::new(io::stdin().lock());
            loop {
                let command = reader.read_command();
                let done = matches!(command, Ok(None))
                    || command
                        .as_ref()
                        .is_err_and(|e| !e.is_complete_line_rejection());
                if tx.send(command).is_err() || done {
                    break;
                }
            }
        })?;
    let mut runtime = Runtime {
        writer: Writer::new(io::stdout().lock()),
        input: rx,
        frame: 0,
        context: Context::default(),
    };
    runtime.emit(Event::Hello(dmd_protocol::CURRENT_VERSION))?;
    let result = runtime.lifecycle();
    if let Err(error) = result {
        if error
            .downcast_ref::<dmd_protocol::EncodeError>()
            .is_some_and(|e| matches!(e, dmd_protocol::EncodeError::Io(_)))
        {
            return Err(error);
        }
        runtime.emit(Event::Error {
            fatal: true,
            message: error.to_string(),
        })?;
        runtime.emit(Event::Finished {
            reason: TerminalReason::Error,
            frame: runtime.frame,
        })?;
        return Err(error);
    }
    Ok(())
}
struct Runtime<W> {
    writer: Writer<W>,
    input: Receiver<Input>,
    frame: u64,
    context: Context,
}
fn open_regular(path: &std::path::Path) -> anyhow::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "Signal and companion must be regular files"
    );
    Ok(file)
}
struct Run {
    recording: Recording,
    output: Output,
    lookahead: Lookahead,
    hashing: Option<Hashing>,
    source: Source,
    expected: String,
    snapshot: Snapshot,
    rate: dmd_protocol::Rate,
    frames: u64,
    reason: Option<TerminalReason>,
    clock: PendingClock,
}
struct PendingClock {
    active: bool,
    credit: f64,
    last: Instant,
    now: Box<dyn Fn() -> Instant + Send>,
}
impl PendingClock {
    fn new() -> Self {
        Self {
            active: false,
            credit: 0.0,
            last: Instant::now(),
            now: Box::new(Instant::now),
        }
    }
    fn begin(&mut self) {
        self.active = true;
        self.credit = 0.0;
        self.last = (self.now)();
    }
    fn account(&mut self, snapshot: &Snapshot) {
        let now = (self.now)();
        if self.active {
            self.credit += crate::pacing::source_time(now.duration_since(self.last), snapshot);
        }
        self.last = now;
    }
    fn take(&mut self) -> f64 {
        std::mem::take(&mut self.credit)
    }
}
impl Run {
    fn setup(start: Start) -> anyhow::Result<(Self, Event)> {
        let source = Source::from_file(open_regular(std::path::Path::new(&start.recording))?)?;
        let metadata = source.metadata();
        let recording = Recording::from_reader(
            open_regular(&dmd_replay::companion_path(&start.recording))?,
            metadata.frames,
        )?;
        let settings = recording.metadata();
        let params = settings.match_initial(&start.module, start.freq, &start.params)?;
        let output = Output::open(&start.output)?;
        drop(output.temporary()?);
        let snapshot = Snapshot {
            state: if start.paused {
                State::Paused
            } else {
                State::Playing
            },
            frame: 0,
            speed: start.speed,
            freq: start.freq,
            params,
        };
        let event = Event::Started(Started {
            module: settings.module.clone(),
            mask: settings.mask.clone(),
            stream: metadata.stream,
            frames: metadata.frames,
            snapshot: snapshot.clone(),
        });
        let expected = settings.signal_sha256.clone();
        let lookahead = Lookahead::new(
            recording.journal()?,
            Context {
                frames: Some(metadata.frames),
            },
        )?;
        // Hashing starts after the caller emits started.
        let hashing = None;
        Ok((
            Self {
                recording,
                output,
                lookahead,
                hashing,
                source,
                expected,
                snapshot,
                rate: metadata.stream.rate,
                frames: metadata.frames,
                reason: None,
                clock: PendingClock::new(),
            },
            event,
        ))
    }
}
impl<W: Write> Runtime<W> {
    fn emit(&mut self, event: Event) -> anyhow::Result<()> {
        Ok(self.writer.write_event(&event, self.context)?)
    }
    fn reject(&mut self, message: impl Into<String>) -> anyhow::Result<()> {
        self.emit(Event::Error {
            fatal: false,
            message: message.into(),
        })
    }
    fn decode(&mut self, input: Input) -> anyhow::Result<Option<Command>> {
        match input {
            Ok(Some(command)) => Ok(Some(command)),
            Ok(None) => Ok(Some(Command::Stop)),
            Err(error) if error.is_complete_line_rejection() => {
                self.reject(error.to_string())?;
                Ok(None)
            }
            Err(DecodeError::Incomplete) => {
                self.reject("Unterminated final command")?;
                Ok(Some(Command::Stop))
            }
            Err(error) => Err(error.into()),
        }
    }
    fn lifecycle(&mut self) -> anyhow::Result<()> {
        let start = loop {
            let input = self.input.recv().context("Command reader disconnected")?;
            match self.decode(input)? {
                Some(Command::Start(start)) => break start,
                Some(Command::Stop) => {
                    return self.emit(Event::Finished {
                        reason: TerminalReason::Stopped,
                        frame: 0,
                    });
                }
                Some(_) => self.reject("Run has not started")?,
                None => {}
            }
        };
        let (mut run, started) = Run::setup(start)?;
        self.context.frames = Some(run.frames);
        self.emit(started)?;
        run.hashing = Some(Hashing::start(run.source.clone(), run.expected.clone()));
        let result = self.play(&mut run);
        self.complete(&mut run, result)
    }
    fn complete(&mut self, run: &mut Run, result: anyhow::Result<()>) -> anyhow::Result<()> {
        let joined = run.lookahead.finish();
        let result = result.and(joined);
        let warning = run.hashing.as_mut().and_then(Hashing::finish);
        if result.is_ok() {
            if let Some(text) = warning {
                self.emit(Event::Log {
                    frame: self.frame,
                    level: LogLevel::Warning,
                    text,
                })?;
            }
            self.emit(Event::Finished {
                reason: run.reason.unwrap_or(TerminalReason::End),
                frame: self.frame,
            })?;
        }
        result
    }
    fn commands(&mut self, run: &mut Run) -> anyhow::Result<()> {
        // A bounded batch prevents an infinite command stream starving playback.
        for _ in 0..32 {
            let input = match self.input.try_recv() {
                Ok(input) => input,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            if let Some(command) = self.decode(input)? {
                self.control(run, command)?;
            }
        }
        Ok(())
    }
    fn control(&mut self, run: &mut Run, command: Command) -> anyhow::Result<()> {
        run.clock.account(&run.snapshot);
        if matches!(command, Command::Stop) {
            run.reason.get_or_insert(TerminalReason::Stopped);
            return Ok(());
        }
        if run.reason.is_some() {
            return self.reject("Run is finalizing");
        }
        match command {
            Command::Start(_) => return self.reject("Run already started"),
            Command::Pause => run.snapshot.state = State::Paused,
            Command::Resume => run.snapshot.state = State::Playing,
            Command::SetSpeed(speed) => {
                if speed == dmd_protocol::Speed::Max && run.clock.active {
                    run.clock.credit = f64::INFINITY;
                }
                run.snapshot.speed = speed;
            }
            Command::Tune(freq) if freq != run.snapshot.freq => {
                return self.reject("Replay frequency is fixed");
            }
            Command::SetParams(patch) => {
                let mut params = run.snapshot.params.clone();
                params.extend(patch);
                if params != run.snapshot.params {
                    return self.reject("Replay parameters are fixed");
                }
            }
            Command::Tune(_) | Command::Stop => {}
        }
        run.snapshot.frame = self.frame;
        self.emit(Event::State(run.snapshot.clone()))
    }
    fn play(&mut self, run: &mut Run) -> anyhow::Result<()> {
        let mut spool = Some(run.output.temporary()?);
        loop {
            self.commands(run)?;
            if run.reason == Some(TerminalReason::Stopped) && self.frame < run.frames {
                return Ok(());
            }
            if run.reason.is_none() && run.snapshot.state == State::Paused {
                thread::sleep(TICK);
                continue;
            }
            run.clock.begin();
            run.lookahead
                .request(spool.take().context("Missing block spool")?)?;
            let Some(unit) = self.lookahead(run)? else {
                return Ok(());
            };
            match unit.ending {
                Ending::Marker => {
                    run.reason.get_or_insert(TerminalReason::End);
                }
                Ending::Eof => return Ok(()),
                Ending::Boundary { from, to } => {
                    let mut budget = Budget::new(from, to, run.rate);
                    budget.credit(run.clock.take());
                    if !self.wait(run, budget)? {
                        return Ok(());
                    }
                    run.clock.active = false;
                    self.submit(run, &unit)?;
                }
                Ending::Suffix => {
                    self.commands(run)?;
                    self.submit(run, &unit)?;
                }
            }
            spool = Some(unit.file);
        }
    }
    fn lookahead(&mut self, run: &mut Run) -> anyhow::Result<Option<Unit>> {
        loop {
            run.clock.account(&run.snapshot);
            self.commands(run)?;
            if run.reason == Some(TerminalReason::Stopped) && self.frame < run.frames {
                return Ok(None);
            }
            match run.lookahead.results.recv_timeout(TICK) {
                Ok(result) => {
                    run.clock.account(&run.snapshot);
                    return result.map(Some);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => bail!("Lookahead worker disconnected"),
            }
        }
    }
    fn wait(&mut self, run: &mut Run, mut budget: Budget) -> anyhow::Result<bool> {
        loop {
            if let Some(ready) = self.pending(run, &mut budget)? {
                return Ok(ready);
            }
            thread::sleep(TICK);
        }
    }
    fn pending(&mut self, run: &mut Run, budget: &mut Budget) -> anyhow::Result<Option<bool>> {
        run.clock.account(&run.snapshot);
        self.commands(run)?;
        budget.credit(run.clock.take());
        if run.reason.is_some() {
            return Ok(Some(self.frame == run.frames));
        }
        Ok((run.snapshot.state == State::Playing && budget.ready()).then_some(true))
    }
    fn submit(&mut self, run: &mut Run, unit: &Unit) -> anyhow::Result<()> {
        let mut reader = Reader::new(BufReader::new(unit.file.try_clone()?));
        while let Some(event) = reader.read_event(self.context)? {
            let DecodedEvent::Known(event) = event else {
                bail!("Unknown event in spool");
            };
            match &event {
                Event::Log { frame, .. } | Event::Progress { frame } => self.frame = *frame,
                Event::Result(result) => run.output.materialize(&run.recording, result)?,
                _ => bail!("Invalid event in spool"),
            }
            self.emit(event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
