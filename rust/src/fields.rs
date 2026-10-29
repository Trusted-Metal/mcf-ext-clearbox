use std::collections::BTreeMap;

use crate::contract::{Attr, AttrValue, Scalar};
use crate::errors::{join_path, validation, Result};
use crate::model::Extra;

type Attrs = BTreeMap<String, Attr>;

/// Largest integer a JS number holds exactly; both codecs reject anything beyond it.
const MAX_SAFE: i64 = 9_007_199_254_740_991;

fn attr_path(path: &str, name: &str) -> String {
    join_path(path, &format!("@{name}"))
}

fn scalar_of<'a>(a: &'a Attr, p: &str) -> Result<&'a Scalar> {
    match &a.value {
        AttrValue::Scalar(s) => Ok(s),
        AttrValue::Array(_) => Err(validation(p, "expected a scalar attribute, got an array")),
    }
}

fn float_text(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        v.to_string()
    }
}

fn text_of(s: &Scalar) -> String {
    match s {
        Scalar::Utf8(v) => v.clone(),
        Scalar::I8(v) => v.to_string(),
        Scalar::U8(v) => v.to_string(),
        Scalar::I16(v) => v.to_string(),
        Scalar::U16(v) => v.to_string(),
        Scalar::I32(v) => v.to_string(),
        Scalar::U32(v) => v.to_string(),
        Scalar::I64(v) => v.to_string(),
        Scalar::U64(v) => v.to_string(),
        Scalar::F32(v) => float_text(f64::from(*v)),
        Scalar::F64(v) => float_text(*v),
    }
}

/// Optional text: trimmed, empty means absent. Numeric dtypes are read as their decimal text.
pub(crate) fn read_opt_string(attrs: &Attrs, name: &str, path: &str) -> Result<Option<String>> {
    let Some(a) = attrs.get(name) else {
        return Ok(None);
    };
    let t = text_of(scalar_of(a, &attr_path(path, name))?);
    let t = t.trim();
    Ok(if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    })
}

/// Required text, kept as stored (not trimmed); the attribute must exist.
pub(crate) fn read_required_string(attrs: &Attrs, name: &str, path: &str) -> Result<String> {
    let p = attr_path(path, name);
    let a = attrs
        .get(name)
        .ok_or_else(|| validation(&p, "required attribute is missing"))?;
    Ok(text_of(scalar_of(a, &p)?))
}

fn safe(n: i64, p: &str) -> Result<i64> {
    if n.abs() > MAX_SAFE {
        return Err(validation(
            p,
            format!("integer {n} does not fit in a JS number"),
        ));
    }
    Ok(n)
}

/// Optional integer: any integer dtype, an integral float, or numeric text; empty text means absent.
pub(crate) fn read_opt_int(attrs: &Attrs, name: &str, path: &str) -> Result<Option<i64>> {
    let Some(a) = attrs.get(name) else {
        return Ok(None);
    };
    let p = attr_path(path, name);
    let from_float = |f: f64| -> Result<Option<i64>> {
        if !f.is_finite() || f.fract() != 0.0 || f.abs() > MAX_SAFE as f64 {
            return Err(validation(&p, format!("expected an integer, got {f}")));
        }
        Ok(Some(f as i64))
    };
    match scalar_of(a, &p)? {
        Scalar::I8(v) => Ok(Some(i64::from(*v))),
        Scalar::U8(v) => Ok(Some(i64::from(*v))),
        Scalar::I16(v) => Ok(Some(i64::from(*v))),
        Scalar::U16(v) => Ok(Some(i64::from(*v))),
        Scalar::I32(v) => Ok(Some(i64::from(*v))),
        Scalar::U32(v) => Ok(Some(i64::from(*v))),
        Scalar::I64(v) => safe(*v, &p).map(Some),
        Scalar::U64(v) => match i64::try_from(*v) {
            Ok(n) => safe(n, &p).map(Some),
            Err(_) => Err(validation(
                &p,
                format!("integer {v} does not fit in a JS number"),
            )),
        },
        Scalar::F32(v) => from_float(f64::from(*v)),
        Scalar::F64(v) => from_float(*v),
        Scalar::Utf8(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(None);
            }
            let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(validation(&p, format!("expected an integer, got \"{t}\"")));
            }
            match t.parse::<i64>() {
                Ok(n) => safe(n, &p).map(Some),
                Err(_) => Err(validation(
                    &p,
                    format!("integer \"{t}\" does not fit in a JS number"),
                )),
            }
        }
    }
}

