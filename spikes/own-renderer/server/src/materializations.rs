//! **A materialization kept, and served** (ADR-0277).
//!
//! A materialization that derives its value is a component the host runs:
//! its body reads what it depends on, `query R(..)`, by the platform's reads
//! (`pw:host/reads`), and the host answers each from R. Its value is kept as
//! the materializer's entry, whose body is text, so the value is encoded here
//! in a form that reads back as it was: each case and field by name, each
//! number by its width. `val_to_json`, the speculation module's form, keeps
//! no width and no case for a record, and reads back as nothing.

use std::collections::BTreeMap;

use pw_host::engine::Val;

/// **Each kept entry, by its materialization's path and its key's text, and
/// the arguments it was derived with**: what an event may reach.
pub type Kept = BTreeMap<(String, Vec<String>), Vec<Val>>;

/// **A read the body asked and was not yet answered**: its import, and the
/// values it was given.
pub type Asked = Option<(String, Vec<Val>)>;

/// **A value as its entry holds it**: an object of one key, naming the kind
/// of the value, and what it holds. A handle (a resource, a future, a
/// stream) is no value an entry can hold, and is refused.
pub fn encode(v: &Val) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let all = |items: &[Val]| -> Result<Vec<serde_json::Value>, String> {
        items.iter().map(encode).collect()
    };
    let some = |v: &Option<Box<Val>>| -> Result<serde_json::Value, String> {
        match v {
            Some(v) => encode(v),
            None => Ok(serde_json::Value::Null),
        }
    };
    Ok(match v {
        Val::Bool(b) => json!({ "bool": b }),
        Val::S8(n) => json!({ "s8": n }),
        Val::U8(n) => json!({ "u8": n }),
        Val::S16(n) => json!({ "s16": n }),
        Val::U16(n) => json!({ "u16": n }),
        Val::S32(n) => json!({ "s32": n }),
        Val::U32(n) => json!({ "u32": n }),
        Val::S64(n) => json!({ "s64": n }),
        Val::U64(n) => json!({ "u64": n }),
        // By its bits: a NaN or an infinity is no JSON number, and the bits
        // read back as the number they were.
        Val::Float32(f) => json!({ "f32": f.to_bits() }),
        Val::Float64(f) => json!({ "f64": f.to_bits() }),
        Val::Char(c) => json!({ "char": c.to_string() }),
        Val::String(s) => json!({ "string": s }),
        Val::List(items) => json!({ "list": all(items)? }),
        Val::FixedLengthList(items) => json!({ "fixed": all(items)? }),
        Val::Tuple(items) => json!({ "tuple": all(items)? }),
        Val::Map(pairs) => json!({
            "map": pairs
                .iter()
                .map(|(k, v)| Ok(json!([encode(k)?, encode(v)?])))
                .collect::<Result<Vec<_>, String>>()?
        }),
        Val::Record(fields) => json!({
            "record": fields
                .iter()
                .map(|(n, v)| Ok(json!([n, encode(v)?])))
                .collect::<Result<Vec<_>, String>>()?
        }),
        Val::Variant(case, payload) => json!({ "variant": [case, some(payload)?] }),
        Val::Enum(case) => json!({ "enum": case }),
        Val::Option(v) => json!({ "option": some(v)? }),
        Val::Result(r) => match r {
            Ok(v) => json!({ "ok": some(v)? }),
            Err(v) => json!({ "err": some(v)? }),
        },
        Val::Flags(names) => json!({ "flags": names }),
        Val::Resource(_) | Val::Future(_) | Val::Stream(_) | Val::ErrorContext(_) => {
            return Err(format!("a handle is no value an entry holds: {v:?}"));
        }
    })
}

