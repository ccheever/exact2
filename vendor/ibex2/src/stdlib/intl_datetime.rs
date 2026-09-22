//! Rust-owned ECMA-402 DateTimeFormat policy for the Linux Hermes adapter.
//!
//! JavaScript performs the observable `Get`, ToPrimitive, and object-shape
//! work. A normalized option vector arrives here; Rust resolves locale
//! extensions/defaults, owns the formatter registry, and partitions the field
//! positions reported by the same ICU formatter used for `format`.
//!
//! @ref LLP 0057#3-the-boundary — primitives and opaque handles cross the boundary
//! @ref LLP 0057#31-what-goes-in-rust-and-what-does-not — JavaScript owns object shape; Rust owns standard-library policy and state

use crate::boundary::{HostArg, HostError, HostValue};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Arc, Mutex, Weak};

const CREATE: u32 = 130;
const FORMAT: u32 = 131;
const FORMAT_PARTS: u32 = 132;
const PART_TYPE: u32 = 133;
const PART_VALUE: u32 = 134;
const RESOLVED: u32 = 135;
const SUPPORTED_LOCALES: u32 = 136;
const CANONICAL_TIME_ZONE: u32 = 137;

const ERA_FIELD: i32 = 0;
const YEAR_FIELD: i32 = 1;
const MONTH_FIELD: i32 = 2;
const DAY_FIELD: i32 = 3;
const HOUR_FIELD: i32 = 4;
const MINUTE_FIELD: i32 = 5;
const SECOND_FIELD: i32 = 6;
const WEEKDAY_FIELD: i32 = 7;
const DAY_PERIOD_FIELD: i32 = 8;
const TIME_ZONE_FIELD: i32 = 9;
const FRACTIONAL_SECOND_FIELD: i32 = 10;
const RELATED_YEAR_FIELD: i32 = 11;
const YEAR_NAME_FIELD: i32 = 12;

extern "C" {
    // Shared with the NumberFormat ICU bridge.
    fn ibex2_icu_default_locale() -> *mut c_char;
    fn ibex2_icu_best_available_locale(tag: *const c_char) -> *mut c_char;
    fn ibex2_icu_default_numbering_system(locale: *const c_char) -> *mut c_char;
    fn ibex2_icu_numbering_system_supported(name: *const c_char) -> c_int;
    fn ibex2_icu_string_destroy(value: *mut c_char);

    fn ibex2_icu_datetime_default_calendar(locale: *const c_char) -> *mut c_char;
    fn ibex2_icu_datetime_calendar_supported(
        locale: *const c_char,
        calendar: *const c_char,
    ) -> c_int;
    fn ibex2_icu_datetime_default_hour_cycle(locale: *const c_char) -> c_int;
    fn ibex2_icu_datetime_canonical_time_zone(zone: *const c_char) -> *mut c_char;
    fn ibex2_icu_datetime_formatter_create(
        locale: *const c_char,
        time_zone: *const c_char,
        skeleton: *const c_char,
        date_style: i32,
        time_style: i32,
    ) -> *mut c_void;
    fn ibex2_icu_datetime_formatter_destroy(value: *mut c_void);
    fn ibex2_icu_datetime_formatter_pattern(
        value: *const c_void,
        length: *mut usize,
    ) -> *const c_char;
    fn ibex2_icu_datetime_format(formatter: *const c_void, millis: f64) -> *mut c_void;
    fn ibex2_icu_datetime_result_text(value: *const c_void, length: *mut usize) -> *const c_char;
    fn ibex2_icu_datetime_result_field_count(value: *const c_void) -> usize;
    fn ibex2_icu_datetime_result_field(
        value: *const c_void,
        index: usize,
        field: *mut i32,
        begin: *mut usize,
        end: *mut usize,
    ) -> c_int;
    fn ibex2_icu_datetime_result_destroy(value: *mut c_void);
}

struct NativeFormatter(*mut c_void);

// RuntimeState is Send because transport completions are published by workers.
// The formatter itself remains on the runtime owner thread.
unsafe impl Send for NativeFormatter {}

impl Drop for NativeFormatter {
    fn drop(&mut self) {
        unsafe { ibex2_icu_datetime_formatter_destroy(self.0) }
    }
}

#[derive(Clone, Debug)]
struct Part {
    kind: &'static str,
    value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Components {
    weekday: Option<String>,
    era: Option<String>,
    year: Option<String>,
    month: Option<String>,
    day: Option<String>,
    hour: Option<String>,
    minute: Option<String>,
    second: Option<String>,
    time_zone_name: Option<String>,
}

impl Components {
    fn any(&self) -> bool {
        self.values().iter().any(|value| value.is_some())
    }

