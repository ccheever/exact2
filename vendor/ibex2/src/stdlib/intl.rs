//! Rust-owned ECMA-402 NumberFormat semantics for the Linux Hermes adapter.
//!
//! JavaScript performs only observable `Get`/coercion order and object
//! modelling. Normalized options arrive here, where locale resolution,
//! defaults, ICU configuration, formatting, parts, and native lifetime live.
//! ICU is an in-process computation backend, never a second policy owner.
//!
//! @ref LLP 0057#3-the-boundary — Rust owns standard-library semantics

use crate::boundary::{HostArg, HostError, HostValue};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Arc, Mutex, Weak};

const CREATE: u32 = 90;
const FORMAT: u32 = 91;
const FORMAT_PARTS: u32 = 92;
const PART_TYPE: u32 = 93;
const PART_VALUE: u32 = 94;
const RESOLVED: u32 = 95;
const SUPPORTED_LOCALES: u32 = 96;
const CURRENCY_DIGITS: u32 = 98;

const INTEGER_FIELD: i32 = 0;
const FRACTION_FIELD: i32 = 1;
const DECIMAL_FIELD: i32 = 2;
const EXPONENT_SYMBOL_FIELD: i32 = 3;
const EXPONENT_SIGN_FIELD: i32 = 4;
const EXPONENT_FIELD: i32 = 5;
const GROUPING_FIELD: i32 = 6;
const CURRENCY_FIELD: i32 = 7;
const PERCENT_FIELD: i32 = 8;
const PERMILL_FIELD: i32 = 9;
const SIGN_FIELD: i32 = 10;
const UNIT_FIELD: i32 = 11;
const COMPACT_FIELD: i32 = 12;

extern "C" {
    fn ibex2_icu_default_locale() -> *mut c_char;
    fn ibex2_icu_best_available_locale(tag: *const c_char) -> *mut c_char;
    fn ibex2_icu_default_numbering_system(locale: *const c_char) -> *mut c_char;
    fn ibex2_icu_numbering_system_supported(name: *const c_char) -> c_int;
    fn ibex2_icu_currency_digits(currency: *const c_char) -> i32;
    fn ibex2_icu_number_formatter_create(
        locale: *const c_char,
        skeleton: *const c_char,
    ) -> *mut c_void;
    fn ibex2_icu_number_formatter_destroy(value: *mut c_void);
    fn ibex2_icu_number_format_double(formatter: *const c_void, value: f64) -> *mut c_void;
    fn ibex2_icu_number_format_decimal(
        formatter: *const c_void,
        value: *const c_char,
    ) -> *mut c_void;
    fn ibex2_icu_number_result_text(value: *const c_void, length: *mut usize) -> *const c_char;
    fn ibex2_icu_number_result_field_count(value: *const c_void) -> usize;
    fn ibex2_icu_number_result_field(
        value: *const c_void,
        index: usize,
        field: *mut i32,
        begin: *mut usize,
        end: *mut usize,
    ) -> c_int;
    fn ibex2_icu_number_result_destroy(value: *mut c_void);
    fn ibex2_icu_string_destroy(value: *mut c_char);
}

struct NativeFormatter(*mut c_void);

// ICU documents UNumberFormatter as immutable and thread-safe. Ibex still
// calls it only from its runtime owner thread; Send is needed solely because
// RuntimeState also contains worker-safe transport state behind an Arc.
unsafe impl Send for NativeFormatter {}

impl Drop for NativeFormatter {
    fn drop(&mut self) {
        unsafe { ibex2_icu_number_formatter_destroy(self.0) }
    }
}

#[derive(Clone, Debug)]
struct Part {
    kind: &'static str,
    value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Decimal,
    Percent,
    Currency,
    Unit,
}

