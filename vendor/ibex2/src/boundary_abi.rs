//! The C ABI carrying `hostCall(op, args)` across the engine seam.
//!
//! LLP 0059.000 §1 asks for **one** surface, so this is one entry point taking
//! an opcode and an argument vector — not N specific entry points, which would
//! be a different design wearing the same words.
//!
//! §1.1: primitives and handles only. A value is a tag plus either a double or
//! a pointer/length pair; strings and byte buffers cross as borrowed spans in
//! and Rust-owned allocations out. Nothing is serialized.
//!
//! @ref LLP 0059.000#1-the-host-call-boundary — one surface, no JSON

use std::ffi::{c_char, c_int, c_uchar};

use crate::boundary::{HostArg, HostError, HostValue};
use crate::grant::GrantSet;
use crate::stdlib::{base64, console, crypto, text, url};

pub const TAG_UNDEFINED: i32 = 0;
pub const TAG_NULL: i32 = 1;
pub const TAG_BOOL: i32 = 2;
pub const TAG_NUMBER: i32 = 3;
pub const TAG_STRING: i32 = 4;
pub const TAG_BYTES: i32 = 5;

/// One value on the boundary. Repr-C so the shim and Rust agree byte for byte.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AbiValue {
    pub tag: i32,
    pub number: f64,
    pub data: *const c_uchar,
    pub len: usize,
}

impl AbiValue {
    fn undefined() -> Self {
        Self {
            tag: TAG_UNDEFINED,
            number: 0.0,
            data: std::ptr::null(),
            len: 0,
        }
    }

    /// Borrow this value as an argument. **No bytes are copied** — see
    /// LLP 0059.000 §1.2. `HostArg` has no owned byte variant, so this cannot
    /// silently regress into a copy.
    ///
    /// # Safety
    /// `data`/`len` must describe a span valid for the duration of the call,
    /// and the returned borrow must not outlive it.
    unsafe fn borrow<'a>(self) -> Result<HostArg<'a>, HostError> {
        Ok(match self.tag {
            TAG_UNDEFINED => HostArg::Undefined,
            TAG_NULL => HostArg::Null,
            TAG_BOOL => HostArg::Bool(self.number != 0.0),
            TAG_NUMBER => HostArg::Number(self.number),
            TAG_STRING => {
                // The engine already produced UTF-8; borrow it rather than
                // re-validating into a new String. Invalid UTF-8 here would be
                // an engine bug, so it is an error rather than a lossy repair.
                let text = std::str::from_utf8(self.span()).map_err(|_| {
                    HostError::InvalidArgument("string argument was not valid UTF-8".into())
                })?;
                HostArg::Str(text)
            }
            TAG_BYTES => HostArg::Bytes(self.span()),
            other => {
                return Err(HostError::InvalidArgument(format!(
                    "unknown value tag {other}"
                )))
            }
        })
    }

    /// Borrow a byte argument mutably, so Rust writes through to the engine's
    /// own buffer.
    ///
    /// This is the inbound half of §1.2 and the reason `ArrayBuffer` is in the
    /// boundary contract at all: `encodeInto` must be visible in the buffer the
    /// caller already holds, and an `encodeInto` that copies has implemented
    /// `encode` with extra steps.
    ///
    /// # Safety
    /// The span must be valid, writable, and unaliased for the call.
    unsafe fn borrow_mut<'a>(self) -> Option<&'a mut [u8]> {
        if self.tag != TAG_BYTES || self.data.is_null() || self.len == 0 {
            return None;
        }
        Some(std::slice::from_raw_parts_mut(
            self.data as *mut u8,
            self.len,
        ))
    }

    unsafe fn span<'a>(self) -> &'a [u8] {
        if self.data.is_null() || self.len == 0 {
            return &[];
        }
        std::slice::from_raw_parts(self.data, self.len)
    }
}

/// Ops the pure tier answers. Delegating and ambient ops arrive with the
/// job-queue adapter; they are not squeezed into this synchronous path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Op {
    ConsoleLog = 1,
    ConsoleInfo = 2,
    ConsoleDebug = 3,
    ConsoleWarn = 4,
    ConsoleError = 5,
    Btoa = 10,
    Atob = 11,
    TextEncode = 20,
    TextDecode = 21,
    TextEncodeInto = 22,
    UrlParse = 30,
    UrlSearchParamsGet = 31,
    UrlSet = 32,
    UrlSearchParamsGetAll = 33,
    UrlSearchParamsHas = 34,
    UrlSearchParamsSet = 35,
    UrlSearchParamsAppend = 36,
    UrlSearchParamsDelete = 37,
    UrlSearchParamsSort = 38,
    UrlSearchParamsEntries = 39,
    UrlSearchParamsNormalize = 29,
    HeadersNew = 40,
    HeadersAppend = 41,
    HeadersSet = 42,
    HeadersGet = 43,
    HeadersHas = 44,
    HeadersDelete = 45,
    HeadersCount = 46,
    HeadersNameAt = 47,
    HeadersValueAt = 48,
    HeadersValidName = 49,
    HeadersValidValue = 50,
    HeadersFree = 51,
    TimerSet = 60,
    TimerSetRepeating = 61,
    TimerClear = 62,
    PerformanceNow = 63,
    CryptoRandomUuid = 70,
    CryptoGetRandomValues = 71,
    FetchControl = 72,
    SqliteResult = 80,
}