    fn values(&self) -> [&Option<String>; 9] {
        [
            &self.weekday,
            &self.era,
            &self.year,
            &self.month,
            &self.day,
            &self.hour,
            &self.minute,
            &self.second,
            &self.time_zone_name,
        ]
    }
}

struct DateTimeFormat {
    native: NativeFormatter,
    locale: String,
    calendar: String,
    numbering_system: String,
    time_zone: String,
    hour_cycle: Option<String>,
    components: Components,
    date_style: Option<String>,
    time_style: Option<String>,
    last_parts: Vec<Part>,
}

#[derive(Default)]
pub(crate) struct Registry {
    inner: Mutex<RegistryInner>,
}

#[derive(Default)]
struct RegistryInner {
    next: u64,
    values: HashMap<u64, DateTimeFormat>,
}

impl Registry {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(RegistryInner {
                next: 1,
                values: HashMap::new(),
            }),
        }
    }

    fn insert(&self, value: DateTimeFormat) -> u64 {
        let mut inner = self.inner.lock().expect("DateTimeFormat registry poisoned");
        let handle = inner.next;
        inner.next = inner
            .next
            .checked_add(1)
            .expect("DateTimeFormat handle space exhausted");
        inner.values.insert(handle, value);
        handle
    }

    fn remove(&self, handle: u64) {
        self.inner
            .lock()
            .expect("DateTimeFormat registry poisoned")
            .values
            .remove(&handle);
    }

    fn contains(&self, handle: u64) -> bool {
        self.inner
            .lock()
            .expect("DateTimeFormat registry poisoned")
            .values
            .contains_key(&handle)
    }
}

struct DateTimeOwner {
    state: Weak<crate::task::RuntimeState>,
    handle: u64,
}

/// Attach a Rust formatter's lifetime to an unreachable JSI NativeState.
///
/// # Safety
/// `state` must point to a live RuntimeState and `handle` must name one of its
/// DateTimeFormat entries.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_datetime_owner_create(
    state: *const crate::task::RuntimeState,
    handle: f64,
) -> *mut c_void {
    let Some(state) = crate::task::clone_queue(state) else {
        return std::ptr::null_mut();
    };
    let Ok(handle) = handle_from_number(handle) else {
        return std::ptr::null_mut();
    };
    if !state.intl_datetime.contains(handle) {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(DateTimeOwner {
        state: Arc::downgrade(&state),
        handle,
    }))
    .cast()
}

/// # Safety
/// `owner` is null or an unfreed pointer returned by owner_create.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_datetime_owner_destroy(owner: *mut c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<DateTimeOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.intl_datetime.remove(owner.handle);
    }
}

/// # Safety
/// `owner` is a live pointer returned by owner_create.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_datetime_owner_handle(owner: *const c_void) -> f64 {
    if owner.is_null() {
        return 0.0;
    }
    (*(owner.cast::<DateTimeOwner>())).handle as f64
}