impl Style {
    fn parse(value: &str) -> Result<Self, HostError> {
        match value {
            "decimal" => Ok(Self::Decimal),
            "percent" => Ok(Self::Percent),
            "currency" => Ok(Self::Currency),
            "unit" => Ok(Self::Unit),
            _ => range("style is not a supported value"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Decimal => "decimal",
            Self::Percent => "percent",
            Self::Currency => "currency",
            Self::Unit => "unit",
        }
    }
}

#[derive(Clone, Debug)]
enum Precision {
    Fraction { minimum: u8, maximum: u8 },
    Significant { minimum: u8, maximum: u8 },
    Compact,
}

struct NumberFormat {
    native: NativeFormatter,
    locale: String,
    numbering_system: String,
    style: Style,
    currency: Option<String>,
    currency_display: Option<String>,
    currency_sign: Option<String>,
    unit: Option<String>,
    unit_display: Option<String>,
    minimum_integer_digits: u8,
    precision: Precision,
    use_grouping: bool,
    notation: String,
    compact_display: Option<String>,
    sign_display: String,
    last_parts: Vec<Part>,
}

#[derive(Default)]
pub(crate) struct Registry {
    inner: Mutex<RegistryInner>,
}

#[derive(Default)]
struct RegistryInner {
    next: u64,
    values: HashMap<u64, NumberFormat>,
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

    fn insert(&self, value: NumberFormat) -> u64 {
        let mut inner = self.inner.lock().expect("Intl registry poisoned");
        let handle = inner.next;
        inner.next = inner
            .next
            .checked_add(1)
            .expect("Intl handle space exhausted");
        inner.values.insert(handle, value);
        handle
    }

    fn remove(&self, handle: u64) {
        self.inner
            .lock()
            .expect("Intl registry poisoned")
            .values
            .remove(&handle);
    }

    fn contains(&self, handle: u64) -> bool {
        self.inner
            .lock()
            .expect("Intl registry poisoned")
            .values
            .contains_key(&handle)
    }
}

struct IntlOwner {
    state: Weak<crate::task::RuntimeState>,
    handle: u64,
}

/// Attach a Rust formatter's lifetime to an unreachable JSI NativeState.
///
/// # Safety
/// `state` must be a live RuntimeState pointer and `handle` a formatter in it.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_owner_create(
    state: *const crate::task::RuntimeState,
    handle: f64,
) -> *mut c_void {
    let Some(state) = crate::task::clone_queue(state) else {
        return std::ptr::null_mut();
    };
    let Ok(handle) = handle_from_number(handle) else {
        return std::ptr::null_mut();
    };
    if !state.intl.contains(handle) {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(IntlOwner {
        state: Arc::downgrade(&state),
        handle,
    }))
    .cast()
}

/// # Safety
/// `owner` is null or an unfreed pointer returned by `ibex2_intl_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_owner_destroy(owner: *mut c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<IntlOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.intl.remove(owner.handle);
    }
}

/// # Safety
/// `owner` is a live pointer returned by `ibex2_intl_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_intl_owner_handle(owner: *const c_void) -> f64 {
    if owner.is_null() {
        return 0.0;
    }
    (*(owner.cast::<IntlOwner>())).handle as f64
}