impl Op {
    fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            1 => Op::ConsoleLog,
            2 => Op::ConsoleInfo,
            3 => Op::ConsoleDebug,
            4 => Op::ConsoleWarn,
            5 => Op::ConsoleError,
            10 => Op::Btoa,
            11 => Op::Atob,
            20 => Op::TextEncode,
            21 => Op::TextDecode,
            22 => Op::TextEncodeInto,
            29 => Op::UrlSearchParamsNormalize,
            30 => Op::UrlParse,
            31 => Op::UrlSearchParamsGet,
            32 => Op::UrlSet,
            33 => Op::UrlSearchParamsGetAll,
            34 => Op::UrlSearchParamsHas,
            35 => Op::UrlSearchParamsSet,
            36 => Op::UrlSearchParamsAppend,
            37 => Op::UrlSearchParamsDelete,
            38 => Op::UrlSearchParamsSort,
            39 => Op::UrlSearchParamsEntries,
            40 => Op::HeadersNew,
            41 => Op::HeadersAppend,
            42 => Op::HeadersSet,
            43 => Op::HeadersGet,
            44 => Op::HeadersHas,
            45 => Op::HeadersDelete,
            46 => Op::HeadersCount,
            47 => Op::HeadersNameAt,
            48 => Op::HeadersValueAt,
            49 => Op::HeadersValidName,
            50 => Op::HeadersValidValue,
            51 => Op::HeadersFree,
            60 => Op::TimerSet,
            61 => Op::TimerSetRepeating,
            62 => Op::TimerClear,
            63 => Op::PerformanceNow,
            70 => Op::CryptoRandomUuid,
            71 => Op::CryptoGetRandomValues,
            72 => Op::FetchControl,
            80 => Op::SqliteResult,
            _ => return None,
        })
    }

    fn console_level(self) -> Option<console::Level> {
        Some(match self {
            Op::ConsoleLog => console::Level::Log,
            Op::ConsoleInfo => console::Level::Info,
            Op::ConsoleDebug => console::Level::Debug,
            Op::ConsoleWarn => console::Level::Warn,
            Op::ConsoleError => console::Level::Error,
            _ => return None,
        })
    }
}

thread_local! {
    /// The console queue for this runtime's thread.
    ///
    /// Thread-local rather than global: a runtime belongs to one thread, and a
    /// process-wide queue would interleave two runtimes' output. Note this is
    /// buffering, NOT authority — LLP 0060 D1 forbids ambient authority, and
    /// nothing capability-bearing may ever live here.
    static CONSOLE: std::cell::RefCell<console::Console> =
        std::cell::RefCell::new(console::Console::with_capacity(4096));
}

/// Drain this thread's console queue.
pub fn drain_console() -> Vec<console::Record> {
    CONSOLE.with(|c| c.borrow_mut().drain())
}

/// Report an error that escaped a host task's callback — a timer's, a
/// settlement's — as a console error. The pump catches so one throwing
/// callback does not stop the tasks behind it; before this it also said
/// nothing, and an application whose timer threw saw the timer stop and
/// nothing else.
///
/// # Safety
/// `message` must be a valid NUL-terminated string or null.
#[no_mangle]
pub unsafe extern "C" fn ibex2_report_uncaught(message: *const c_char) {
    let text = if message.is_null() {
        "uncaught error".to_string()
    } else {
        std::ffi::CStr::from_ptr(message)
            .to_string_lossy()
            .into_owned()
    };
    let line = format!("Uncaught {text}");
    CONSOLE.with(|c| {
        c.borrow_mut()
            .write(console::Level::Error, &[HostArg::Str(&line)])
    });
}