pub(crate) fn dispatch(
    op: u32,
    args: &[HostArg<'_>],
    state: Option<&crate::task::RuntimeState>,
) -> Option<Result<HostValue, HostError>> {
    if !(CREATE..=CANONICAL_TIME_ZONE).contains(&op) {
        return None;
    }
    Some(dispatch_inner(op, args, state))
}

fn dispatch_inner(
    op: u32,
    args: &[HostArg<'_>],
    state: Option<&crate::task::RuntimeState>,
) -> Result<HostValue, HostError> {
    match op {
        SUPPORTED_LOCALES => return supported_locales(args).map(HostValue::Str),
        CANONICAL_TIME_ZONE => return canonical_time_zone(args).map(HostValue::Str),
        _ => {}
    }
    let state = state.ok_or_else(|| HostError::Failed("no runtime state".into()))?;
    if op == CREATE {
        return DateTimeFormat::create(args)
            .map(|value| HostValue::Number(state.intl_datetime.insert(value) as f64));
    }
    let handle = args
        .first()
        .and_then(number_arg)
        .ok_or_else(|| HostError::InvalidArgument("DateTimeFormat owner expected".into()))
        .and_then(handle_from_number)?;
    let mut registry = state
        .intl_datetime
        .inner
        .lock()
        .expect("DateTimeFormat registry poisoned");
    let formatter = registry
        .values
        .get_mut(&handle)
        .ok_or_else(|| type_error_value("incompatible DateTimeFormat receiver"))?;
    match op {
        FORMAT => formatter.format(args.get(1)).map(HostValue::Str),
        FORMAT_PARTS => {
            formatter.last_parts = formatter.format_parts(args.get(1))?;
            Ok(HostValue::Number(formatter.last_parts.len() as f64))
        }
        PART_TYPE | PART_VALUE => {
            let index = args
                .get(1)
                .and_then(number_arg)
                .filter(|value| value.is_finite() && *value >= 0.0 && value.fract() == 0.0)
                .map(|value| value as usize)
                .ok_or_else(|| {
                    HostError::InvalidArgument("DateTimeFormat part index expected".into())
                })?;
            let part = formatter.last_parts.get(index).ok_or_else(|| {
                HostError::InvalidArgument("DateTimeFormat part index out of range".into())
            })?;
            Ok(HostValue::Str(if op == PART_TYPE {
                part.kind.to_string()
            } else {
                part.value.clone()
            }))
        }
        RESOLVED => formatter.resolved(args.get(1)),
        _ => unreachable!(),
    }
}

impl DateTimeFormat {
    fn create(args: &[HostArg<'_>]) -> Result<Self, HostError> {
        if args.len() != 19 {
            return Err(HostError::InvalidArgument(
                "Intl.DateTimeFormat normalized option vector is incomplete".into(),
            ));
        }
        let requested = required_str(args, 0, "locale list")?;
        one_of(
            required_str(args, 1, "locale matcher")?,
            &["lookup", "best fit"],
            "localeMatcher",
        )?;
        let calendar_option = optional_str(args, 2)?.map(str::to_ascii_lowercase);
        let numbering_option = optional_str(args, 3)?.map(str::to_ascii_lowercase);
        let hour12 = optional_bool(args, 4)?;
        let hour_cycle_option = optional_str(args, 5)?;
        let time_zone = required_str(args, 6, "timeZone")?.to_string();
        let requested_components = Components {
            weekday: component(args, 7, &["narrow", "short", "long"], "weekday")?,
            era: component(args, 8, &["narrow", "short", "long"], "era")?,
            year: component(args, 9, &["2-digit", "numeric"], "year")?,
            month: component(
                args,
                10,
                &["2-digit", "numeric", "narrow", "short", "long"],
                "month",
            )?,
            day: component(args, 11, &["2-digit", "numeric"], "day")?,
            hour: component(args, 12, &["2-digit", "numeric"], "hour")?,
            minute: component(args, 13, &["2-digit", "numeric"], "minute")?,
            second: component(args, 14, &["2-digit", "numeric"], "second")?,
            time_zone_name: component(args, 15, &["short", "long"], "timeZoneName")?,
        };
        one_of(
            required_str(args, 16, "format matcher")?,
            &["basic", "best fit"],
            "formatMatcher",
        )?;
        let date_style = style_option(args, 17, "dateStyle")?;
        let time_style = style_option(args, 18, "timeStyle")?;
        if (date_style.is_some() || time_style.is_some()) && requested_components.any() {
            return type_error("dateStyle and timeStyle may not be used with explicit components");
        }

        let (data_locale, extensions) = resolve_data_locale(requested)?;
        let default_calendar = default_calendar(&data_locale)?;
        let extension_calendar = extensions
            .calendar
            .as_deref()
            .filter(|value| calendar_supported(&data_locale, value));
        let option_calendar = calendar_option
            .as_deref()
            .filter(|value| calendar_supported(&data_locale, value));
        let calendar = option_calendar
            .or(extension_calendar)
            .unwrap_or(&default_calendar)
            .to_string();

        let default_numbering = default_numbering_system(&data_locale)?;
        let extension_numbering = extensions
            .numbering_system
            .as_deref()
            .filter(|value| numbering_system_supported(value));
        let option_numbering = numbering_option
            .as_deref()
            .filter(|value| numbering_system_supported(value));
        let numbering_system = option_numbering
            .or(extension_numbering)
            .unwrap_or(&default_numbering)
            .to_string();

        let default_hour_cycle = default_hour_cycle(&data_locale)?;
        let extension_hour_cycle = extensions
            .hour_cycle
            .as_deref()
            .filter(|value| valid_hour_cycle(value));
        let locale_hour_cycle = if hour12.is_some() {
            extension_hour_cycle.unwrap_or(default_hour_cycle.as_str())
        } else {
            hour_cycle_option
                .filter(|value| valid_hour_cycle(value))
                .or(extension_hour_cycle)
                .unwrap_or(default_hour_cycle.as_str())
        };
        let effective_hour_cycle = match hour12 {
            Some(true) if matches!(default_hour_cycle.as_str(), "h11" | "h23") => "h11",
            Some(true) => "h12",
            Some(false) if matches!(default_hour_cycle.as_str(), "h11" | "h23") => "h23",
            Some(false) => "h24",
            None => locale_hour_cycle,
        };

        let locale = resolved_locale(
            &data_locale,
            &extensions,
            hour12,
            hour_cycle_option,
            &calendar,
            &numbering_system,
            locale_hour_cycle,
        );
        let effective_locale = locale_with_extensions(
            &data_locale,
            [
                ("ca", calendar.as_str()),
                ("hc", effective_hour_cycle),
                ("nu", numbering_system.as_str()),
            ],
        );
        let skeleton = make_skeleton(&requested_components, effective_hour_cycle);
        let native = native_formatter(
            &effective_locale,
            &time_zone,
            &skeleton,
            date_style,
            time_style,
        )?;
        let pattern = native_pattern(&native)?;
        let pattern_components = components_from_pattern(&pattern);
        let style_mode = date_style.is_some() || time_style.is_some();
        let hour_cycle = if pattern_components.hour.is_some() || time_style.is_some() {
            hour_cycle_from_pattern(&pattern).map(str::to_string)
        } else {
            None
        };

        Ok(Self {
            native,
            locale,
            calendar,
            numbering_system,
            time_zone,
            hour_cycle,
            components: if style_mode {
                Components::default()
            } else {
                pattern_components
            },
            date_style: date_style.map(str::to_string),
            time_style: time_style.map(str::to_string),
            last_parts: Vec::new(),
        })
    }

    fn format(&self, value: Option<&HostArg<'_>>) -> Result<String, HostError> {
        native_result(&self.native, checked_time(value)?).map(|result| result.text)
    }

    fn format_parts(&self, value: Option<&HostArg<'_>>) -> Result<Vec<Part>, HostError> {
        let result = native_result(&self.native, checked_time(value)?)?;
        Ok(partition(&result))
    }

    fn resolved(&self, field: Option<&HostArg<'_>>) -> Result<HostValue, HostError> {
        let field = field
            .and_then(number_arg)
            .filter(|value| value.is_finite() && value.fract() == 0.0)
            .map(|value| value as u8)
            .ok_or_else(|| HostError::InvalidArgument("resolved option field expected".into()))?;
        let string = |value: &str| Ok(HostValue::Str(value.to_string()));
        match field {
            0 => string(&self.locale),
            1 => string(&self.calendar),
            2 => string(&self.numbering_system),
            3 => string(&self.time_zone),
            4 => option_value(self.hour_cycle.as_deref()),
            5 => Ok(self
                .hour_cycle
                .as_deref()
                .map(|value| HostValue::Bool(matches!(value, "h11" | "h12")))
                .unwrap_or(HostValue::Undefined)),
            6 => option_value(self.components.weekday.as_deref()),
            7 => option_value(self.components.era.as_deref()),
            8 => option_value(self.components.year.as_deref()),
            9 => option_value(self.components.month.as_deref()),
            10 => option_value(self.components.day.as_deref()),
            11 => option_value(self.components.hour.as_deref()),
            12 => option_value(self.components.minute.as_deref()),
            13 => option_value(self.components.second.as_deref()),
            14 => option_value(self.components.time_zone_name.as_deref()),
            15 => option_value(self.date_style.as_deref()),
            16 => option_value(self.time_style.as_deref()),
            _ => Err(HostError::InvalidArgument(
                "unknown DateTimeFormat resolved option field".into(),
            )),
        }
    }
}

struct NativeResult {
    text: String,
    spans: Vec<(i32, usize, usize)>,
}

fn checked_time(value: Option<&HostArg<'_>>) -> Result<f64, HostError> {
    let value = value
        .and_then(number_arg)
        .ok_or_else(|| HostError::InvalidArgument("date time value expected".into()))?;
    if value.is_finite() && value.abs() <= 8_640_000_000_000_000.0 {
        Ok(value.trunc())
    } else {
        range("Invalid time value")
    }
}

fn native_result(formatter: &NativeFormatter, millis: f64) -> Result<NativeResult, HostError> {
    let raw = unsafe { ibex2_icu_datetime_format(formatter.0, millis) };
    if raw.is_null() {
        return Err(HostError::Failed("ICU could not format the date".into()));
    }
    struct Guard(*mut c_void);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { ibex2_icu_datetime_result_destroy(self.0) }
        }
    }
    let guard = Guard(raw);
    let mut length = 0;
    let text = unsafe { ibex2_icu_datetime_result_text(guard.0, &mut length) };
    if text.is_null() {
        return Err(HostError::Failed("ICU returned no formatted date".into()));
    }
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), length) };
    let text = std::str::from_utf8(bytes)
        .map_err(|_| HostError::Failed("ICU returned invalid UTF-8".into()))?
        .to_string();
    let count = unsafe { ibex2_icu_datetime_result_field_count(guard.0) };
    let mut spans = Vec::with_capacity(count);
    for index in 0..count {
        let (mut field, mut begin, mut end) = (0, 0, 0);
        if unsafe {
            ibex2_icu_datetime_result_field(guard.0, index, &mut field, &mut begin, &mut end)
        } != 0
            && begin <= end
            && end <= text.len()
            && text.is_char_boundary(begin)
            && text.is_char_boundary(end)
        {
            spans.push((field, begin, end));
        }
    }
    Ok(NativeResult { text, spans })
}

