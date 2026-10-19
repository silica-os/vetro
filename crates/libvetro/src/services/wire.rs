//! Defensive readers for D-Bus variants: a missing key or an odd type is `None`, never an error.

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

/// Looks through nested variants.
pub(crate) fn inner<'a, 'b>(mut value: &'a Value<'b>) -> &'a Value<'b> {
    while let Value::Value(next) = value {
        value = next;
    }
    value
}

pub(crate) fn as_bool(value: &Value<'_>) -> Option<bool> {
    match inner(value) {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

pub(crate) fn as_f64(value: &Value<'_>) -> Option<f64> {
    let number = match inner(value) {
        Value::F64(n) => *n,
        Value::U8(n) => f64::from(*n),
        Value::I16(n) => f64::from(*n),
        Value::U16(n) => f64::from(*n),
        Value::I32(n) => f64::from(*n),
        Value::U32(n) => f64::from(*n),
        Value::I64(n) => *n as f64,
        Value::U64(n) => *n as f64,
        _ => return None,
    };
    number.is_finite().then_some(number)
}

pub(crate) fn as_i64(value: &Value<'_>) -> Option<i64> {
    match inner(value) {
        Value::U8(n) => Some(i64::from(*n)),
        Value::I16(n) => Some(i64::from(*n)),
        Value::U16(n) => Some(i64::from(*n)),
        Value::I32(n) => Some(i64::from(*n)),
        Value::U32(n) => Some(i64::from(*n)),
        Value::I64(n) => Some(*n),
        Value::U64(n) => i64::try_from(*n).ok(),
        Value::F64(n) if n.is_finite() => Some(n.round() as i64),
        _ => None,
    }
}

pub(crate) fn as_str<'a>(value: &'a Value<'_>) -> Option<&'a str> {
    match inner(value) {
        Value::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

/// A dictionary of the daemon, read by key.
pub(crate) fn dict_bool(dict: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    dict.get(key).and_then(|v| as_bool(v))
}

pub(crate) fn dict_f64(dict: &HashMap<String, OwnedValue>, key: &str) -> Option<f64> {
    dict.get(key).and_then(|v| as_f64(v))
}

pub(crate) fn dict_i64(dict: &HashMap<String, OwnedValue>, key: &str) -> Option<i64> {
    dict.get(key).and_then(|v| as_i64(v))
}

pub(crate) fn dict_str<'a>(dict: &'a HashMap<String, OwnedValue>, key: &str) -> Option<&'a str> {
    dict.get(key).and_then(|v| as_str(v))
}