/// **A value as its entry held it**, read back.
pub fn decode(j: &serde_json::Value) -> Result<Val, String> {
    let wrong = || format!("no value an entry holds: {j}");
    let object = j.as_object().filter(|o| o.len() == 1).ok_or_else(wrong)?;
    let (kind, held) = object.iter().next().ok_or_else(wrong)?;
    let int = |held: &serde_json::Value| held.as_i64().ok_or_else(wrong);
    let uint = |held: &serde_json::Value| held.as_u64().ok_or_else(wrong);
    let all = |held: &serde_json::Value| -> Result<Vec<Val>, String> {
        held.as_array()
            .ok_or_else(wrong)?
            .iter()
            .map(decode)
            .collect()
    };
    let some = |held: &serde_json::Value| -> Result<Option<Box<Val>>, String> {
        match held {
            serde_json::Value::Null => Ok(None),
            v => Ok(Some(Box::new(decode(v)?))),
        }
    };
    Ok(match kind.as_str() {
        "bool" => Val::Bool(held.as_bool().ok_or_else(wrong)?),
        "s8" => Val::S8(i8::try_from(int(held)?).map_err(|_| wrong())?),
        "u8" => Val::U8(u8::try_from(uint(held)?).map_err(|_| wrong())?),
        "s16" => Val::S16(i16::try_from(int(held)?).map_err(|_| wrong())?),
        "u16" => Val::U16(u16::try_from(uint(held)?).map_err(|_| wrong())?),
        "s32" => Val::S32(i32::try_from(int(held)?).map_err(|_| wrong())?),
        "u32" => Val::U32(u32::try_from(uint(held)?).map_err(|_| wrong())?),
        "s64" => Val::S64(int(held)?),
        "u64" => Val::U64(uint(held)?),
        "f32" => Val::Float32(f32::from_bits(
            u32::try_from(uint(held)?).map_err(|_| wrong())?,
        )),
        "f64" => Val::Float64(f64::from_bits(uint(held)?)),
        "char" => {
            let s = held.as_str().ok_or_else(wrong)?;
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Val::Char(c),
                _ => return Err(wrong()),
            }
        }
        "string" => Val::String(held.as_str().ok_or_else(wrong)?.to_string()),
        "list" => Val::List(all(held)?),
        "fixed" => Val::FixedLengthList(all(held)?),
        "tuple" => Val::Tuple(all(held)?),
        "map" => Val::Map(
            held.as_array()
                .ok_or_else(wrong)?
                .iter()
                .map(|pair| match pair.as_array().map(Vec::as_slice) {
                    Some([k, v]) => Ok((decode(k)?, decode(v)?)),
                    _ => Err(wrong()),
                })
                .collect::<Result<_, String>>()?,
        ),
        "record" => Val::Record(
            held.as_array()
                .ok_or_else(wrong)?
                .iter()
                .map(|field| match field.as_array().map(Vec::as_slice) {
                    Some([n, v]) => Ok((n.as_str().ok_or_else(wrong)?.to_string(), decode(v)?)),
                    _ => Err(wrong()),
                })
                .collect::<Result<_, String>>()?,
        ),
        "variant" => match held.as_array().map(Vec::as_slice) {
            Some([case, payload]) => {
                Val::Variant(case.as_str().ok_or_else(wrong)?.to_string(), some(payload)?)
            }
            _ => return Err(wrong()),
        },
        "enum" => Val::Enum(held.as_str().ok_or_else(wrong)?.to_string()),
        "option" => Val::Option(some(held)?),
        "ok" => Val::Result(Ok(some(held)?)),
        "err" => Val::Result(Err(some(held)?)),
        "flags" => Val::Flags(
            held.as_array()
                .ok_or_else(wrong)?
                .iter()
                .map(|n| n.as_str().map(str::to_string).ok_or_else(wrong))
                .collect::<Result<_, String>>()?,
        ),
        _ => return Err(wrong()),
    })
}

/// A value as its entry's text holds it.
pub fn encode_text(v: &Val) -> Result<String, String> {
    encode(v).map(|j| j.to_string())
}

/// A value as its entry's text held it, read back.
pub fn decode_text(text: &str) -> Result<Val, String> {
    decode(&serde_json::from_str(text).map_err(|e| format!("an entry's text: {e}"))?)
}