fn dispatch(
    op: Op,
    args: &[HostArg],
    state: Option<&crate::task::RuntimeState>,
) -> Result<HostValue, HostError> {
    if let Some(result) = crate::stdlib::headers_ops::dispatch(op as u32, args, state) {
        return result;
    }
    if let Some(level) = op.console_level() {
        CONSOLE.with(|c| c.borrow_mut().write(level, args));
        return Ok(HostValue::Undefined);
    }

    let first_str = |label: &str| -> Result<&str, HostError> {
        args.first()
            .and_then(HostArg::as_str)
            .ok_or_else(|| HostError::InvalidArgument(format!("{label} expects a string")))
    };
    fn str_at<'a>(args: &'a [HostArg<'a>], index: usize, what: &str) -> Result<&'a str, HostError> {
        args.get(index)
            .and_then(HostArg::as_str)
            .ok_or_else(|| HostError::InvalidArgument(format!("expected {what}")))
    }

    if matches!(
        op,
        Op::TimerSet | Op::TimerSetRepeating | Op::TimerClear | Op::PerformanceNow
    ) {
        let state = state.ok_or_else(|| HostError::Failed("no runtime state".into()))?;
        let number = |index: usize| -> f64 {
            match args.get(index) {
                Some(HostArg::Number(n)) => *n,
                _ => 0.0,
            }
        };
        return Ok(match op {
            Op::PerformanceNow => HostValue::Number(state.now()),
            Op::TimerClear => {
                state.clear_timer(number(0) as u64);
                HostValue::Undefined
            }
            _ => HostValue::Number(state.set_timer(number(0), op == Op::TimerSetRepeating) as f64),
        });
    }

    match op {
        Op::CryptoRandomUuid => crypto::random_uuid().map(HostValue::Str),
        Op::SqliteResult => crate::sqlite_abi::result_field(args, state),
        Op::FetchControl => {
            let state = state.ok_or_else(|| HostError::Failed("no runtime state".into()))?;
            match args.first() {
                Some(HostArg::Number(0.0)) => Ok(HostValue::Number(state.create_control() as f64)),
                Some(HostArg::Number(action)) if *action == 1.0 || *action == 2.0 => {
                    let handle = match args.get(1) {
                        Some(HostArg::Number(n)) => valid_handle(*n)?,
                        _ => {
                            return Err(HostError::InvalidArgument(
                                "fetch control handle expected".into(),
                            ))
                        }
                    };
                    if *action == 1.0 {
                        if let Some(control) = state.control(handle) {
                            control.abort();
                        }
                    } else {
                        state.release_control(handle);
                    }
                    Ok(HostValue::Undefined)
                }
                _ => Err(HostError::InvalidArgument(
                    "unknown fetch control operation".into(),
                )),
            }
        }
        Op::Btoa => base64::btoa(first_str("btoa")?).map(HostValue::Str),
        Op::Atob => base64::atob(first_str("atob")?).map(HostValue::Str),
        Op::TextEncode => Ok(HostValue::Bytes(text::encode(first_str("encode")?))),
        Op::TextDecode => {
            let bytes = args
                .first()
                .and_then(HostArg::as_bytes)
                .ok_or_else(|| HostError::InvalidArgument("decode expects bytes".into()))?;
            let fatal = matches!(args.get(1), Some(HostArg::Bool(true)));
            let ignore_bom = matches!(args.get(2), Some(HostArg::Bool(true)));
            let on_invalid = if fatal {
                text::OnInvalid::Throw
            } else {
                text::OnInvalid::Replace
            };
            text::decode(bytes, on_invalid, ignore_bom).map(HostValue::Str)
        }
        Op::UrlParse => {
            let base = args.get(1).and_then(HostArg::as_str);
            // Every component, one per line: the object shape is the binding
            // layer's job, and it should not have to ask eleven times.
            url::parse(first_str("URL")?, base).map(|parsed| HostValue::Str(parsed.joined()))
        }
        Op::UrlSet => {
            let field = str_at(args, 1, "a field name")?;
            let value = str_at(args, 2, "a value")?;
            url::set(first_str("URL")?, field, value).map(|parsed| HostValue::Str(parsed.joined()))
        }
        // URLSearchParams: the object's whole state is its query string, and
        // every method is one crossing that reads or rewrites it here, so the
        // list semantics — order, duplicates, form-urlencoded escaping — have
        // exactly one implementation.
        Op::UrlSearchParamsNormalize => Ok(HostValue::Str(
            url::SearchParams::parse(first_str("URLSearchParams")?).to_query_string(),
        )),
        Op::UrlSearchParamsGet => {
            let params = url::SearchParams::parse(first_str("URLSearchParams")?);
            Ok(match params.get(str_at(args, 1, "a name")?) {
                Some(value) => HostValue::Str(value.to_string()),
                None => HostValue::Null,
            })
        }
        Op::UrlSearchParamsGetAll => {
            let params = url::SearchParams::parse(first_str("URLSearchParams")?);
            Ok(HostValue::Str(
                params.get_all_json(str_at(args, 1, "a name")?),
            ))
        }
        Op::UrlSearchParamsHas => {
            let params = url::SearchParams::parse(first_str("URLSearchParams")?);
            let name = str_at(args, 1, "a name")?;
            Ok(HostValue::Bool(
                match args.get(2).and_then(HostArg::as_str) {
                    Some(value) => params.has_pair(name, value),
                    None => params.has(name),
                },
            ))
        }
        Op::UrlSearchParamsSet | Op::UrlSearchParamsAppend => {
            let mut params = url::SearchParams::parse(first_str("URLSearchParams")?);
            let name = str_at(args, 1, "a name")?;
            let value = str_at(args, 2, "a value")?;
            if op == Op::UrlSearchParamsSet {
                params.set(name, value);
            } else {
                params.append(name, value);
            }
            Ok(HostValue::Str(params.to_query_string()))
        }
        Op::UrlSearchParamsDelete => {
            let mut params = url::SearchParams::parse(first_str("URLSearchParams")?);
            let name = str_at(args, 1, "a name")?;
            match args.get(2).and_then(HostArg::as_str) {
                Some(value) => params.delete_pair(name, value),
                None => params.delete(name),
            }
            Ok(HostValue::Str(params.to_query_string()))
        }
        Op::UrlSearchParamsSort => {
            let mut params = url::SearchParams::parse(first_str("URLSearchParams")?);
            params.sort();
            Ok(HostValue::Str(params.to_query_string()))
        }
        Op::UrlSearchParamsEntries => Ok(HostValue::Str(
            url::SearchParams::parse(first_str("URLSearchParams")?).entries_json(),
        )),
        Op::TextEncodeInto | Op::CryptoGetRandomValues => {
            unreachable!("handled in ibex2_host_call, which owns the mutable span")
        }
        _ => unreachable!("console ops returned above"),
    }
}

/// The single host-call entry point.
///
/// Returns 0 on success and 1 when the operation failed or was refused; `out`
/// receives the result or the error message either way. Any string or byte
/// buffer in `out` is Rust-owned and must be released with
/// `ibex2_host_release`.
///
/// # Safety
/// `argv` must point to `argc` initialized `AbiValue`s whose spans are valid
/// for this call, and `out` must be a valid writable pointer. Write-through
/// operations require exclusive, writable destination spans, disjoint from
/// the argument descriptors and `out`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_host_call(
    state: *const crate::task::RuntimeState,
    op: u32,
    argv: *const AbiValue,
    argc: usize,
    out: *mut AbiValue,
) -> c_int {
    if out.is_null() {
        return -1;
    }
    *out = AbiValue::undefined();

    let raw = if argv.is_null() || argc == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(argv, argc)
    };

    // Validate before borrowing mutably: unlike ordinary arguments this span
    // is written through. No shared slice into the buffer may coexist with it.
    // The binding passes an ArrayBuffer and intrinsic view offset/length,
    // never user-overridable TypedArray properties.
    if op == Op::CryptoGetRandomValues as u32 {
        let result = fill_random_view(raw);
        return match result {
            Ok(()) => 0,
            Err(err) => fail(out, &err.to_string()),
        };
    }

    let mut args = Vec::with_capacity(raw.len());
    for value in raw {
        match value.borrow() {
            Ok(value) => args.push(value),
            Err(err) => return fail(out, &err.to_string()),
        }
    }

    let state = crate::task::clone_queue(state);
    #[cfg(all(feature = "hermes", target_os = "linux"))]
    if let Some(result) = crate::stdlib::intl::dispatch(op, &args, state.as_deref()) {
        return match result {
            Ok(value) => {
                *out = leak_value(value);
                0
            }
            Err(err) => fail(out, &err.to_string()),
        };
    }
    #[cfg(all(feature = "hermes", target_os = "linux"))]
    if let Some(result) = crate::stdlib::intl_datetime::dispatch(op, &args, state.as_deref()) {
        return match result {
            Ok(value) => {
                *out = leak_value(value);
                0
            }
            Err(err) => fail(out, &err.to_string()),
        };
    }
    #[cfg(all(feature = "hermes", target_os = "linux"))]
    if let Some(result) = crate::stdlib::intl_case::dispatch(op, &args) {
        return match result {
            Ok(value) => {
                *out = leak_value(value);
                0
            }
            Err(err) => fail(out, &err.to_string()),
        };
    }

    let Some(op) = Op::from_u32(op) else {
        return fail(out, &format!("unknown host op {op}"));
    };

    // encodeInto is the one op that writes through to the caller's buffer, so
    // it takes the mutable span here rather than through the shared immutable
    // argument vector. Arg 0 is the source string, arg 1 the destination.
    if op == Op::TextEncodeInto {
        let Some(source) = args.first().and_then(HostArg::as_str) else {
            return fail(out, "encodeInto expects a string source");
        };
        let Some(destination) = raw.get(1).copied().and_then(|value| value.borrow_mut()) else {
            return fail(out, "encodeInto expects a writable destination buffer");
        };
        let (read, written) = crate::stdlib::text::encode_into(source, destination);
        // (read, written) packed as the spec's two fields; the binding layer
        // turns this into { read, written }.
        *out = leak_value(HostValue::Str(format!("{read},{written}")));
        return 0;
    }

    match dispatch(op, &args, state.as_deref()) {
        Ok(value) => {
            *out = leak_value(value);
            0
        }
        Err(err) => fail(out, &err.to_string()),
    }
}