pub(crate) fn dispatch(
    op: u32,
    args: &[HostArg<'_>],
    state: Option<&crate::task::RuntimeState>,
) -> Option<Result<HostValue, HostError>> {
    if !(CREATE..=SUPPORTED_LOCALES).contains(&op) && op != CURRENCY_DIGITS {
        return None;
    }
    Some(dispatch_inner(op, args, state))
}

fn dispatch_inner(
    op: u32,
    args: &[HostArg<'_>],
    state: Option<&crate::task::RuntimeState>,
) -> Result<HostValue, HostError> {
    if op == SUPPORTED_LOCALES {
        return supported_locales(args).map(HostValue::Str);
    }
    if op == CURRENCY_DIGITS {
        let currency = required_str(args, 0, "currency code")?;
        if !valid_currency(currency) {
            return range("currency is not a well-formed currency code");
        }
        return Ok(HostValue::Number(currency_digits(currency) as f64));
    }
    let state = state.ok_or_else(|| HostError::Failed("no runtime state".into()))?;
    if op == CREATE {
        return NumberFormat::create(args)
            .map(|value| HostValue::Number(state.intl.insert(value) as f64));
    }
    let handle = args
        .first()
        .and_then(number_arg)
        .ok_or_else(|| HostError::InvalidArgument("Intl formatter handle expected".into()))
        .and_then(handle_from_number)?;
    let mut registry = state.intl.inner.lock().expect("Intl registry poisoned");
    let formatter = registry
        .values
        .get_mut(&handle)
        .ok_or_else(|| HostError::Failed("TypeError: incompatible NumberFormat receiver".into()))?;
    match op {
        FORMAT => formatter
            .format(args.get(1), args.get(2))
            .map(HostValue::Str),
        FORMAT_PARTS => {
            formatter.last_parts = formatter.format_parts(args.get(1), args.get(2))?;
            Ok(HostValue::Number(formatter.last_parts.len() as f64))
        }
        PART_TYPE | PART_VALUE => {
            let index = args
                .get(1)
                .and_then(number_arg)
                .filter(|n| n.is_finite() && *n >= 0.0 && n.fract() == 0.0)
                .map(|n| n as usize)
                .ok_or_else(|| HostError::InvalidArgument("Intl part index expected".into()))?;
            let part = formatter
                .last_parts
                .get(index)
                .ok_or_else(|| HostError::InvalidArgument("Intl part index out of range".into()))?;
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

impl NumberFormat {
    fn create(args: &[HostArg<'_>]) -> Result<Self, HostError> {
        if args.len() != 18 {
            return Err(HostError::InvalidArgument(
                "Intl.NumberFormat normalized option vector is incomplete".into(),
            ));
        }
        let requested = required_str(args, 0, "locale list")?;
        let matcher = one_of(
            required_str(args, 1, "locale matcher")?,
            &["lookup", "best fit"],
            "localeMatcher",
        )?;
        // ResolveLocale case-normalizes Unicode type options before comparing
        // them with the locale data selected by this 2020 profile.
        let requested_numbering = optional_str(args, 2)?.map(str::to_ascii_lowercase);
        if requested_numbering
            .as_deref()
            .is_some_and(|value| !valid_unicode_type(value))
        {
            return range("numberingSystem is not a Unicode locale type");
        }
        let style = Style::parse(required_str(args, 3, "style")?)?;
        let currency = optional_str(args, 4)?.map(ascii_uppercase);
        if currency
            .as_ref()
            .is_some_and(|value| !valid_currency(value))
        {
            return range("currency is not a well-formed currency code");
        }
        if style == Style::Currency && currency.is_none() {
            return type_error("currency is required with currency style");
        }
        let currency_display = one_of(
            required_str(args, 5, "currency display")?,
            &["code", "symbol", "narrowSymbol", "name"],
            "currencyDisplay",
        )?
        .to_string();
        let currency_sign = one_of(
            required_str(args, 6, "currency sign")?,
            &["standard", "accounting"],
            "currencySign",
        )?
        .to_string();
        let unit = optional_str(args, 7)?.map(str::to_string);
        if unit.as_ref().is_some_and(|value| !valid_unit(value)) {
            return range("unit is not a sanctioned unit identifier");
        }
        if style == Style::Unit && unit.is_none() {
            return type_error("unit is required with unit style");
        }
        let unit_display = one_of(
            required_str(args, 8, "unit display")?,
            &["short", "narrow", "long"],
            "unitDisplay",
        )?
        .to_string();
        let notation = one_of(
            required_str(args, 9, "notation")?,
            &["standard", "scientific", "engineering", "compact"],
            "notation",
        )?
        .to_string();
        let minimum_integer_digits = integer_option(args, 10, 1, 21, "minimumIntegerDigits")?
            .ok_or_else(|| HostError::InvalidArgument("minimumIntegerDigits missing".into()))?;
        let minimum_fraction = integer_option(args, 11, 0, 20, "minimumFractionDigits")?;
        let maximum_fraction = integer_option(args, 12, 0, 20, "maximumFractionDigits")?;
        let minimum_significant = integer_option(args, 13, 1, 21, "minimumSignificantDigits")?;
        let maximum_significant = integer_option(args, 14, 1, 21, "maximumSignificantDigits")?;
        let compact_display = one_of(
            required_str(args, 15, "compact display")?,
            &["short", "long"],
            "compactDisplay",
        )?
        .to_string();
        let use_grouping = args
            .get(16)
            .and_then(bool_arg)
            .ok_or_else(|| HostError::InvalidArgument("useGrouping must be boolean".into()))?;
        let sign_display = one_of(
            required_str(args, 17, "sign display")?,
            &["auto", "never", "always", "exceptZero"],
            "signDisplay",
        )?
        .to_string();

        let (data_locale, extension_numbering) = resolve_locale(requested, matcher)?;
        let default_numbering = default_numbering_system(&data_locale)?;
        let option_numbering = requested_numbering
            .as_deref()
            .filter(|name| numbering_system_supported(name));
        let numbering_system = option_numbering
            .or_else(|| {
                extension_numbering
                    .as_deref()
                    .filter(|name| numbering_system_supported(name))
            })
            .unwrap_or(&default_numbering)
            .to_string();
        let locale = if extension_numbering.as_deref() == Some(numbering_system.as_str()) {
            format!("{data_locale}-u-nu-{numbering_system}")
        } else {
            data_locale.clone()
        };

        let currency_default = currency.as_deref().map(currency_digits).unwrap_or(0);
        let (minimum_fraction_default, maximum_fraction_default) = if style == Style::Currency {
            (currency_default, currency_default)
        } else if style == Style::Percent {
            (0, 0)
        } else {
            (0, 3)
        };
        let precision = if minimum_significant.is_some() || maximum_significant.is_some() {
            let minimum = minimum_significant.unwrap_or(1);
            let maximum = maximum_significant.unwrap_or(21);
            if maximum < minimum {
                return range("maximumSignificantDigits is less than minimumSignificantDigits");
            }
            Precision::Significant { minimum, maximum }
        } else if minimum_fraction.is_some() || maximum_fraction.is_some() {
            let minimum = minimum_fraction.unwrap_or(minimum_fraction_default);
            let default_maximum = maximum_fraction_default.max(minimum);
            let maximum = maximum_fraction.unwrap_or(default_maximum);
            if maximum < minimum {
                return range("maximumFractionDigits is less than minimumFractionDigits");
            }
            Precision::Fraction { minimum, maximum }
        } else if notation == "compact" {
            Precision::Compact
        } else {
            Precision::Fraction {
                minimum: minimum_fraction_default,
                maximum: maximum_fraction_default,
            }
        };

        let currency = (style == Style::Currency).then_some(currency).flatten();
        let unit = (style == Style::Unit).then_some(unit).flatten();
        let currency_display = (style == Style::Currency).then_some(currency_display);
        let currency_sign = (style == Style::Currency).then_some(currency_sign);
        let unit_display = (style == Style::Unit).then_some(unit_display);
        let compact_display = (notation == "compact").then_some(compact_display);
        let skeleton = make_skeleton(SkeletonOptions {
            style,
            currency: currency.as_deref(),
            currency_display: currency_display.as_deref(),
            currency_sign: currency_sign.as_deref(),
            unit: unit.as_deref(),
            unit_display: unit_display.as_deref(),
            precision: &precision,
            minimum_integer_digits,
            use_grouping,
            notation: &notation,
            compact_display: compact_display.as_deref(),
            sign_display: &sign_display,
            numbering_system: &numbering_system,
        });
        let native = native_formatter(&data_locale, &skeleton)?;
        Ok(Self {
            native,
            locale,
            numbering_system,
            style,
            currency,
            currency_display,
            currency_sign,
            unit,
            unit_display,
            minimum_integer_digits,
            precision,
            use_grouping,
            notation,
            compact_display,
            sign_display,
            last_parts: Vec::new(),
        })
    }

    fn format(
        &self,
        kind: Option<&HostArg<'_>>,
        value: Option<&HostArg<'_>>,
    ) -> Result<String, HostError> {
        native_result(&self.native, kind, value).map(|result| result.text)
    }

    fn format_parts(
        &self,
        kind: Option<&HostArg<'_>>,
        value: Option<&HostArg<'_>>,
    ) -> Result<Vec<Part>, HostError> {
        let result = native_result(&self.native, kind, value)?;
        Ok(partition(&result))
    }

    fn resolved(&self, field: Option<&HostArg<'_>>) -> Result<HostValue, HostError> {
        let field = field
            .and_then(number_arg)
            .filter(|n| n.fract() == 0.0)
            .map(|n| n as u8)
            .ok_or_else(|| HostError::InvalidArgument("resolved option field expected".into()))?;
        let string = |value: &str| Ok(HostValue::Str(value.to_string()));
        match field {
            0 => string(&self.locale),
            1 => string(&self.numbering_system),
            2 => string(self.style.name()),
            3 => option_value(self.currency.as_deref()),
            4 => option_value(self.currency_display.as_deref()),
            5 => option_value(self.currency_sign.as_deref()),
            6 => option_value(self.unit.as_deref()),
            7 => option_value(self.unit_display.as_deref()),
            8 => Ok(HostValue::Number(self.minimum_integer_digits as f64)),
            9 => match self.precision {
                Precision::Fraction { minimum, .. } => Ok(HostValue::Number(minimum as f64)),
                _ => Ok(HostValue::Undefined),
            },
            10 => match self.precision {
                Precision::Fraction { maximum, .. } => Ok(HostValue::Number(maximum as f64)),
                _ => Ok(HostValue::Undefined),
            },
            11 => match self.precision {
                Precision::Significant { minimum, .. } => Ok(HostValue::Number(minimum as f64)),
                _ => Ok(HostValue::Undefined),
            },
            12 => match self.precision {
                Precision::Significant { maximum, .. } => Ok(HostValue::Number(maximum as f64)),
                _ => Ok(HostValue::Undefined),
            },
            13 => Ok(HostValue::Bool(self.use_grouping)),
            14 => string(&self.notation),
            15 => option_value(self.compact_display.as_deref()),
            16 => string(&self.sign_display),
            _ => Err(HostError::InvalidArgument(
                "unknown resolved option field".into(),
            )),
        }
    }
}

struct NativeResult {
    text: String,
    spans: Vec<(i32, usize, usize)>,
    special: Option<&'static str>,
    negative: bool,
}

fn native_result(
    formatter: &NativeFormatter,
    kind: Option<&HostArg<'_>>,
    value: Option<&HostArg<'_>>,
) -> Result<NativeResult, HostError> {
    let kind = kind
        .and_then(HostArg::as_str)
        .ok_or_else(|| HostError::InvalidArgument("numeric kind expected".into()))?;
    let (raw, special, negative) = if kind == "number" {
        let number = value
            .and_then(number_arg)
            .ok_or_else(|| HostError::InvalidArgument("number expected".into()))?;
        let special = if number.is_nan() {
            Some("nan")
        } else if number.is_infinite() {
            Some("infinity")
        } else {
            None
        };
        // ECMAScript has one NaN value for Intl purposes. Hermes' canonical
        // NaN happens to carry the sign bit on this profile; forwarding that
        // representation would make signDisplay produce "-NaN". Normalize
        // it before ICU and never classify NaN as negative.
        let negative = !number.is_nan() && number.is_sign_negative();
        let number = if number.is_nan() { f64::NAN } else { number };
        (
            unsafe { ibex2_icu_number_format_double(formatter.0, number) },
            special,
            negative,
        )
    } else if kind == "bigint" {
        let decimal = value
            .and_then(HostArg::as_str)
            .ok_or_else(|| HostError::InvalidArgument("BigInt decimal expected".into()))?;
        if !valid_decimal_integer(decimal) {
            return Err(HostError::InvalidArgument("invalid BigInt decimal".into()));
        }
        let value = CString::new(decimal).expect("validated decimal contains no NUL");
        (
            unsafe { ibex2_icu_number_format_decimal(formatter.0, value.as_ptr()) },
            None,
            decimal.starts_with('-'),
        )
    } else {
        return Err(HostError::InvalidArgument("unknown numeric kind".into()));
    };
    if raw.is_null() {
        return Err(HostError::Failed("ICU could not format the number".into()));
    }
    struct Guard(*mut c_void);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { ibex2_icu_number_result_destroy(self.0) }
        }
    }
    let guard = Guard(raw);
    let mut length = 0;
    let text = unsafe { ibex2_icu_number_result_text(guard.0, &mut length) };
    if text.is_null() {
        return Err(HostError::Failed("ICU returned no formatted number".into()));
    }
    let text = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), length) };
    let text = std::str::from_utf8(text)
        .map_err(|_| HostError::Failed("ICU returned invalid UTF-8".into()))?
        .to_string();
    let count = unsafe { ibex2_icu_number_result_field_count(guard.0) };
    let mut spans = Vec::with_capacity(count);
    for index in 0..count {
        let (mut field, mut begin, mut end) = (0, 0, 0);
        if unsafe {
            ibex2_icu_number_result_field(guard.0, index, &mut field, &mut begin, &mut end)
        } != 0
            && begin <= end
            && end <= text.len()
            && text.is_char_boundary(begin)
            && text.is_char_boundary(end)
        {
            spans.push((field, begin, end));
        }
    }
    Ok(NativeResult {
        text,
        spans,
        special,
        negative,
    })
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
    for pair in boundaries.windows(2) {
        let (begin, end) = (pair[0], pair[1]);
        if begin == end {
            continue;
        }
        let field = result
            .spans
            .iter()
            .filter(|(_, start, limit)| *start <= begin && end <= *limit)
            .min_by_key(|(_, start, limit)| limit - start)
            .map(|(field, _, _)| *field);
        let value = &result.text[begin..end];
        let kind = part_kind(field, value, result.special, result.negative);
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

fn part_kind(
    field: Option<i32>,
    value: &str,
    special: Option<&'static str>,
    negative: bool,
) -> &'static str {
    match field {
        Some(INTEGER_FIELD) => special.unwrap_or("integer"),
        Some(FRACTION_FIELD) => "fraction",
        Some(DECIMAL_FIELD) => "decimal",
        Some(EXPONENT_SYMBOL_FIELD) => "exponentSeparator",
        Some(EXPONENT_SIGN_FIELD) => "exponentMinusSign",
        Some(EXPONENT_FIELD) => "exponentInteger",
        Some(GROUPING_FIELD) => "group",
        Some(CURRENCY_FIELD) => "currency",
        Some(PERCENT_FIELD) | Some(PERMILL_FIELD) => "percentSign",
        Some(SIGN_FIELD) if has_plus(value) => "plusSign",
        Some(SIGN_FIELD) if negative && has_minus(value) => "minusSign",
        Some(UNIT_FIELD) => "unit",
        Some(COMPACT_FIELD) => "compact",
        _ => "literal",
    }
}

struct SkeletonOptions<'a> {
    style: Style,
    currency: Option<&'a str>,
    currency_display: Option<&'a str>,
    currency_sign: Option<&'a str>,
    unit: Option<&'a str>,
    unit_display: Option<&'a str>,
    precision: &'a Precision,
    minimum_integer_digits: u8,
    use_grouping: bool,
    notation: &'a str,
    compact_display: Option<&'a str>,
    sign_display: &'a str,
    numbering_system: &'a str,
}

fn make_skeleton(options: SkeletonOptions<'_>) -> String {
    let SkeletonOptions {
        style,
        currency,
        currency_display,
        currency_sign,
        unit,
        unit_display,
        precision,
        minimum_integer_digits,
        use_grouping,
        notation,
        compact_display,
        sign_display,
        numbering_system,
    } = options;
    let mut tokens = Vec::new();
    match notation {
        "scientific" | "engineering" => tokens.push(notation.to_string()),
        "compact" => tokens.push(format!("compact-{}", compact_display.unwrap_or("short"))),
        _ => {}
    }
    match style {
        Style::Percent => {
            tokens.push("percent".into());
            tokens.push("scale/100".into());
        }
        Style::Currency => {
            tokens.push(format!(
                "currency/{}",
                currency.expect("validated currency")
            ));
            tokens.push(
                match currency_display.expect("currency display") {
                    "code" => "unit-width-iso-code",
                    "narrowSymbol" => "unit-width-narrow",
                    "name" => "unit-width-full-name",
                    _ => "unit-width-short",
                }
                .into(),
            );
        }
        Style::Unit => {
            tokens.push(format!("unit/{}", unit.expect("validated unit")));
            tokens.push(
                match unit_display.expect("unit display") {
                    "narrow" => "unit-width-narrow",
                    "long" => "unit-width-full-name",
                    _ => "unit-width-short",
                }
                .into(),
            );
        }
        Style::Decimal => {}
    }
    match precision {
        Precision::Fraction { minimum, maximum } => {
            tokens.push(format!(
                ".{}{}",
                "0".repeat(*minimum as usize),
                "#".repeat((maximum - minimum) as usize)
            ));
        }
        Precision::Significant { minimum, maximum } => {
            tokens.push(format!(
                "{}{}",
                "@".repeat(*minimum as usize),
                "#".repeat((maximum - minimum) as usize)
            ));
        }
        Precision::Compact => {}
    }
    tokens.push("rounding-mode-half-up".into());
    if minimum_integer_digits > 1 {
        tokens.push(format!(
            "integer-width/*{}",
            "0".repeat(minimum_integer_digits as usize)
        ));
    }
    if !use_grouping {
        tokens.push("group-off".into());
    }
    let sign = if currency_sign == Some("accounting") {
        match sign_display {
            "always" => "sign-accounting-always",
            "exceptZero" => "sign-accounting-except-zero",
            "never" => "sign-never",
            _ => "sign-accounting",
        }
    } else {
        match sign_display {
            "always" => "sign-always",
            "exceptZero" => "sign-except-zero",
            "never" => "sign-never",
            _ => "sign-auto",
        }
    };
    tokens.push(sign.into());
    tokens.push(format!("numbering-system/{numbering_system}"));
    tokens.join(" ")
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
        let (base, _) = locale_base_and_nu(locale);
        if best_available_locale(&base)?.is_some() {
            result.push(locale);
        }
    }
    Ok(result.join("\n"))
}