fn partition(result: &NativeResult) -> Vec<Part> {
    let mut boundaries = vec![0, result.text.len()];
    for &(_, begin, end) in &result.spans {
        boundaries.push(begin);
        boundaries.push(end);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut parts: Vec<Part> = Vec::new();
    for range in boundaries.windows(2) {
        let (begin, end) = (range[0], range[1]);
        if begin == end {
            continue;
        }
        let field = result
            .spans
            .iter()
            .filter(|(_, start, limit)| *start <= begin && end <= *limit)
            .min_by_key(|(_, start, limit)| limit - start)
            .map(|(field, _, _)| *field);
        let kind = part_kind(field);
        let value = &result.text[begin..end];
        if let Some(last) = parts.last_mut().filter(|part| part.kind == kind) {
            last.value.push_str(value);
        } else {
            parts.push(Part {
                kind,
                value: value.to_string(),
            });
        }
    }
    parts
}

fn part_kind(field: Option<i32>) -> &'static str {
    match field {
        Some(ERA_FIELD) => "era",
        Some(YEAR_FIELD) => "year",
        Some(MONTH_FIELD) => "month",
        Some(DAY_FIELD) => "day",
        Some(HOUR_FIELD) => "hour",
        Some(MINUTE_FIELD) => "minute",
        Some(SECOND_FIELD) => "second",
        Some(WEEKDAY_FIELD) => "weekday",
        Some(DAY_PERIOD_FIELD) => "dayPeriod",
        Some(TIME_ZONE_FIELD) => "timeZoneName",
        Some(FRACTIONAL_SECOND_FIELD) => "fractionalSecond",
        Some(RELATED_YEAR_FIELD) => "relatedYear",
        Some(YEAR_NAME_FIELD) => "yearName",
        _ => "literal",
    }
}

