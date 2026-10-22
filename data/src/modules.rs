//! A module's names from Rust data code (LLP 1133 §5, LLP 1133.002 D13):
//! what a generated binding builds a request with and reads a reply by. The
//! binding itself is generated from the app's plan (`contract::module_rust`)
//! into a data crate's `OUT_DIR`: a record per shape and per call's options,
//! and per name a request builder and a reply parser. A source returns the
//! request as `Answer::Later` from `answer` and reads the reply in `parse`;
//! the runner admits the body as it admits a Contract call. Rust has no
//! synchronous form (D14).

use exact_runner::{DataError, Outcome, Request};
pub use serde_json;
use serde_json::{Map, Value as Json};

/// A call that failed: one of the module's declared codes (`PERMISSION_DENIED`,
/// `TIMEOUT`, …, or `NotSupportedError`), with what TypeScript's rejection
/// says beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleError {
    pub code: String,
    pub message: String,
}

impl ModuleError {
    pub fn new(code: &str, message: impl Into<String>) -> ModuleError {
        ModuleError {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ModuleError {}

impl From<ModuleError> for DataError {
    fn from(e: ModuleError) -> DataError {
        DataError::Unavailable(e.message)
    }
}

/// A value a binding carries, as the wire's JSON. Encoding leaves an omitted
/// option out (`None`), so it stays the platform's (D2), and fails on what
/// JSON cannot carry (a non-finite number) or a value outside an enum.
/// Decoding takes an absent or `null` option as `None` and ignores keys the
/// shape does not name, as TypeScript's reading does.
pub trait ModuleValue: Sized {
    fn encode(&self) -> Result<Option<Json>, String>;
    fn decode(json: &Json) -> Result<Self, String>;
}

impl ModuleValue for f64 {
    fn encode(&self) -> Result<Option<Json>, String> {
        serde_json::Number::from_f64(*self)
            .map(|n| Some(Json::Number(n)))
            .ok_or_else(|| format!("{self} is not a number JSON carries"))
    }
    fn decode(json: &Json) -> Result<Self, String> {
        json.as_f64()
            .ok_or_else(|| format!("{json} is not a number"))
    }
}

impl ModuleValue for bool {
    fn encode(&self) -> Result<Option<Json>, String> {
        Ok(Some(Json::Bool(*self)))
    }
    fn decode(json: &Json) -> Result<Self, String> {
        json.as_bool()
            .ok_or_else(|| format!("{json} is not a bool"))
    }
}

impl ModuleValue for String {
    fn encode(&self) -> Result<Option<Json>, String> {
        Ok(Some(Json::String(self.clone())))
    }
    fn decode(json: &Json) -> Result<Self, String> {
        json.as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{json} is not a string"))
    }
}

impl<T: ModuleValue> ModuleValue for Option<T> {
    fn encode(&self) -> Result<Option<Json>, String> {
        self.as_ref().map_or(Ok(None), T::encode)
    }
    fn decode(json: &Json) -> Result<Self, String> {
        if json.is_null() {
            return Ok(None);
        }
        T::decode(json).map(Some)
    }
}

impl<T: ModuleValue> ModuleValue for Vec<T> {
    fn encode(&self) -> Result<Option<Json>, String> {
        let items = self.iter().map(|v| Ok(v.encode()?.unwrap_or(Json::Null)));
        Ok(Some(Json::Array(items.collect::<Result<_, String>>()?)))
    }
    fn decode(json: &Json) -> Result<Self, String> {
        let items = json
            .as_array()
            .ok_or_else(|| format!("{json} is not a list"))?;
        items.iter().map(T::decode).collect()
    }
}

/// A record's field into `object`, under its declared name.
pub fn put<T: ModuleValue>(
    object: &mut Map<String, Json>,
    key: &str,
    value: &T,
) -> Result<(), String> {
    if let Some(json) = value.encode().map_err(|e| format!("{key}: {e}"))? {
        object.insert(key.into(), json);
    }
    Ok(())
}

/// A record's field from `object`: absent is `null`, which an option reads
/// as `None` and anything else refuses.
pub fn take<T: ModuleValue>(object: &Json, key: &str) -> Result<T, String> {
    T::decode(object.get(key).unwrap_or(&Json::Null)).map_err(|e| format!("{key}: {e}"))
}

/// A string field's value, refused unless it is one of the enum's `members`.
pub fn member(value: &str, members: &[&str]) -> Result<(), String> {
    match members.contains(&value) {
        true => Ok(()),
        false => Err(format!("{value:?} is not one of {}", members.join(", "))),
    }
}

/// The request for `extension`'s `name` (`verb` is `source` or `send`) with
/// these keyword arguments: LLP 1124 D5's body.
pub fn request(extension: &str, verb: &str, name: &str, args: Json) -> Request {
    let body = serde_json::json!({ "extension": extension, verb: name, "args": args });
    Request::native(body.to_string().into_bytes())
}

/// The reply to `extension.name`'s request: its JSON, or the module's error.
/// A module's failure is a 200 whose body is `{"error": code}` alone; a
/// reply that is no module's at all (a host with none) is `NotSupportedError`.
pub fn reply(name: &str, outcome: &Outcome) -> Result<Json, ModuleError> {
    let unsupported = |why: String| {
        ModuleError::new(
            "NotSupportedError",
            format!("{name}: NotSupportedError ({why})"),
        )
    };
    let body = match outcome {
        Outcome::Response(r) if r.status == 200 => &r.body,
        Outcome::Response(r) => return Err(unsupported(format!("status {}", r.status))),
        Outcome::Failed { message, .. } => return Err(unsupported(message.clone())),
        other => return Err(unsupported(other.summary())),
    };
    let json: Json = serde_json::from_slice(body)
        .map_err(|e| unsupported(format!("the reply is not JSON: {e}")))?;
    // `{"error": code}`, with a `message` beside it or not, as TypeScript's
    // binding reads it.
    let error = json
        .as_object()
        .filter(|o| o.keys().all(|k| k == "error" || k == "message"))
        .and_then(|o| {
            Some((
                o.get("error")?.as_str()?,
                o.get("message").and_then(Json::as_str),
            ))
        });
    match error {
        Some((code, message)) => Err(ModuleError::new(
            code,
            message.map_or_else(|| format!("{name}: {code}"), str::to_owned),
        )),
        None => Ok(json),
    }
}

/// A reply that is not the declared shape.
pub fn malformed(name: &str, why: String) -> ModuleError {
    ModuleError::new(
        "NotSupportedError",
        format!("{name}: the reply is not its shape: {why}"),
    )
}

/// The most bytes one call carries: base64 makes them 4/3, inside the native
/// request's 4 MiB (LLP 1133.006 D8 item 1).
pub const MAX_CALL_BYTES: usize = 2 * 1024 * 1024;

/// Bytes as the wire carries them, `{"$bytes": base64}`; `DataError` past
/// [`MAX_CALL_BYTES`], naming the path form.
pub fn bytes(name: &str, value: &[u8]) -> Result<Json, ModuleError> {
    if value.len() > MAX_CALL_BYTES {
        return Err(ModuleError::new(
            "DataError",
            format!(
                "{name}: {} bytes is more than the {MAX_CALL_BYTES} a call carries; write them to an app:/ file and pass its path",
                value.len()
            ),
        ));
    }
    Ok(serde_json::json!({ "$bytes": crate::envelope::base64(value) }))
}

/// An `app:/` path as the wire carries it, `{"$path": …}`: the runner admits
/// it under the source's `fs.read`, and the host hands the implementation
/// the file (LLP 1133.006 D8 item 3).
pub fn path(value: &str) -> Json {
    serde_json::json!({ "$path": value })
}

/// The place a call of a sequence takes (LLP 1133.006 D2): `args` gains
/// `$sequence`, so the implementation supersedes an earlier call of it that
/// has not landed.
pub fn sequence(args: &mut Json, counter: &std::sync::atomic::AtomicU64) {
    let seq = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if let Some(o) = args.as_object_mut() {
        o.insert("$sequence".into(), serde_json::json!({ "seq": seq }));
    }
}