// SAFETY: same span validity and exclusive-write requirements as host_call.
unsafe fn fill_random_view(raw: &[AbiValue]) -> Result<(), HostError> {
    let invalid =
        || HostError::InvalidArgument("getRandomValues expects a buffer, offset and length".into());
    let buffer = raw
        .first()
        .filter(|v| v.tag == TAG_BYTES)
        .ok_or_else(invalid)?;
    let index = |i: usize| -> Result<usize, HostError> {
        let v = raw
            .get(i)
            .filter(|v| v.tag == TAG_NUMBER)
            .ok_or_else(invalid)?;
        if !v.number.is_finite()
            || v.number < 0.0
            || v.number.fract() != 0.0
            || v.number > 9_007_199_254_740_991.0
            || v.number >= usize::MAX as f64
        {
            return Err(invalid());
        }
        Ok(v.number as usize)
    };
    let offset = index(1)?;
    let length = index(2)?;
    if offset > buffer.len || length > buffer.len - offset {
        return Err(invalid());
    }
    // Enforce the quota before creating a writable borrow or touching entropy.
    crypto::check_length(length)?;
    if length == 0 {
        return Ok(());
    }
    if buffer.data.is_null() {
        return Err(invalid());
    }
    let destination = std::slice::from_raw_parts_mut((buffer.data as *mut u8).add(offset), length);
    crypto::get_random_values(destination)
}

fn fail(out: *mut AbiValue, message: &str) -> c_int {
    // SAFETY: callers check `out` for null before reaching here.
    unsafe { *out = leak_value(HostValue::Str(message.to_string())) };
    1
}

/// Move a value into a Rust allocation the shim borrows until it releases it.
fn leak_value(value: HostValue) -> AbiValue {
    match value {
        HostValue::Undefined => AbiValue::undefined(),
        HostValue::Null => AbiValue {
            tag: TAG_NULL,
            ..AbiValue::undefined()
        },
        HostValue::Bool(b) => AbiValue {
            tag: TAG_BOOL,
            number: if b { 1.0 } else { 0.0 },
            ..AbiValue::undefined()
        },
        HostValue::Number(n) => AbiValue {
            tag: TAG_NUMBER,
            number: n,
            ..AbiValue::undefined()
        },
        HostValue::Str(text) => {
            let boxed = text.into_bytes().into_boxed_slice();
            let len = boxed.len();
            AbiValue {
                tag: TAG_STRING,
                number: 0.0,
                data: Box::into_raw(boxed) as *const c_uchar,
                len,
            }
        }
        HostValue::Bytes(bytes) => {
            let boxed = bytes.into_boxed_slice();
            let len = boxed.len();
            AbiValue {
                tag: TAG_BYTES,
                number: 0.0,
                data: Box::into_raw(boxed) as *const c_uchar,
                len,
            }
        }
    }
}

/// Build a grant set from a spec and hand ownership to the caller.
///
/// This is how authority becomes *carried* (LLP 0060 D1): the caller binds the
/// result to a binding at install time, and the binding passes it back on every
/// call. There is no way to ask for "the current grants" — no ambient lookup
/// exists, by construction.
///
/// # Safety
/// `spec` must be a NUL-terminated UTF-8 string. Release with
/// `ibex2_grants_destroy`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_grants_create(spec: *const c_char) -> *const GrantSet {
    let spec = if spec.is_null() {
        ""
    } else {
        match std::ffi::CStr::from_ptr(spec).to_str() {
            Ok(text) => text,
            Err(_) => return std::ptr::null(),
        }
    };
    match GrantSet::parse(spec) {
        Ok(set) => std::sync::Arc::into_raw(std::sync::Arc::new(set)),
        Err(_) => std::ptr::null(),
    }
}

/// Retain a binding's grant independently of its installer's lifetime.
///
/// # Safety
/// `grants` must be null or a live Arc-backed grant returned by this ABI
/// (or `bindings::Context::grants_ptr`). Release with `ibex2_grants_destroy`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_grants_retain(grants: *const GrantSet) -> *const GrantSet {
    if !grants.is_null() {
        std::sync::Arc::increment_strong_count(grants);
    }
    grants
}

/// # Safety
/// `grants` must come from `ibex2_grants_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_grants_destroy(grants: *const GrantSet) {
    if !grants.is_null() {
        drop(std::sync::Arc::from_raw(grants));
    }
}

unsafe fn clone_grants(grants: *const GrantSet) -> Option<std::sync::Arc<GrantSet>> {
    if grants.is_null() {
        return None;
    }
    std::sync::Arc::increment_strong_count(grants);
    Some(std::sync::Arc::from_raw(grants))
}