#[derive(Default)]
struct LocaleExtensions {
    calendar: Option<String>,
    numbering_system: Option<String>,
    hour_cycle: Option<String>,
}

fn resolve_data_locale(requested: &str) -> Result<(String, LocaleExtensions), HostError> {
    for locale in locale_list(requested) {
        let (base, extensions) = locale_base_and_extensions(locale);
        if let Some(available) = best_available_locale(&base)? {
            return Ok((available, extensions));
        }
    }
    let default = owned_icu_string(unsafe { ibex2_icu_default_locale() })
        .ok_or_else(|| HostError::Failed("ICU has no default locale".into()))?;
    let (base, _) = locale_base_and_extensions(&default);
    let available = best_available_locale(&base)?
        .ok_or_else(|| HostError::Failed("ICU default locale is unavailable".into()))?;
    Ok((available, LocaleExtensions::default()))
}

fn locale_base_and_extensions(locale: &str) -> (String, LocaleExtensions) {
    let parts: Vec<&str> = locale.split('-').collect();
    let first_extension = parts
        .iter()
        .position(|part| part.len() == 1)
        .unwrap_or(parts.len());
    let base = parts[..first_extension].join("-");
    let mut result = LocaleExtensions::default();
    let private_use = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("x"))
        .unwrap_or(parts.len());
    let Some(unicode) = parts[..private_use]
        .iter()
        .position(|part| part.eq_ignore_ascii_case("u"))
    else {
        return (base, result);
    };
    let mut index = unicode + 1;
    while index < parts.len() && parts[index].len() != 1 {
        if parts[index].len() != 2 {
            index += 1;
            continue;
        }
        let key = parts[index].to_ascii_lowercase();
        index += 1;
        let start = index;
        while index < parts.len() && parts[index].len() > 2 {
            index += 1;
        }
        if start == index {
            continue;
        }
        let value = parts[start..index].join("-").to_ascii_lowercase();
        match key.as_str() {
            "ca" => result.calendar = Some(value),
            "nu" => result.numbering_system = Some(value),
            "hc" => result.hour_cycle = Some(value),
            _ => {}
        }
    }
    (base, result)
}

#[allow(clippy::too_many_arguments)]
fn resolved_locale(
    base: &str,
    extensions: &LocaleExtensions,
    hour12: Option<bool>,
    hour_cycle_option: Option<&str>,
    calendar: &str,
    numbering: &str,
    locale_hour_cycle: &str,
) -> String {
    let mut kept = Vec::new();
    // ResolveLocale retains a requested keyword when an unsupported option
    // leaves that requested value selected. Comparing the selected value is
    // sufficient; consulting the raw option would incorrectly strip it.
    if extensions.calendar.as_deref() == Some(calendar) {
        kept.push(("ca", calendar));
    }
    if extensions.hour_cycle.as_deref() == Some(locale_hour_cycle)
        && hour12.is_none()
        && hour_cycle_option.is_none_or(|value| value == locale_hour_cycle)
    {
        kept.push(("hc", locale_hour_cycle));
    }
    if extensions.numbering_system.as_deref() == Some(numbering) {
        kept.push(("nu", numbering));
    }
    locale_with_extensions(base, kept)
}

fn locale_with_extensions<'a>(
    base: &str,
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let values: Vec<_> = values.into_iter().collect();
    if values.is_empty() {
        return base.to_string();
    }
    let mut result = format!("{base}-u");
    for (key, value) in values {
        result.push('-');
        result.push_str(key);
        result.push('-');
        result.push_str(value);
    }
    result
}

fn make_skeleton(components: &Components, hour_cycle: &str) -> String {
    let mut result = String::new();
    push_width(
        &mut result,
        components.weekday.as_deref(),
        "EEEEE",
        "EEE",
        "EEEE",
    );
    push_width(
        &mut result,
        components.era.as_deref(),
        "GGGGG",
        "GGG",
        "GGGG",
    );
    push_numeric(&mut result, components.year.as_deref(), 'y');
    match components.month.as_deref() {
        Some("2-digit") => result.push_str("MM"),
        Some("numeric") => result.push('M'),
        Some("narrow") => result.push_str("MMMMM"),
        Some("short") => result.push_str("MMM"),
        Some("long") => result.push_str("MMMM"),
        _ => {}
    }
    push_numeric(&mut result, components.day.as_deref(), 'd');
    if let Some(width) = components.hour.as_deref() {
        let symbol = match hour_cycle {
            "h11" => 'K',
            "h12" => 'h',
            "h24" => 'k',
            _ => 'H',
        };
        result.extend(std::iter::repeat_n(
            symbol,
            usize::from(width == "2-digit") + 1,
        ));
    }
    push_numeric(&mut result, components.minute.as_deref(), 'm');
    push_numeric(&mut result, components.second.as_deref(), 's');
    match components.time_zone_name.as_deref() {
        Some("short") => result.push('z'),
        Some("long") => result.push_str("zzzz"),
        _ => {}
    }
    result
}

