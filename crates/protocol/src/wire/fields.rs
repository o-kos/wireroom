use crate::*;

pub(super) fn wrong(field: &str, expected: &'static str) -> ValidationError {
    ValidationError::Type {
        field: field.into(),
        expected,
    }
}
pub(super) fn invalid(field: &str) -> ValidationError {
    ValidationError::Value(field.into())
}
pub(super) fn object(value: Json, field: &str) -> Result<Object, ValidationError> {
    match value {
        Json::Object(value) => Ok(value),
        _ => Err(wrong(field, "object")),
    }
}
pub(super) fn array(value: Json, field: &str) -> Result<Vec<Json>, ValidationError> {
    match value {
        Json::Array(value) => Ok(value),
        _ => Err(wrong(field, "array")),
    }
}
pub(super) fn string(value: Json, field: &str) -> Result<String, ValidationError> {
    match value {
        Json::String(value) => Ok(value),
        _ => Err(wrong(field, "string")),
    }
}
pub(super) fn boolean(value: Json, field: &str) -> Result<bool, ValidationError> {
    match value {
        Json::Bool(value) => Ok(value),
        _ => Err(wrong(field, "boolean")),
    }
}
pub(super) fn take(map: &mut Object, field: &str) -> Result<Json, ValidationError> {
    map.remove(field)
        .ok_or_else(|| ValidationError::Missing(field.into()))
}
pub(super) fn text(map: &mut Object, field: &str) -> Result<String, ValidationError> {
    string(take(map, field)?, field)
}
pub(super) fn nonempty(value: String, field: &str) -> Result<String, ValidationError> {
    if value.is_empty() {
        return Err(invalid(field));
    }
    Ok(value)
}
pub(super) fn identifier(map: &mut Object, field: &str) -> Result<String, ValidationError> {
    let value = nonempty(text(map, field)?, field)?;
    if value.contains('\0') {
        return Err(invalid(field));
    }
    Ok(value)
}
pub(super) fn unsigned(value: Json, field: &str) -> Result<u64, ValidationError> {
    let Json::Number(value) = value else {
        return Err(wrong(field, "unsigned integer token"));
    };
    if !value.as_str().bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(field));
    }
    value.as_str().parse().map_err(|_| invalid(field))
}
pub(super) fn uint(map: &mut Object, field: &str) -> Result<u64, ValidationError> {
    unsigned(take(map, field)?, field)
}
pub(super) fn u32_field(map: &mut Object, field: &str) -> Result<u32, ValidationError> {
    u32::try_from(uint(map, field)?).map_err(|_| invalid(field))
}
pub(super) fn finite(value: Json, field: &str) -> Result<f64, ValidationError> {
    let Json::Number(value) = value else {
        return Err(wrong(field, "finite number"));
    };
    let number: f64 = value.as_str().parse().map_err(|_| invalid(field))?;
    if !number.is_finite() {
        return Err(invalid(field));
    }
    Ok(number)
}
pub(super) fn speed(value: Json) -> Result<Speed, ValidationError> {
    if value == Json::String("max".into()) {
        return Ok(Speed::Max);
    }
    let value = finite(value, "speed")?;
    if value <= 0.0 {
        return Err(invalid("speed"));
    }
    Ok(Speed::Numeric(value))
}
pub(super) fn params(value: Json) -> Result<Parameters, ValidationError> {
    object(value, "params")?
        .into_iter()
        .map(|(key, value)| Ok((key, string(value, "parameter value")?)))
        .collect()
}
pub(super) fn exact(map: Object) -> Result<(), ValidationError> {
    if let Some(key) = map.into_keys().next() {
        return Err(ValidationError::UnknownField(key));
    }
    Ok(())
}
pub(super) fn pair(value: Json, field: &str) -> Result<[Json; 2], ValidationError> {
    array(value, field)?.try_into().map_err(|_| invalid(field))
}
pub(super) fn frame(value: u64, context: Context) -> Result<(), ValidationError> {
    if context.frames.is_some_and(|total| value > total) {
        return Err(invalid("frame"));
    }
    Ok(())
}
pub(super) fn span(value: Json, context: Context) -> Result<Span, ValidationError> {
    let [start, end] = pair(value, "samples")?;
    let span = Span {
        start: unsigned(start, "samples.start")?,
        end: unsigned(end, "samples.end")?,
    };
    if span.start > span.end {
        return Err(invalid("samples"));
    }
    frame(span.end, context)?;
    Ok(span)
}
pub(super) fn phase_component(value: Json) -> Result<i16, ValidationError> {
    let Json::Number(value) = value else {
        return Err(wrong("points", "integer token"));
    };
    let token = value.as_str();
    if !token
        .strip_prefix('-')
        .unwrap_or(token)
        .bytes()
        .all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("points"));
    }
    let value: i16 = token.parse().map_err(|_| invalid("points"))?;
    if !(-1000..=1000).contains(&value) {
        return Err(invalid("points"));
    }
    Ok(value)
}
