use super::fields::invalid;
use crate::*;

fn number(value: impl ToString) -> Result<Json, ValidationError> {
    Number::new(value.to_string()).map(Json::Number)
}
fn float(value: f64, field: &str) -> Result<Json, ValidationError> {
    if !value.is_finite() {
        return Err(invalid(field));
    }
    number(value)
}
fn string(value: &str) -> Json {
    Json::String(value.into())
}
fn speed(value: Speed) -> Result<Json, ValidationError> {
    match value {
        Speed::Max => Ok(string("max")),
        Speed::Numeric(value) => float(value, "speed"),
    }
}
fn params(values: &Parameters) -> Json {
    Json::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), string(value)))
            .collect(),
    )
}
fn tagged(field: &str, name: &str) -> Object {
    Object::from([(field.into(), string(name))])
}
fn put(map: &mut Object, name: &str, value: Json) {
    map.insert(name.into(), value);
}

pub(crate) fn command_json(command: &Command) -> Result<Json, ValidationError> {
    let name = match command {
        Command::Start(_) => "start",
        Command::Pause => "pause",
        Command::Resume => "resume",
        Command::SetSpeed(_) => "set_speed",
        Command::Tune(_) => "tune",
        Command::SetParams(_) => "set_params",
        Command::Stop => "stop",
    };
    let mut map = tagged("command", name);
    match command {
        Command::Start(start) => {
            put(&mut map, "recording", string(&start.recording));
            put(&mut map, "module", string(&start.module));
            put(&mut map, "params", params(&start.params));
            put(&mut map, "freq", float(start.freq, "freq")?);
            put(&mut map, "speed", speed(start.speed)?);
            put(&mut map, "paused", Json::Bool(start.paused));
            put(&mut map, "output", string(&start.output));
        }
        Command::SetSpeed(value) => put(&mut map, "speed", speed(*value)?),
        Command::Tune(value) => put(&mut map, "freq", float(*value, "freq")?),
        Command::SetParams(value) => put(&mut map, "params", params(value)),
        _ => {}
    }
    Ok(Json::Object(map))
}
fn snapshot(map: &mut Object, value: &Snapshot) -> Result<(), ValidationError> {
    put(
        map,
        "state",
        string(match value.state {
            State::Playing => "playing",
            State::Paused => "paused",
        }),
    );
    put(map, "frame", number(value.frame)?);
    put(map, "speed", speed(value.speed)?);
    put(map, "freq", float(value.freq, "freq")?);
    put(map, "params", params(&value.params));
    Ok(())
}
fn stream(value: Stream) -> Result<Json, ValidationError> {
    Ok(Json::Object(Object::from([
        (
            "format".into(),
            string(match value.format {
                StreamFormat::Real => "real",
                StreamFormat::Iq => "iq",
            }),
        ),
        ("channels".into(), number(value.channels)?),
        (
            "rate".into(),
            Json::Array(vec![
                number(value.rate.numerator)?,
                number(value.rate.denominator)?,
            ]),
        ),
    ])))
}
pub(crate) fn event_json(event: &Event) -> Result<Json, ValidationError> {
    let name = match event {
        Event::Hello(_) => "hello",
        Event::Started(_) => "started",
        Event::State(_) => "state",
        Event::Progress { .. } => "progress",
        Event::Log { .. } => "log",
        Event::Result(_) => "result",
        Event::Error { .. } => "error",
        Event::Finished { .. } => "finished",
    };
    let mut map = tagged("event", name);
    match event {
        Event::Hello(version) => put(
            &mut map,
            "protocol",
            Json::Object(Object::from([
                ("major".into(), number(version.major)?),
                ("minor".into(), number(version.minor)?),
            ])),
        ),
        Event::Started(value) => {
            put(&mut map, "module", string(&value.module));
            put(&mut map, "mask", Json::Object(value.mask.clone()));
            put(&mut map, "stream", stream(value.stream)?);
            put(&mut map, "frames", number(value.frames)?);
            snapshot(&mut map, &value.snapshot)?;
        }
        Event::State(value) => snapshot(&mut map, value)?,
        Event::Progress { frame } => put(&mut map, "frame", number(frame)?),
        Event::Log { frame, level, text } => {
            put(&mut map, "frame", number(frame)?);
            put(
                &mut map,
                "level",
                string(match level {
                    LogLevel::Debug => "debug",
                    LogLevel::Info => "info",
                    LogLevel::Warning => "warning",
                    LogLevel::Error => "error",
                }),
            );
            put(&mut map, "text", string(text));
        }
        Event::Result(value) => result(&mut map, value)?,
        Event::Error { fatal, message } => {
            put(&mut map, "fatal", Json::Bool(*fatal));
            put(&mut map, "message", string(message));
        }
        Event::Finished { reason, frame } => {
            put(
                &mut map,
                "reason",
                string(match reason {
                    TerminalReason::End => "end",
                    TerminalReason::Stopped => "stopped",
                    TerminalReason::Error => "error",
                }),
            );
            put(&mut map, "frame", number(frame)?);
        }
    }
    Ok(Json::Object(map))
}
fn result(map: &mut Object, value: &ResultEvent) -> Result<(), ValidationError> {
    for (key, value) in &value.extra {
        if matches!(
            key.as_str(),
            "event" | "samples" | "bits" | "text" | "log" | "ale" | "params" | "binary"
        ) {
            return Err(invalid(key));
        }
        put(map, key, value.clone());
    }
    put(
        map,
        "samples",
        Json::Array(vec![
            number(value.samples.start)?,
            number(value.samples.end)?,
        ]),
    );
    for (name, value) in [
        ("bits", &value.bits),
        ("text", &value.text),
        ("log", &value.log),
    ] {
        if let Some(value) = value {
            put(map, name, string(value));
        }
    }
    for (name, value) in [("ale", &value.ale), ("params", &value.params)] {
        if let Some(value) = value {
            put(map, name, value.clone());
        }
    }
    if let Some(descriptors) = &value.binary {
        let values = descriptors
            .iter()
            .map(descriptor)
            .collect::<Result<_, _>>()?;
        put(map, "binary", Json::Array(values));
    }
    Ok(())
}
fn descriptor(value: &Descriptor) -> Result<Json, ValidationError> {
    let mut map = Object::new();
    match value {
        Descriptor::Phase { id, body, points } => {
            put(&mut map, "id", string(id));
            put(&mut map, "type", string("phase"));
            put(&mut map, "body", body.clone());
            let points = points
                .iter()
                .map(|point| Ok(Json::Array(vec![number(point.i)?, number(point.q)?])))
                .collect::<Result<_, ValidationError>>()?;
            put(&mut map, "points", Json::Array(points));
        }
        Descriptor::File {
            id,
            kind,
            body,
            path,
            size,
        } => {
            if kind == "phase" {
                return Err(invalid("file type"));
            }
            put(&mut map, "id", string(id));
            put(&mut map, "type", string(kind));
            put(&mut map, "body", body.clone());
            put(&mut map, "path", string(path));
            put(&mut map, "size", number(size)?);
        }
    }
    Ok(Json::Object(map))
}