fn push_width(out: &mut String, value: Option<&str>, narrow: &str, short: &str, long: &str) {
    out.push_str(match value {
        Some("narrow") => narrow,
        Some("short") => short,
        Some("long") => long,
        _ => "",
    });
}

fn push_numeric(out: &mut String, value: Option<&str>, symbol: char) {
    if let Some(value) = value {
        out.push(symbol);
        if value == "2-digit" {
            out.push(symbol);
        }
    }
}

fn components_from_pattern(pattern: &str) -> Components {
    let mut result = Components::default();
    for (symbol, width) in pattern_runs(pattern) {
        let value = match symbol {
            'E' | 'e' | 'c' => Some(if width >= 5 {
                "narrow"
            } else if width == 4 {
                "long"
            } else {
                "short"
            }),
            'G' => Some(if width >= 5 {
                "narrow"
            } else if width == 4 {
                "long"
            } else {
                "short"
            }),
            'M' | 'L' => Some(if width >= 5 {
                "narrow"
            } else if width == 4 {
                "long"
            } else if width == 3 {
                "short"
            } else if width == 2 {
                "2-digit"
            } else {
                "numeric"
            }),
            'y' | 'Y' | 'u' | 'U' | 'r' | 'd' | 'h' | 'H' | 'K' | 'k' | 'm' | 's' => {
                Some(if width == 2 { "2-digit" } else { "numeric" })
            }
            'z' | 'v' | 'V' | 'O' | 'X' | 'x' | 'Z' => {
                Some(if width >= 4 { "long" } else { "short" })
            }
            _ => None,
        };
        let Some(value) = value else { continue };
        let target = match symbol {
            'E' | 'e' | 'c' => &mut result.weekday,
            'G' => &mut result.era,
            'y' | 'Y' | 'u' | 'U' | 'r' => &mut result.year,
            'M' | 'L' => &mut result.month,
            'd' => &mut result.day,
            'h' | 'H' | 'K' | 'k' => &mut result.hour,
            'm' => &mut result.minute,
            's' => &mut result.second,
            _ => &mut result.time_zone_name,
        };
        if target.is_none() {
            *target = Some(value.to_string());
        }
    }
    result
}

fn pattern_runs(pattern: &str) -> Vec<(char, usize)> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut runs = Vec::new();
    let (mut index, mut quoted) = (0, false);
    while index < chars.len() {
        if chars[index] == '\'' {
            if index + 1 < chars.len() && chars[index + 1] == '\'' {
                index += 2;
            } else {
                quoted = !quoted;
                index += 1;
            }
            continue;
        }
        if quoted || !chars[index].is_ascii_alphabetic() {
            index += 1;
            continue;
        }
        let symbol = chars[index];
        let start = index;
        while index < chars.len() && chars[index] == symbol {
            index += 1;
        }
        runs.push((symbol, index - start));
    }
    runs
}

fn hour_cycle_from_pattern(pattern: &str) -> Option<&'static str> {
    pattern_runs(pattern)
        .into_iter()
        .find_map(|(symbol, _)| match symbol {
            'K' => Some("h11"),
            'h' => Some("h12"),
            'H' => Some("h23"),
            'k' => Some("h24"),
            _ => None,
        })
}