/// **A value's text in a key** (ADR-0277): a string as it is, a number or a
/// `Bool` as it is written, and anything else as its entry holds it. The
/// text an event's value is compared with, by `Graph::reaches`, at the same
/// position of an entry's key: the store's ids are strings on both sides.
pub fn key_text(v: &Val) -> String {
    match v {
        Val::String(s) => s.clone(),
        Val::Bool(b) => b.to_string(),
        Val::S8(n) => n.to_string(),
        Val::U8(n) => n.to_string(),
        Val::S16(n) => n.to_string(),
        Val::U16(n) => n.to_string(),
        Val::S32(n) => n.to_string(),
        Val::U32(n) => n.to_string(),
        Val::S64(n) => n.to_string(),
        Val::U64(n) => n.to_string(),
        other => encode_text(other).unwrap_or_else(|e| e),
    }
}

/// **A read's answer, by what was asked** (ADR-0277): the read and the
/// values it was given, as their entry would hold them, so one derivation
/// answers the same question once.
pub fn asked(read: &str, given: &[Val]) -> Result<String, String> {
    let given: Vec<serde_json::Value> = given.iter().map(encode).collect::<Result<_, _>>()?;
    Ok(format!("{read}{}", serde_json::Value::Array(given)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each kind of value an entry holds reads back as it was, its widths
    /// and its cases kept.
    #[test]
    fn a_value_reads_back_as_it_was() {
        let record = Val::Record(vec![
            ("sections".to_string(), Val::S64(4)),
            ("items".to_string(), Val::S64(12)),
        ]);
        let values = [
            Val::Bool(true),
            Val::S8(-3),
            Val::U8(250),
            Val::S16(-300),
            Val::U16(60_000),
            Val::S32(-70_000),
            Val::U32(4_000_000_000),
            Val::S64(i64::MIN),
            Val::U64(u64::MAX),
            Val::Float32(1.5),
            Val::Float64(f64::NAN),
            Val::Float64(-0.0),
            Val::Char('é'),
            Val::String("12 items in 4 sections".to_string()),
            Val::List(vec![record.clone(), record.clone()]),
            Val::Tuple(vec![Val::S64(1), Val::String("a".into())]),
            Val::Map(vec![(Val::String("k".into()), Val::S64(1))]),
            record,
            Val::Variant("not-found".into(), None),
            Val::Variant("found".into(), Some(Box::new(Val::S64(7)))),
            Val::Enum("open".into()),
            Val::Option(None),
            Val::Option(Some(Box::new(Val::S64(0)))),
            Val::Result(Ok(Some(Box::new(Val::S64(1))))),
            Val::Result(Err(None)),
            Val::Flags(vec!["a".into(), "c".into()]),
            Val::FixedLengthList(vec![Val::U8(1), Val::U8(2)]),
        ];
        for v in values {
            let held = encode(&v).expect("encodes");
            let back = decode(&held).expect("decodes");
            // A NaN is no value equal to itself: its bits are.
            match (&v, &back) {
                (Val::Float64(a), Val::Float64(b)) => assert_eq!(a.to_bits(), b.to_bits()),
                _ => assert_eq!(back, v, "{held}"),
            }
        }
    }

    /// The control: what no entry holds is refused, not read as something.
    #[test]
    fn what_no_entry_holds_is_refused() {
        for bad in [
            serde_json::json!(12),
            serde_json::json!({}),
            serde_json::json!({ "s64": 1, "u64": 2 }),
            serde_json::json!({ "u8": 300 }),
            serde_json::json!({ "char": "ab" }),
            serde_json::json!({ "record": [["a"]] }),
            serde_json::json!({ "colour": "red" }),
        ] {
            assert!(decode(&bad).is_err(), "{bad}");
        }
    }

    /// A read asked twice with the same values is one question; another
    /// value, or another read, is another.
    #[test]
    fn a_read_is_asked_by_its_values() {
        let a = asked("pw:host/reads#m", &[Val::String("47".into())]).expect("asked");
        assert_eq!(
            a,
            asked("pw:host/reads#m", &[Val::String("47".into())]).expect("asked")
        );
        assert_ne!(
            a,
            asked("pw:host/reads#m", &[Val::String("48".into())]).expect("asked")
        );
        assert_ne!(
            a,
            asked("pw:host/reads#n", &[Val::String("47".into())]).expect("asked")
        );
    }
}