/// Read a field from a stored response.
///
/// A `Response` crosses as a **handle**, not a value: §1.1 forbids serializing
/// at the boundary, and a response is a status plus headers plus a body. These
/// are the accessors the binding layer turns back into a `Response` object.
///
/// # Safety
/// `state` must be live; `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ibex2_response_field(
    state: *const crate::task::RuntimeState,
    handle: f64,
    field: u32,
    name: *const AbiValue,
    out: *mut AbiValue,
) -> c_int {
    if out.is_null() {
        return -1;
    }
    *out = AbiValue::undefined();
    let Some(state) = crate::task::clone_queue(state) else {
        return fail(out, "no runtime state");
    };
    let handle = match valid_handle(handle) {
        Ok(handle) => handle,
        Err(err) => return fail(out, &err.to_string()),
    };
    if field == 8 {
        state.cancel_response(handle);
        return 0;
    }

    let result = state.with_response(handle, |response| match field {
        0 => Ok(HostValue::Number(f64::from(response.status))),
        1 => Ok(HostValue::Bool(response.ok())),
        2 => Ok(HostValue::Str(response.url.clone())),
        3 => {
            let wanted = match name.as_ref().and_then(|v| v.borrow().ok()) {
                Some(HostArg::Str(text)) => text.to_string(),
                _ => return Err(HostError::InvalidArgument("header name expected".into())),
            };
            Ok(match response.headers.get(&wanted) {
                Some(value) => HostValue::Str(value.to_string()),
                None => HostValue::Null,
            })
        }
        5 => Ok(HostValue::Bool(response.redirected)),
        // Every header as JSON pairs, so the binding can build a Headers
        // object once and the record can be released when the body is.
        7 => Ok(HostValue::Str(format!(
            "[{}]",
            response
                .headers
                .sorted_entries()
                .iter()
                .map(|(name, value)| format!(
                    "[{},{}]",
                    crate::stdlib::url::json_string(name),
                    crate::stdlib::url::json_string(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ))),
        other => Err(HostError::InvalidArgument(format!(
            "unknown response field {other}"
        ))),
    });

    match result {
        Some(Ok(value)) => {
            *out = leak_value(value);
            0
        }
        Some(Err(err)) => fail(out, &err.to_string()),
        None => fail(out, "TypeError: unknown response handle"),
    }
}

/// Start a delegating op on another thread.
///
/// Returns immediately. The op is **not** run on the JavaScript thread — that
/// is the entire point of the delegating shape (LLP 0059.000 §1.1) — and it
/// touches no engine state, because nothing in `crate::task` may hold a `jsi`
/// value.
///
/// # Safety
/// `argv` must point to `argc` initialized `AbiValue`s valid for this call.
/// Arguments are copied here, deliberately: they must outlive the call that
/// started the work, so the borrowed-span rule that governs synchronous ops
/// cannot apply.
#[no_mangle]
pub unsafe extern "C" fn ibex2_async_begin(
    state: *const crate::task::RuntimeState,
    grants: *const GrantSet,
    op: u32,
    argv: *const AbiValue,
    argc: usize,
    task_id: u64,
) -> c_int {
    let Some(state) = crate::task::clone_queue(state) else {
        return 1;
    };
    let raw = if argv.is_null() || argc == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(argv, argc)
    };

    // Owned snapshot: the worker outlives this call, so it cannot borrow.
    let mut owned = Vec::with_capacity(raw.len());
    for value in raw {
        match value.borrow() {
            Ok(HostArg::Str(text)) => owned.push(HostValue::Str(text.to_string())),
            Ok(HostArg::Bytes(bytes)) => owned.push(HostValue::Bytes(bytes.to_vec())),
            Ok(HostArg::Number(n)) => owned.push(HostValue::Number(n)),
            Ok(HostArg::Bool(b)) => owned.push(HostValue::Bool(b)),
            Ok(HostArg::Null) => owned.push(HostValue::Null),
            Ok(HostArg::Undefined) => owned.push(HostValue::Undefined),
            Err(_) => return 1,
        }
    }

    let Some(op) = AsyncOp::from_u32(op) else {
        return 1;
    };

    // The binding's own grants, captured at install time and handed back on
    // every call. Nothing here consults ambient state.
    let grants = clone_grants(grants).unwrap_or_else(|| std::sync::Arc::new(GrantSet::none()));

    // Counted before the thread starts, so the loop cannot see an idle moment
    // between "started" and "running".
    state.task_started();
    let work = move || {
        let result = run_async(op, &owned, &state, &grants);
        state.queue.complete(task_id, result);
        state.task_finished();
    };
    if op == AsyncOp::ReadBody {
        crate::pool::run_body(work);
    } else {
        crate::pool::run(work);
    }
    0
}

/// The delegating ops. `fetch` joins this list once the adapter is proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
enum AsyncOp {
    /// Echo the first argument back after leaving the thread. Exists so the
    /// ordering contract can be tested without a network in the way.
    Echo = 100,
    /// `fetch`. Resolves with a response handle, never with a serialized body.
    Fetch = 101,
    ReadBody = 102,
    /// `fs`, one op per method. Delegating and capability-bearing, so it leaves
    /// the JavaScript thread like fetch does — LLP 0059.000 §3.11 has no
    /// synchronous variants on purpose.
    FsReadFile = 110,
    FsWriteFile = 111,
    FsAppendFile = 112,
    FsReadDir = 113,
    FsMkdir = 114,
    FsRemove = 115,
    FsStat = 116,
    FsRename = 117,
    FsCopyFile = 118,
    FsRealpath = 119,
    FsAtomicWriteFile = 120,
    SqliteOpen = 150,
    SqlitePrepare = 151,
    SqliteExecute = 152,
    SqliteQuery = 153,
    SqliteStatementExecute = 154,
    SqliteStatementQuery = 155,
    SqliteTransaction = 156,
    SqliteClose = 157,
    SqliteStatementClose = 158,
}

impl AsyncOp {
    fn from_u32(value: u32) -> Option<Self> {
        match value {
            100 => Some(AsyncOp::Echo),
            101 => Some(AsyncOp::Fetch),
            102 => Some(AsyncOp::ReadBody),
            110 => Some(AsyncOp::FsReadFile),
            111 => Some(AsyncOp::FsWriteFile),
            112 => Some(AsyncOp::FsAppendFile),
            113 => Some(AsyncOp::FsReadDir),
            114 => Some(AsyncOp::FsMkdir),
            115 => Some(AsyncOp::FsRemove),
            116 => Some(AsyncOp::FsStat),
            117 => Some(AsyncOp::FsRename),
            118 => Some(AsyncOp::FsCopyFile),
            119 => Some(AsyncOp::FsRealpath),
            120 => Some(AsyncOp::FsAtomicWriteFile),
            150 => Some(AsyncOp::SqliteOpen),
            151 => Some(AsyncOp::SqlitePrepare),
            152 => Some(AsyncOp::SqliteExecute),
            153 => Some(AsyncOp::SqliteQuery),
            154 => Some(AsyncOp::SqliteStatementExecute),
            155 => Some(AsyncOp::SqliteStatementQuery),
            156 => Some(AsyncOp::SqliteTransaction),
            157 => Some(AsyncOp::SqliteClose),
            158 => Some(AsyncOp::SqliteStatementClose),

            _ => None,
        }
    }
}

fn valid_handle(n: f64) -> Result<u64, HostError> {
    if n.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&n) {
        Ok(n as u64)
    } else {
        Err(HostError::InvalidArgument("invalid handle".into()))
    }
}

fn run_async(
    op: AsyncOp,
    args: &[HostValue],
    state: &crate::task::RuntimeState,
    grants: &GrantSet,
) -> Result<HostValue, HostError> {
    if (150..=158).contains(&(op as u32)) {
        return crate::sqlite_abi::run(op as u32, args, state, grants);
    }
    if let Some(fs_op) = fs_op_for(op) {
        return run_fs(fs_op, args, grants, state.app_directories());
    }
    match op {
        AsyncOp::Fetch => {
            use crate::stdlib::fetch::{is_valid_name, is_valid_value, RedirectMode, Request};
            let url = match args.first() {
                Some(HostValue::Str(url)) => url.clone(),
                _ => return Err(HostError::InvalidArgument("fetch expects a URL".into())),
            };
            let mut request = Request::get(&url);
            if let Some(HostValue::Str(method)) = args.get(1) {
                if !method.is_empty() {
                    request.method = method.to_ascii_uppercase();
                }
            }
            if let Some(HostValue::Bytes(body)) = args.get(2) {
                request.body = Some(body.clone());
            }
            if let Some(HostValue::Str(mode)) = args.get(3) {
                request.redirect = match mode.as_str() {
                    "manual" => RedirectMode::Manual,
                    "error" => RedirectMode::Error,
                    _ => RedirectMode::Follow,
                };
            }
            // @ref LLP 0059.000#35-fetch--delegating-capability-bearing — request headers cross by handle, validated before transport
            match args.get(4) {
                None | Some(HostValue::Undefined) => {}
                Some(HostValue::Number(handle))
                    if handle.fract() == 0.0
                        && (1.0..=9_007_199_254_740_991.0).contains(handle) =>
                {
                    request.headers = state
                        .with_headers(*handle as u64, |headers| {
                            for (name, value) in headers.entries() {
                                if !is_valid_name(name) || !is_valid_value(value) {
                                    return Err(HostError::InvalidArgument(
                                        "fetch headers contain an invalid name or value".into(),
                                    ));
                                }
                            }
                            Ok(headers.clone())
                        })
                        .ok_or_else(|| {
                            HostError::InvalidArgument("unknown fetch headers handle".into())
                        })??;
                }
                _ => {
                    return Err(HostError::InvalidArgument(
                        "fetch expects a headers handle".into(),
                    ));
                }
            }
            let control = match args.get(5) {
                None | Some(HostValue::Undefined) => crate::stdlib::abort::AbortController::new(),
                Some(HostValue::Number(n)) => state
                    .control(valid_handle(*n)?)
                    .ok_or_else(|| HostError::InvalidArgument("unknown fetch control".into()))?,
                _ => {
                    return Err(HostError::InvalidArgument(
                        "fetch control handle expected".into(),
                    ))
                }
            };
            let response = crate::stdlib::fetch::fetch_stream(
                state.transport(),
                grants,
                request,
                &control.signal(),
            )?;
            Ok(HostValue::Number(state.store_response(
                response,
                control,
                match args.get(5) {
                    Some(HostValue::Number(n)) => Some(*n as u64),
                    _ => None,
                },
            ) as f64))
        }
        AsyncOp::ReadBody => match args.first() {
            Some(HostValue::Number(n)) => state.read_response(valid_handle(*n)?),
            _ => Err(HostError::InvalidArgument(
                "response handle expected".into(),
            )),
        },
        AsyncOp::Echo => {
            let text = match args.first() {
                Some(HostValue::Str(text)) => text.clone(),
                _ => return Err(HostError::InvalidArgument("echo expects a string".into())),
            };
            // A deliberate marker: an argument of "fail" rejects, so the
            // rejection path is exercised by the same op.
            if text == "fail" {
                return Err(HostError::Failed("echo was asked to fail".into()));
            }
            Ok(HostValue::Str(text))
        }
        _ => unreachable!("fs ops returned above, via fs_op_for"),
    }
}

/// Resolve a module and produce its executable form.
///
/// Returns 0 on success, writing the resolved specifier into `out_resolved` and
/// the module's bytes into `out_source` — Hermes bytecode when a compiler is
/// configured, wrapped source otherwise. 1 on failure with the message in
/// `out_resolved`. Both are Rust-owned and released with `ibex2_host_release`.
///
/// # Safety
/// All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ibex2_loader_load(
    state: *const crate::task::RuntimeState,
    from: *const c_char,
    specifier: *const c_char,
    out_resolved: *mut AbiValue,
    out_source: *mut AbiValue,
) -> c_int {
    if out_resolved.is_null() || out_source.is_null() {
        return 1;
    }
    *out_resolved = AbiValue::undefined();
    *out_source = AbiValue::undefined();

    let Some(state) = crate::task::clone_queue(state) else {
        return fail(out_resolved, "no runtime state");
    };
    let read = |raw: *const c_char| -> String {
        if raw.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(raw).to_string_lossy().into_owned()
        }
    };

    match state.load_module(&read(from), &read(specifier)) {
        Ok((resolved, bytes)) => {
            *out_resolved = leak_value(HostValue::Str(resolved));
            *out_source = leak_value(HostValue::Bytes(bytes));
            0
        }
        Err(message) => fail(out_resolved, &message),
    }
}