fn supported_locales(args: &[HostArg<'_>]) -> Result<String, HostError> {
    let requested = required_str(args, 0, "locale list")?;
    one_of(
        required_str(args, 1, "locale matcher")?,
        &["lookup", "best fit"],
        "localeMatcher",
    )?;
    let mut result = Vec::new();
    for locale in locale_list(requested) {
        let (base, _) = locale_base_and_extensions(locale);
        if best_available_locale(&base)?.is_some() {
            result.push(locale);
        }
    }
    Ok(result.join("\n"))
}

fn canonical_time_zone(args: &[HostArg<'_>]) -> Result<String, HostError> {
    let explicit = optional_str(args, 0)?;
    let owned = explicit
        .map(|value| CString::new(value).map_err(|_| range_value("Invalid time zone")))
        .transpose()?;
    let raw = unsafe {
        ibex2_icu_datetime_canonical_time_zone(
            owned
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
        )
    };
    owned_icu_string(raw).ok_or_else(|| {
        if explicit.is_some() {
            range_value("Invalid time zone")
        } else {
            HostError::Failed("ICU has no default time zone".into())
        }
    })
}

fn default_calendar(locale: &str) -> Result<String, HostError> {
    let locale = CString::new(locale).expect("canonical locale contains no NUL");
    owned_icu_string(unsafe { ibex2_icu_datetime_default_calendar(locale.as_ptr()) })
        .ok_or_else(|| HostError::Failed("ICU has no default calendar".into()))
}

fn calendar_supported(locale: &str, calendar: &str) -> bool {
    let Ok(locale) = CString::new(locale) else {
        return false;
    };
    let Ok(calendar) = CString::new(calendar) else {
        return false;
    };
    unsafe { ibex2_icu_datetime_calendar_supported(locale.as_ptr(), calendar.as_ptr()) != 0 }
}

fn default_hour_cycle(locale: &str) -> Result<String, HostError> {
    let locale = CString::new(locale).expect("canonical locale contains no NUL");
    match unsafe { ibex2_icu_datetime_default_hour_cycle(locale.as_ptr()) } {
        11 => Ok("h11".into()),
        12 => Ok("h12".into()),
        23 => Ok("h23".into()),
        24 => Ok("h24".into()),
        _ => Err(HostError::Failed("ICU has no default hour cycle".into())),
    }
}

fn default_numbering_system(locale: &str) -> Result<String, HostError> {
    let locale = CString::new(locale).expect("canonical locale contains no NUL");
    owned_icu_string(unsafe { ibex2_icu_default_numbering_system(locale.as_ptr()) })
        .ok_or_else(|| HostError::Failed("ICU has no default numbering system".into()))
}

fn numbering_system_supported(name: &str) -> bool {
    CString::new(name)
        .ok()
        .is_some_and(|name| unsafe { ibex2_icu_numbering_system_supported(name.as_ptr()) != 0 })
}

fn native_formatter(
    locale: &str,
    time_zone: &str,
    skeleton: &str,
    date_style: Option<&str>,
    time_style: Option<&str>,
) -> Result<NativeFormatter, HostError> {
    let locale = CString::new(locale).expect("locale contains no NUL");
    let time_zone = CString::new(time_zone).expect("time zone contains no NUL");
    let skeleton = CString::new(skeleton).expect("skeleton contains no NUL");
    let native = unsafe {
        ibex2_icu_datetime_formatter_create(
            locale.as_ptr(),
            time_zone.as_ptr(),
            skeleton.as_ptr(),
            style_number(date_style),
            style_number(time_style),
        )
    };
    if native.is_null() {
        Err(HostError::Failed(
            "ICU rejected the DateTimeFormat configuration".into(),
        ))
    } else {
        Ok(NativeFormatter(native))
    }
}

fn native_pattern(formatter: &NativeFormatter) -> Result<String, HostError> {
    let mut length = 0;
    let raw = unsafe { ibex2_icu_datetime_formatter_pattern(formatter.0, &mut length) };
    if raw.is_null() {
        return Err(HostError::Failed("ICU returned no date pattern".into()));
    }
    let bytes = unsafe { std::slice::from_raw_parts(raw.cast::<u8>(), length) };
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|_| HostError::Failed("ICU returned an invalid date pattern".into()))
}

fn style_number(value: Option<&str>) -> i32 {
    match value {
        Some("full") => 0,
        Some("long") => 1,
        Some("medium") => 2,
        Some("short") => 3,
        _ => -1,
    }
}

fn best_available_locale(tag: &str) -> Result<Option<String>, HostError> {
    let tag =
        CString::new(tag).map_err(|_| HostError::InvalidArgument("locale contains NUL".into()))?;
    Ok(owned_icu_string(unsafe {
        ibex2_icu_best_available_locale(tag.as_ptr())
    }))
}

fn owned_icu_string(value: *mut c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let result = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned();
    unsafe { ibex2_icu_string_destroy(value) };
    Some(result)
}

fn locale_list(value: &str) -> impl Iterator<Item = &str> {
    value.split('\n').filter(|item| !item.is_empty())
}

fn required_str<'a>(
    args: &'a [HostArg<'a>],
    index: usize,
    name: &str,
) -> Result<&'a str, HostError> {
    optional_str(args, index)?
        .ok_or_else(|| HostError::InvalidArgument(format!("{name} must be a string")))
}

fn optional_str<'a>(args: &'a [HostArg<'a>], index: usize) -> Result<Option<&'a str>, HostError> {
    match args.get(index) {
        Some(HostArg::Undefined) | None => Ok(None),
        Some(HostArg::Str(value)) => Ok(Some(value)),
        _ => Err(HostError::InvalidArgument(format!(
            "argument {index} must be a string or undefined"
        ))),
    }
}

fn optional_bool(args: &[HostArg<'_>], index: usize) -> Result<Option<bool>, HostError> {
    match args.get(index) {
        Some(HostArg::Undefined) | None => Ok(None),
        Some(HostArg::Bool(value)) => Ok(Some(*value)),
        _ => Err(HostError::InvalidArgument(format!(
            "argument {index} must be a boolean or undefined"
        ))),
    }
}

fn number_arg(value: &HostArg<'_>) -> Option<f64> {
    if let HostArg::Number(value) = value {
        Some(*value)
    } else {
        None
    }
}

fn component(
    args: &[HostArg<'_>],
    index: usize,
    allowed: &[&str],
    name: &str,
) -> Result<Option<String>, HostError> {
    optional_str(args, index)?
        .map(|value| one_of(value, allowed, name).map(str::to_string))
        .transpose()
}

fn style_option<'a>(
    args: &'a [HostArg<'a>],
    index: usize,
    name: &str,
) -> Result<Option<&'a str>, HostError> {
    optional_str(args, index)?
        .map(|value| one_of(value, &["full", "long", "medium", "short"], name))
        .transpose()
}