fn resolve_locale(requested: &str, _matcher: &str) -> Result<(String, Option<String>), HostError> {
    for locale in locale_list(requested) {
        let (base, nu) = locale_base_and_nu(locale);
        if let Some(available) = best_available_locale(&base)? {
            return Ok((available, nu));
        }
    }
    let default = owned_icu_string(unsafe { ibex2_icu_default_locale() })
        .ok_or_else(|| HostError::Failed("ICU has no default locale".into()))?;
    let (base, _) = locale_base_and_nu(&default);
    let available = best_available_locale(&base)?
        .ok_or_else(|| HostError::Failed("ICU default locale is unavailable".into()))?;
    Ok((available, None))
}

fn locale_base_and_nu(locale: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = locale.split('-').collect();
    let first_extension = parts
        .iter()
        .position(|part| part.len() == 1)
        .unwrap_or(parts.len());
    let base = parts[..first_extension].join("-");
    let mut nu = None;
    let private_use = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("x"))
        .unwrap_or(parts.len());
    if let Some(u) = parts[..private_use]
        .iter()
        .position(|part| part.eq_ignore_ascii_case("u"))
    {
        let mut index = u + 1;
        while index < private_use && parts[index].len() != 1 {
            if parts[index].eq_ignore_ascii_case("nu") {
                index += 1;
                let start = index;
                while index < private_use && parts[index].len() > 2 {
                    index += 1;
                }
                if start < index {
                    nu = Some(parts[start..index].join("-").to_ascii_lowercase());
                }
                break;
            }
            index += 1;
        }
    }
    (base, nu)
}