/// The grant set for one module, as an owned pointer.
///
/// # Safety
/// The result must be released with `ibex2_grants_destroy`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_loader_grants_for(
    state: *const crate::task::RuntimeState,
    specifier: *const c_char,
) -> *const GrantSet {
    let Some(state) = crate::task::clone_queue(state) else {
        return std::ptr::null();
    };
    let specifier = if specifier.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(specifier)
            .to_string_lossy()
            .into_owned()
    };
    std::sync::Arc::into_raw(state.grants_for(&specifier))
}

/// Milliseconds until the next timer, or -1 when none is scheduled.
///
/// # Safety
/// `state` must be a live runtime state.
#[no_mangle]
pub unsafe extern "C" fn ibex2_millis_until_next_timer(
    state: *const crate::task::RuntimeState,
) -> f64 {
    let Some(state) = crate::task::clone_queue(state) else {
        return -1.0;
    };
    state.millis_until_next_timer().unwrap_or(-1.0)
}

/// Block until a completion is ready or `timeout_ms` elapses.
///
/// # Safety
/// `state` must be a live runtime state.
#[no_mangle]
pub unsafe extern "C" fn ibex2_wait_for_completion(
    state: *const crate::task::RuntimeState,
    timeout_ms: u64,
) -> c_int {
    let Some(state) = crate::task::clone_queue(state) else {
        return 0;
    };
    i32::from(
        state
            .queue
            .wait(std::time::Duration::from_millis(timeout_ms)),
    )
}