fn handle_from_number(value: f64) -> Result<u64, HostError> {
    if value.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&value) {
        Ok(value as u64)
    } else {
        Err(HostError::InvalidArgument(
            "invalid DateTimeFormat handle".into(),
        ))
    }
}

fn one_of<'a>(value: &'a str, allowed: &[&str], name: &str) -> Result<&'a str, HostError> {
    if allowed.contains(&value) {
        Ok(value)
    } else {
        range(&format!("{name} is not a supported value"))
    }
}

fn valid_hour_cycle(value: &str) -> bool {
    matches!(value, "h11" | "h12" | "h23" | "h24")
}

fn option_value(value: Option<&str>) -> Result<HostValue, HostError> {
    Ok(value
        .map(|value| HostValue::Str(value.to_string()))
        .unwrap_or(HostValue::Undefined))
}

fn range<T>(message: &str) -> Result<T, HostError> {
    Err(range_value(message))
}

fn range_value(message: &str) -> HostError {
    HostError::Failed(format!("RangeError: {message}"))
}

fn type_error<T>(message: &str) -> Result<T, HostError> {
    Err(type_error_value(message))
}

fn type_error_value(message: &str) -> HostError {
    HostError::Failed(format!("TypeError: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_relevant_unicode_extensions_and_canonical_order() {
        let (base, extensions) =
            locale_base_and_extensions("de-DE-u-nu-arab-co-phonebk-ca-buddhist-hc-h23");
        assert_eq!(base, "de-DE");
        assert_eq!(extensions.calendar.as_deref(), Some("buddhist"));
        assert_eq!(extensions.numbering_system.as_deref(), Some("arab"));
        assert_eq!(extensions.hour_cycle.as_deref(), Some("h23"));
        assert_eq!(
            locale_with_extensions(&base, [("ca", "buddhist"), ("hc", "h23"), ("nu", "arab")]),
            "de-DE-u-ca-buddhist-hc-h23-nu-arab"
        );
    }

    #[test]
    fn private_use_subtags_are_not_unicode_extensions() {
        let (base, extensions) = locale_base_and_extensions("en-x-u-ca-buddhist-nu-arab");
        assert_eq!(base, "en");
        assert!(extensions.calendar.is_none());
        assert!(extensions.numbering_system.is_none());
        assert!(extensions.hour_cycle.is_none());

        let (_, extensions) = locale_base_and_extensions("en-u-ca-buddhist-nu-arab-x-u-hc-h24");
        assert_eq!(extensions.calendar.as_deref(), Some("buddhist"));
        assert_eq!(extensions.numbering_system.as_deref(), Some("arab"));
        assert!(extensions.hour_cycle.is_none());
    }

    #[test]
    fn skeleton_preserves_requested_widths_and_hour_cycle() {
        let components = Components {
            weekday: Some("long".into()),
            year: Some("numeric".into()),
            month: Some("2-digit".into()),
            day: Some("2-digit".into()),
            hour: Some("2-digit".into()),
            minute: Some("2-digit".into()),
            time_zone_name: Some("long".into()),
            ..Components::default()
        };
        assert_eq!(make_skeleton(&components, "h23"), "EEEEyMMddHHmmzzzz");
    }

    #[test]
    fn quoted_pattern_text_does_not_become_a_component() {
        let components = components_from_pattern("d 'de' MMMM 'at' HH:mm");
        assert_eq!(components.day.as_deref(), Some("numeric"));
        assert_eq!(components.month.as_deref(), Some("long"));
        assert_eq!(components.hour.as_deref(), Some("2-digit"));
        assert_eq!(components.minute.as_deref(), Some("2-digit"));
        assert!(components.era.is_none());
        assert_eq!(hour_cycle_from_pattern("d 'h' HH:mm"), Some("h23"));
    }

    #[test]
    fn time_clip_range_is_exact() {
        assert!(checked_time(Some(&HostArg::Number(8_640_000_000_000_000.0))).is_ok());
        assert!(checked_time(Some(&HostArg::Number(8_640_000_000_000_001.0))).is_err());
        assert!(checked_time(Some(&HostArg::Number(f64::NAN))).is_err());
        assert_eq!(
            checked_time(Some(&HostArg::Number(-0.1)))
                .expect("negative fraction")
                .to_bits(),
            (-0.0_f64).to_bits()
        );
        assert_eq!(
            checked_time(Some(&HostArg::Number(999.9))).expect("positive fraction"),
            999.0
        );
    }
}