fn locale_list(value: &str) -> impl Iterator<Item = &str> {
    value.split('\n').filter(|item| !item.is_empty())
}

fn best_available_locale(tag: &str) -> Result<Option<String>, HostError> {
    let tag =
        CString::new(tag).map_err(|_| HostError::InvalidArgument("locale contains NUL".into()))?;
    Ok(owned_icu_string(unsafe {
        ibex2_icu_best_available_locale(tag.as_ptr())
    }))
}

fn default_numbering_system(locale: &str) -> Result<String, HostError> {
    let locale = CString::new(locale).expect("canonical locale contains no NUL");
    owned_icu_string(unsafe { ibex2_icu_default_numbering_system(locale.as_ptr()) })
        .ok_or_else(|| HostError::Failed("ICU has no numbering system for locale".into()))
}

fn numbering_system_supported(name: &str) -> bool {
    CString::new(name)
        .ok()
        .is_some_and(|name| unsafe { ibex2_icu_numbering_system_supported(name.as_ptr()) != 0 })
}

fn currency_digits(currency: &str) -> u8 {
    let currency = CString::new(currency).expect("currency contains no NUL");
    unsafe { ibex2_icu_currency_digits(currency.as_ptr()) as u8 }
}

fn native_formatter(locale: &str, skeleton: &str) -> Result<NativeFormatter, HostError> {
    let locale = CString::new(locale).expect("locale contains no NUL");
    let skeleton = CString::new(skeleton).expect("skeleton contains no NUL");
    let native = unsafe { ibex2_icu_number_formatter_create(locale.as_ptr(), skeleton.as_ptr()) };
    if native.is_null() {
        Err(HostError::Failed(
            "ICU rejected the NumberFormat configuration".into(),
        ))
    } else {
        Ok(NativeFormatter(native))
    }
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
fn number_arg(value: &HostArg<'_>) -> Option<f64> {
    if let HostArg::Number(value) = value {
        Some(*value)
    } else {
        None
    }
}
fn bool_arg(value: &HostArg<'_>) -> Option<bool> {
    if let HostArg::Bool(value) = value {
        Some(*value)
    } else {
        None
    }
}
fn integer_option(
    args: &[HostArg<'_>],
    index: usize,
    min: u8,
    max: u8,
    name: &str,
) -> Result<Option<u8>, HostError> {
    match args.get(index) {
        Some(HostArg::Undefined) => Ok(None),
        Some(HostArg::Number(value))
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= min as f64
                && *value <= max as f64 =>
        {
            Ok(Some(*value as u8))
        }
        _ => range(&format!("{name} is outside its allowed range")),
    }
}
fn handle_from_number(value: f64) -> Result<u64, HostError> {
    if value.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&value) {
        Ok(value as u64)
    } else {
        Err(HostError::InvalidArgument(
            "invalid Intl formatter handle".into(),
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
fn option_value(value: Option<&str>) -> Result<HostValue, HostError> {
    Ok(value
        .map(|v| HostValue::Str(v.to_string()))
        .unwrap_or(HostValue::Undefined))
}
fn range<T>(message: &str) -> Result<T, HostError> {
    Err(HostError::Failed(format!("RangeError: {message}")))
}
fn type_error<T>(message: &str) -> Result<T, HostError> {
    Err(HostError::Failed(format!("TypeError: {message}")))
}
fn ascii_uppercase(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_lowercase() {
                (byte - b'a' + b'A') as char
            } else {
                byte as char
            }
        })
        .collect()
}
fn valid_currency(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}
fn valid_unicode_type(value: &str) -> bool {
    value.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}
fn valid_decimal_integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}
fn has_plus(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '+' | '\u{ff0b}'))
}
fn has_minus(value: &str) -> bool {
    value
        .chars()
        .any(|ch| matches!(ch, '-' | '\u{2212}' | '\u{fe63}' | '\u{ff0d}'))
}