fn fs_op_for(op: AsyncOp) -> Option<crate::stdlib::fs::FsOp> {
    use crate::stdlib::fs::FsOp;
    Some(match op {
        AsyncOp::FsReadFile => FsOp::ReadFile,
        AsyncOp::FsWriteFile => FsOp::WriteFile,
        AsyncOp::FsAppendFile => FsOp::AppendFile,
        AsyncOp::FsReadDir => FsOp::ReadDir,
        AsyncOp::FsMkdir => FsOp::Mkdir,
        AsyncOp::FsRemove => FsOp::Remove,
        AsyncOp::FsStat => FsOp::Stat,
        AsyncOp::FsRename => FsOp::Rename,
        AsyncOp::FsCopyFile => FsOp::CopyFile,
        AsyncOp::FsRealpath => FsOp::Realpath,
        AsyncOp::FsAtomicWriteFile => FsOp::AtomicWriteFile,
        _ => return None,
    })
}

/// Normalize, admit, then act — in that order, always.
///
/// Normalizing after the check would let `/data/../etc/passwd` pass a `/data`
/// grant, which is the whole reason the order is stated rather than implied.
fn run_fs(
    op: crate::stdlib::fs::FsOp,
    args: &[HostValue],
    grants: &GrantSet,
    directories: Option<&crate::stdlib::app_fs::AppDirectories>,
) -> Result<HostValue, HostError> {
    use crate::stdlib::fs::{run, FsResult};

    let path_arg = |index: usize| -> Result<&str, HostError> {
        match args.get(index) {
            Some(HostValue::Str(text)) => Ok(text),
            _ => Err(HostError::InvalidArgument("fs expects a path".into())),
        }
    };

    let path = path_arg(0)?;
    let destination = if op.takes_second_path() {
        Some(path_arg(1)?)
    } else {
        None
    };

    // Data is the second argument for single-path writes.
    let data = match args.get(1) {
        Some(HostValue::Bytes(bytes)) => Some(bytes.as_slice()),
        _ => None,
    };

    if matches!(
        op,
        crate::stdlib::fs::FsOp::WriteFile
            | crate::stdlib::fs::FsOp::AppendFile
            | crate::stdlib::fs::FsOp::AtomicWriteFile
    ) && data.is_none()
    {
        return Err(HostError::InvalidArgument(
            "fs writes require an ArrayBuffer or typed array".into(),
        ));
    }

    Ok(
        match run(grants, directories, op, path, destination, data)? {
            FsResult::Done => HostValue::Undefined,
            FsResult::Bytes(bytes) => HostValue::Bytes(bytes),
            FsResult::Text(text) => HostValue::Str(text),
            // NUL cannot occur in a filename; newlines can. The JSI adapter
            // turns this flat record into an array without a document codec.
            FsResult::Names(names) => HostValue::Str(names.join("\0")),
            FsResult::Stat(stat) => HostValue::Str(format!(
                "{}\t{}\t{}\t{}",
                stat.size,
                u8::from(stat.is_file),
                u8::from(stat.is_directory),
                stat.modified_ms
            )),
        },
    )
}

/// Take at most ONE admitted host task for the engine to run.
///
/// LLP 0058.000.000 §8: one task per drive cycle, from one FIFO carrying both
/// timer deliveries and settlements. `kind` is 0 for none, 1 for a settlement,
/// 2 for a timer; a timer's handle arrives in `task_id`.
///
/// # Safety
/// All out pointers must be valid and writable.
#[no_mangle]
pub unsafe extern "C" fn ibex2_take_task(
    state: *const crate::task::RuntimeState,
    kind: *mut c_int,
    task_id: *mut u64,
    out: *mut AbiValue,
    is_error: *mut c_int,
) -> c_int {
    if kind.is_null() || task_id.is_null() || out.is_null() || is_error.is_null() {
        return 0;
    }
    *kind = 0;
    let Some(state) = crate::task::clone_queue(state) else {
        return 0;
    };
    let Some(task) = state.queue.take() else {
        return 0;
    };
    match task {
        crate::task::HostTask::Timer { handle } => {
            *kind = 2;
            *task_id = handle;
            1
        }
        crate::task::HostTask::Settlement(completion) => {
            *kind = 1;
            *task_id = completion.task_id;
            match completion.result {
                Ok(value) => {
                    *is_error = 0;
                    *out = leak_value(value);
                }
                Err(err) => {
                    *is_error = 1;
                    *out = leak_value(HostValue::Str(err.to_string()));
                }
            }
            1
        }
    }
}