/// Optional float: any numeric dtype or numeric text; empty text means absent.
pub(crate) fn read_opt_float(attrs: &Attrs, name: &str, path: &str) -> Result<Option<f64>> {
    let Some(a) = attrs.get(name) else {
        return Ok(None);
    };
    let p = attr_path(path, name);
    Ok(Some(match scalar_of(a, &p)? {
        Scalar::I8(v) => f64::from(*v),
        Scalar::U8(v) => f64::from(*v),
        Scalar::I16(v) => f64::from(*v),
        Scalar::U16(v) => f64::from(*v),
        Scalar::I32(v) => f64::from(*v),
        Scalar::U32(v) => f64::from(*v),
        Scalar::I64(v) => safe(*v, &p)? as f64,
        Scalar::U64(v) => match i64::try_from(*v) {
            Ok(n) => safe(n, &p)? as f64,
            Err(_) => {
                return Err(validation(
                    &p,
                    format!("integer {v} does not fit in a JS number"),
                ))
            }
        },
        Scalar::F32(v) => f64::from(*v),
        Scalar::F64(v) => *v,
        Scalar::Utf8(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(None);
            }
            match t.parse::<f64>() {
                Ok(n) if n.is_finite() => n,
                _ => return Err(validation(&p, format!("expected a number, got \"{t}\""))),
            }
        }
    }))
}

/// Optional flag: stored as integer 0 or 1; anything else is an error.
pub(crate) fn read_opt_bool(attrs: &Attrs, name: &str, path: &str) -> Result<Option<bool>> {
    match read_opt_int(attrs, name, path)? {
        None => Ok(None),
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        Some(n) => Err(validation(
            attr_path(path, name),
            format!("expected 0 or 1, got {n}"),
        )),
    }
}

/// Every attribute not in `known`, copied so the model never aliases the input tree.
pub(crate) fn take_extra(attrs: &Attrs, known: &[&str]) -> Extra {
    attrs
        .iter()
        .filter(|(n, _)| !known.contains(&n.as_str()))
        .map(|(n, a)| (n.clone(), a.clone()))
        .collect()
}

// --- writers: an absent number or flag is stored as an empty utf8 string, as core's typed writers did ---

pub(crate) fn str_attr(v: Option<&str>) -> Attr {
    Attr::scalar(v.unwrap_or(""))
}

pub(crate) fn int_attr(v: Option<i64>, path: &str) -> Result<Attr> {
    match v {
        None => Ok(str_attr(None)),
        Some(n) => match i32::try_from(n) {
            Ok(n) => Ok(Attr::scalar(n)),
            Err(_) => Err(validation(
                path,
                format!("{n} is not an integer in the i32 range"),
            )),
        },
    }
}

pub(crate) fn float_attr(v: Option<f64>) -> Attr {
    match v {
        None => str_attr(None),
        Some(n) => Attr::scalar(n),
    }
}

pub(crate) fn bool_attr(v: Option<bool>) -> Attr {
    match v {
        None => str_attr(None),
        Some(b) => Attr::scalar(i32::from(b)),
    }
}

/// Adds `extra` to `attrs`; a name the codec itself writes is an error rather than an override.
pub(crate) fn put_extra(
    attrs: &mut Attrs,
    extra: &Extra,
    known: &[&str],
    path: &str,
) -> Result<()> {
    for (name, a) in extra {
        if known.contains(&name.as_str()) {
            return Err(validation(
                attr_path(path, name),
                "extra attribute collides with a modelled attribute",
            ));
        }
        attrs.insert(name.clone(), a.clone());
    }
    Ok(())
}

pub(crate) fn node_name(name: &str, path: &str) -> Result<()> {
    if name.is_empty() || name.contains('/') || name.contains('\0') {
        return Err(validation(
            path,
            format!("invalid name \"{name}\": must be non-empty and contain no \"/\" or NUL"),
        ));
    }
    Ok(())
}
