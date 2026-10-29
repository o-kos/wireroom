use super::fields::*;
use crate::*;

pub(crate) fn command(value: Json) -> Result<Command, ValidationError> {
    let mut map = object(value, "command")?;
    let name = text(&mut map, "command")?;
    let command = match name.as_str() {
        "start" => Command::Start(Start {
            recording: identifier(&mut map, "recording")?,
            module: identifier(&mut map, "module")?,
            params: params(take(&mut map, "params")?)?,
            freq: finite(take(&mut map, "freq")?, "freq")?,
            speed: speed(take(&mut map, "speed")?)?,
            paused: boolean(take(&mut map, "paused")?, "paused")?,
            output: identifier(&mut map, "output")?,
        }),
        "pause" => Command::Pause,
        "resume" => Command::Resume,
        "stop" => Command::Stop,
        "set_speed" => Command::SetSpeed(speed(take(&mut map, "speed")?)?),
        "tune" => Command::Tune(finite(take(&mut map, "freq")?, "freq")?),
        "set_params" => Command::SetParams(params(take(&mut map, "params")?)?),
        _ => return Err(ValidationError::UnknownCommand(name)),
    };
    exact(map)?;
    Ok(command)
}

pub(crate) fn event(value: Json, context: Context) -> Result<DecodedEvent, ValidationError> {
    let mut map = object(value, "event")?;
    let name = text(&mut map, "event")?;
    let event = match name.as_str() {
        "hello" => {
            let version = version(take(&mut map, "protocol")?)?;
            exact(map)?;
            Event::Hello(version)
        }
        "started" => Event::Started(started(&mut map, context)?),
        "state" => Event::State(snapshot(&mut map, context)?),
        "progress" => Event::Progress {
            frame: checked_frame(&mut map, context)?,
        },
        "log" => log(&mut map, context)?,
        "result" => Event::Result(result(map, context)?),
        "error" => Event::Error {
            fatal: boolean(take(&mut map, "fatal")?, "fatal")?,
            message: nonempty(text(&mut map, "message")?, "message")?,
        },
        "finished" => finished(&mut map, context)?,
        _ => return Ok(DecodedEvent::Unknown { name, fields: map }),
    };
    Ok(DecodedEvent::Known(event))
}
fn version(value: Json) -> Result<Version, ValidationError> {
    let mut map = object(value, "protocol")?;
    let version = Version {
        major: u32_field(&mut map, "major")?,
        minor: u32_field(&mut map, "minor")?,
    };
    exact(map)?;
    version.validate()?;
    Ok(version)
}
fn checked_frame(map: &mut Object, context: Context) -> Result<Frame, ValidationError> {
    let value = uint(map, "frame")?;
    frame(value, context)?;
    Ok(value)
}
fn snapshot(map: &mut Object, context: Context) -> Result<Snapshot, ValidationError> {
    let state = match text(map, "state")?.as_str() {
        "playing" => State::Playing,
        "paused" => State::Paused,
        _ => return Err(invalid("state")),
    };
    Ok(Snapshot {
        state,
        frame: checked_frame(map, context)?,
        speed: speed(take(map, "speed")?)?,
        freq: finite(take(map, "freq")?, "freq")?,
        params: params(take(map, "params")?)?,
    })
}
fn stream(value: Json) -> Result<Stream, ValidationError> {
    let mut map = object(value, "stream")?;
    let format = match text(&mut map, "format")?.as_str() {
        "real" => StreamFormat::Real,
        "iq" => StreamFormat::Iq,
        _ => return Err(invalid("stream.format")),
    };
    let channels = u32_field(&mut map, "channels")?;
    let [a, b] = pair(take(&mut map, "rate")?, "rate")?;
    let rate = Rate {
        numerator: unsigned(a, "rate.numerator")?,
        denominator: unsigned(b, "rate.denominator")?,
    };
    if channels == 0 || rate.numerator == 0 || rate.denominator == 0 {
        return Err(invalid("stream"));
    }
    Ok(Stream {
        format,
        channels,
        rate,
    })
}
fn started(map: &mut Object, context: Context) -> Result<Started, ValidationError> {
    let started = Started {
        module: identifier(map, "module")?,
        mask: object(take(map, "mask")?, "mask")?,
        stream: stream(take(map, "stream")?)?,
        frames: uint(map, "frames")?,
        snapshot: snapshot(map, context)?,
    };
    if started.snapshot.frame != 0 {
        return Err(invalid("started.frame"));
    }
    Ok(started)
}
fn log(map: &mut Object, context: Context) -> Result<Event, ValidationError> {
    let level = match text(map, "level")?.as_str() {
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warning" => LogLevel::Warning,
        "error" => LogLevel::Error,
        _ => return Err(invalid("level")),
    };
    Ok(Event::Log {
        frame: checked_frame(map, context)?,
        level,
        text: text(map, "text")?,
    })
}
fn finished(map: &mut Object, context: Context) -> Result<Event, ValidationError> {
    let reason = match text(map, "reason")?.as_str() {
        "end" => TerminalReason::End,
        "stopped" => TerminalReason::Stopped,
        "error" => TerminalReason::Error,
        _ => return Err(invalid("reason")),
    };
    let frame = checked_frame(map, context)?;
    if reason == TerminalReason::End && context.frames.is_some_and(|total| frame != total) {
        return Err(invalid("finished.frame"));
    }
    Ok(Event::Finished { reason, frame })
}
fn optional_text(map: &mut Object, name: &str) -> Result<Option<String>, ValidationError> {
    map.remove(name)
        .map(|value| string(value, name))
        .transpose()
}
fn result(mut map: Object, context: Context) -> Result<ResultEvent, ValidationError> {
    let samples = span(take(&mut map, "samples")?, context)?;
    let bits = optional_text(&mut map, "bits")?;
    if bits
        .as_ref()
        .is_some_and(|bits| !bits.bytes().all(|byte| matches!(byte, b'0' | b'1')))
    {
        return Err(invalid("bits"));
    }
    let text = optional_text(&mut map, "text")?;
    let log = optional_text(&mut map, "log")?;
    let ale = map.remove("ale");
    let params = map.remove("params");
    let binary = map
        .remove("binary")
        .map(|value| {
            array(value, "binary")?
                .into_iter()
                .map(descriptor)
                .collect()
        })
        .transpose()?;
    Ok(ResultEvent {
        samples,
        bits,
        text,
        log,
        ale,
        params,
        binary,
        extra: map,
    })
}
fn descriptor(value: Json) -> Result<Descriptor, ValidationError> {
    let mut map = object(value, "binary descriptor")?;
    let id = identifier(&mut map, "id")?;
    let kind = nonempty(text(&mut map, "type")?, "type")?;
    let body = take(&mut map, "body")?;
    if kind == "phase" {
        forbid(&map, &["path", "size"])?;
        let points = array(take(&mut map, "points")?, "points")?
            .into_iter()
            .map(|point| {
                let [i, q] = pair(point, "points")?;
                Ok(PhasePoint {
                    i: phase_component(i)?,
                    q: phase_component(q)?,
                })
            })
            .collect::<Result<_, ValidationError>>()?;
        return Ok(Descriptor::Phase { id, body, points });
    }
    forbid(&map, &["points"])?;
    let path = text(&mut map, "path")?;
    validate_result_path(&path)?;
    let size = uint(&mut map, "size")?;
    Ok(Descriptor::File {
        id,
        kind,
        body,
        path,
        size,
    })
}
fn forbid(map: &Object, names: &[&str]) -> Result<(), ValidationError> {
    for name in names {
        if map.contains_key(*name) {
            return Err(invalid(name));
        }
    }
    Ok(())
}