fn valid_unit(value: &str) -> bool {
    const UNITS: &[&str] = &[
        "acre",
        "bit",
        "byte",
        "celsius",
        "centimeter",
        "day",
        "degree",
        "fahrenheit",
        "fluid-ounce",
        "foot",
        "gallon",
        "gigabit",
        "gigabyte",
        "gram",
        "hectare",
        "hour",
        "inch",
        "kilobit",
        "kilobyte",
        "kilogram",
        "kilometer",
        "liter",
        "megabit",
        "megabyte",
        "meter",
        "mile",
        "mile-scandinavian",
        "milliliter",
        "millimeter",
        "millisecond",
        "minute",
        "month",
        "ounce",
        "percent",
        "petabyte",
        "pound",
        "second",
        "stone",
        "terabit",
        "terabyte",
        "week",
        "yard",
        "year",
    ];
    if UNITS.contains(&value) {
        return true;
    }
    let mut parts = value.split("-per-");
    matches!((parts.next(), parts.next(), parts.next()), (Some(a), Some(b), None) if UNITS.contains(&a) && UNITS.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_carries_es2020_rounding_and_display_choices() {
        let skeleton = make_skeleton(SkeletonOptions {
            style: Style::Currency,
            currency: Some("USD"),
            currency_display: Some("code"),
            currency_sign: Some("accounting"),
            unit: None,
            unit_display: None,
            precision: &Precision::Fraction {
                minimum: 2,
                maximum: 4,
            },
            minimum_integer_digits: 3,
            use_grouping: false,
            notation: "scientific",
            compact_display: None,
            sign_display: "always",
            numbering_system: "arab",
        });
        for token in [
            "scientific",
            "currency/USD",
            "unit-width-iso-code",
            ".00##",
            "rounding-mode-half-up",
            "integer-width/*000",
            "group-off",
            "sign-accounting-always",
            "numbering-system/arab",
        ] {
            assert!(
                skeleton.split(' ').any(|item| item == token),
                "missing {token}: {skeleton}"
            );
        }
    }

    #[test]
    fn only_es2020_sanctioned_simple_and_compound_units_pass() {
        assert!(valid_unit("meter"));
        assert!(valid_unit("kilometer-per-hour"));
        assert!(!valid_unit("lightyear"));
        assert!(!valid_unit("meter-per-second-per-hour"));
    }

    #[test]
    fn locale_extension_extracts_only_numbering_system() {
        assert_eq!(
            locale_base_and_nu("de-DE-u-ca-gregory-nu-arab"),
            ("de-DE".into(), Some("arab".into()))
        );
        assert_eq!(locale_base_and_nu("fr-FR"), ("fr-FR".into(), None));
        assert_eq!(locale_base_and_nu("en-x-u-nu-arab"), ("en".into(), None));
        assert_eq!(
            locale_base_and_nu("en-u-nu-arab-x-u-nu-latn"),
            ("en".into(), Some("arab".into()))
        );
        assert_eq!(
            locale_base_and_nu("en-u-nu-arab-foobar"),
            ("en".into(), Some("arab-foobar".into()))
        );
    }
}