/// Move every timer due now into the host-task FIFO, and report how many.
///
/// # Safety
/// `state` must be a live runtime state.
#[no_mangle]
pub unsafe extern "C" fn ibex2_admit_due_timers(state: *const crate::task::RuntimeState) -> c_int {
    let Some(state) = crate::task::clone_queue(state) else {
        return 0;
    };
    state.admit_due_timers() as c_int
}

/// Claim the driver, so a nested drive records a wakeup instead of starting a
/// second host task inside project JavaScript.
///
/// # Safety
/// `state` must be a live runtime state.
#[no_mangle]
pub unsafe extern "C" fn ibex2_begin_drive(state: *const crate::task::RuntimeState) -> c_int {
    match crate::task::clone_queue(state) {
        Some(state) => c_int::from(state.begin_drive()),
        None => 0,
    }
}

/// # Safety
/// `state` must be a live runtime state.
#[no_mangle]
pub unsafe extern "C" fn ibex2_end_drive(state: *const crate::task::RuntimeState) {
    if let Some(state) = crate::task::clone_queue(state) {
        state.end_drive();
    }
}

/// Release a value produced by `ibex2_host_call`.
///
/// # Safety
/// `value` must be a value this module produced and not yet released.
#[no_mangle]
pub unsafe extern "C" fn ibex2_host_release(value: *mut AbiValue) {
    if value.is_null() {
        return;
    }
    let owned = &mut *value;
    if !owned.data.is_null() && owned.len > 0 {
        // slice_from_raw_parts_mut builds the fat pointer directly; going
        // through slice::from_raw_parts_mut would create a reference to memory
        // we are about to free.
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            owned.data as *mut u8,
            owned.len,
        )));
    }
    *owned = AbiValue::undefined();
}

/// Drain the console into a newline-joined `level:message` block.
///
/// A convenience for the spike's tests; the embedder drains structurally.
///
/// # Safety
/// The returned pointer must be freed with `ibex2_host_free_string`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_console_drain() -> *mut c_char {
    let text = drain_console()
        .into_iter()
        .map(|record| format!("{}:{}", record.level.as_str(), record.message))
        .collect::<Vec<_>>()
        .join("\n");
    match std::ffi::CString::new(text) {
        Ok(text) => text.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `value` must come from `ibex2_console_drain`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_host_free_string(value: *mut c_char) {
    if !value.is_null() {
        drop(std::ffi::CString::from_raw(value));
    }
}

/// How many environment variables this grant set may read.
///
/// `process.env` is built from this list at binding time rather than checked at
/// read time: the snapshot contains exactly the granted variables, so an
/// ungranted one is `undefined` because it is absent, not because a check
/// refused it. Authority carried by the binding (LLP 0060 D1), in its most
/// literal form.
///
/// # Safety
/// `grants` must come from `ibex2_grants_create` and still be alive.
#[no_mangle]
pub unsafe extern "C" fn ibex2_grants_env_count(grants: *const GrantSet) -> usize {
    match grants.as_ref() {
        Some(set) => set.readable_env().len(),
        None => 0,
    }
}

/// The name and current value of granted variable `index`.
///
/// Returns 0 and leaves the outputs null when the variable is not set in the
/// process environment, so `process.env.MISSING` is `undefined` as it is in
/// Node — a grant is permission to read, not a guarantee there is something
/// there.
///
/// # Safety
/// `grants` must be live; both out-pointers must be valid. The returned
/// strings are owned by the caller and freed with `ibex2_string_free`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_grants_env_at(
    grants: *const GrantSet,
    index: usize,
    out_name: *mut *mut std::ffi::c_char,
    out_value: *mut *mut std::ffi::c_char,
) -> std::ffi::c_int {
    *out_name = std::ptr::null_mut();
    *out_value = std::ptr::null_mut();
    let Some(set) = grants.as_ref() else {
        return 0;
    };
    let names = set.readable_env();
    let Some(name) = names.get(index) else {
        return 0;
    };
    let Ok(value) = std::env::var(name) else {
        return 0;
    };
    let (Ok(name_c), Ok(value_c)) = (std::ffi::CString::new(*name), std::ffi::CString::new(value))
    else {
        return 0;
    };
    *out_name = name_c.into_raw();
    *out_value = value_c.into_raw();
    1
}

/// Release a string handed out by `ibex2_grants_env_at`.
///
/// # Safety
/// `value` must come from that function and be freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn ibex2_string_free(value: *mut std::ffi::c_char) {
    if !value.is_null() {
        drop(std::ffi::CString::from_raw(value));
    }
}

#[cfg(test)]
mod fetch_header_tests {
    use super::*;
    use crate::stdlib::fetch::{Headers, Request, Transport};
    use crate::task::RuntimeState;

    struct NoRequests;
    impl Transport for NoRequests {
        fn open(
            &self,
            _: &Request,
            _: &crate::stdlib::abort::AbortSignal,
        ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
            panic!("invalid headers reached the transport");
        }
    }

    #[test]
    fn fetch_validates_the_contents_of_a_registered_header_list() {
        // The engine normally validates on append. Native embedders can
        // populate the same registry, so a known handle is not validation.
        let state = RuntimeState::new(Box::new(NoRequests));
        let grants = GrantSet::parse("net.fetch http://127.0.0.1:80").unwrap();
        let mut failures = Vec::new();
        for (name, value) in [
            ("", "value"),
            ("bad name", "value"),
            ("colon:", "value"),
            ("x", "bad\r\ninjection"),
            ("x", "bad\rvalue"),
            ("x", "bad\nvalue"),
            ("x", "bad\0value"),
            ("x", "\u{100}"),
        ] {
            let mut headers = Headers::new();
            headers.set(name, value);
            let args = [
                HostValue::Str("http://127.0.0.1/".into()),
                HostValue::Undefined,
                HostValue::Undefined,
                HostValue::Undefined,
                HostValue::Number(state.store_headers(headers) as f64),
            ];
            let result = run_async(AsyncOp::Fetch, &args, &state, &grants);
            if !matches!(result, Err(HostError::InvalidArgument(_))) {
                failures.push(format!("{name:?}: {value:?}: {result:?}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
