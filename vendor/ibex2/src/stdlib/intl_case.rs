//! Locale-sensitive String case mapping over the Linux ICU closure.
//!
//! The engine adapter carries JavaScript strings as explicit UTF-16LE bytes,
//! preserving embedded NUL and lone surrogates. ICU performs only the Unicode
//! mapping; observable JavaScript coercion and locale-list selection stay in
//! the binding prelude.
//!
//! @ref LLP 0057#3-the-boundary — Rust owns standard-library semantics

use crate::boundary::{HostArg, HostError, HostValue};
use std::ffi::{c_char, c_int};

const INTL_CASE: u32 = 97;

extern "C" {
    fn ibex2_icu_case_map(
        mode: u8,
        locale_data: *const c_char,
        locale_length: usize,
        input: *const u16,
        input_length: usize,
        output: *mut u16,
        output_capacity: usize,
        output_length: *mut usize,
    ) -> c_int;
}

pub(crate) fn dispatch(op: u32, args: &[HostArg<'_>]) -> Option<Result<HostValue, HostError>> {
    if op != INTL_CASE {
        return None;
    }
    Some(map(args).map(HostValue::Bytes))
}

fn map(args: &[HostArg<'_>]) -> Result<Vec<u8>, HostError> {
    let mode = match args.first().and_then(HostArg::as_str) {
        Some("lower") => 0,
        Some("upper") => 1,
        _ => return invalid("Intl case mode must be lower or upper"),
    };
    let locale = args
        .get(1)
        .and_then(HostArg::as_str)
        .ok_or_else(|| HostError::InvalidArgument("Intl case locale must be a string".into()))?;
    let bytes = args.get(2).and_then(HostArg::as_bytes).ok_or_else(|| {
        HostError::InvalidArgument("Intl case input must be UTF-16LE bytes".into())
    })?;
    if bytes.len() % 2 != 0 {
        return invalid("Intl case input has an incomplete UTF-16LE code unit");
    }

    let input: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect();
    let mut required = 0usize;
    let status = unsafe {
        ibex2_icu_case_map(
            mode,
            locale.as_ptr().cast(),
            locale.len(),
            input.as_ptr(),
            input.len(),
            std::ptr::null_mut(),
            0,
            &mut required,
        )
    };
    check_status(status)?;

    let mut output = vec![0u16; required];
    let mut written = required;
    let status = unsafe {
        ibex2_icu_case_map(
            mode,
            locale.as_ptr().cast(),
            locale.len(),
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            output.len(),
            &mut written,
        )
    };
    check_status(status)?;
    if written > output.len() {
        return Err(HostError::Failed(
            "Intl case mapper returned an invalid output length".into(),
        ));
    }
    output.truncate(written);

    let byte_length = output
        .len()
        .checked_mul(2)
        .ok_or_else(|| HostError::Failed("Intl case result is too large".into()))?;
    let mut result = Vec::with_capacity(byte_length);
    for unit in output {
        result.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(result)
}

fn check_status(status: c_int) -> Result<(), HostError> {
    match status {
        0 => Ok(()),
        1 => invalid("Intl case mapper rejected its input"),
        2 => Err(HostError::Failed("ICU case mapping failed".into())),
        3 => Err(HostError::Failed(
            "Intl case result changed between sizing and mapping".into(),
        )),
        _ => Err(HostError::Failed("native Intl case mapping failed".into())),
    }
}

fn invalid<T>(message: &str) -> Result<T, HostError> {
    Err(HostError::InvalidArgument(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_other_ops() {
        assert!(dispatch(96, &[]).is_none());
        assert!(dispatch(98, &[]).is_none());
    }

    #[test]
    fn rejects_odd_utf16_byte_count_before_native_call() {
        let args = [
            HostArg::Str("lower"),
            HostArg::Str("en-US"),
            HostArg::Bytes(&[0x61]),
        ];
        assert!(matches!(
            dispatch(INTL_CASE, &args),
            Some(Err(HostError::InvalidArgument(_)))
        ));
    }

    #[test]
    fn rejects_unknown_mode_before_native_call() {
        let args = [
            HostArg::Str("title"),
            HostArg::Str("en-US"),
            HostArg::Bytes(&[]),
        ];
        assert!(matches!(
            dispatch(INTL_CASE, &args),
            Some(Err(HostError::InvalidArgument(_)))
        ));
    }
}
