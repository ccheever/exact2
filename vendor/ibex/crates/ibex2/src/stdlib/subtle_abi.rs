//! Primitive/handle projection of [`super::subtle`] for JavaScript engines.
//!
//! This module intentionally returns only numbers, strings, booleans, and
//! byte buffers. `CryptoKey` material remains in `RuntimeState`'s table.

use crate::boundary::{HostArg, HostError, HostValue};
use crate::host_opcodes::subtle::{
    DECRYPT, DERIVE_BITS, DERIVE_KEY, DIGEST, ENCRYPT, EXPORT_KEY, GENERATE_KEY, IMPORT_KEY, SIGN,
    VERIFY,
};

use super::subtle::{
    self, AesGcmParams, DeriveAlgorithm, DerivedKeyAlgorithm, ExportedKey, GenerateAlgorithm,
    HashAlgorithm, ImportAlgorithm, JsonWebKey, KeyFormat, KeyUsage, SignatureAlgorithm,
};

fn failed(error: subtle::Error) -> HostError {
    HostError::Failed(error.to_string())
}

fn invalid(message: impl Into<String>) -> HostError {
    HostError::InvalidArgument(message.into())
}

fn string<'a>(args: &'a [HostArg<'a>], index: usize, label: &str) -> Result<&'a str, HostError> {
    args.get(index)
        .and_then(HostArg::as_str)
        .ok_or_else(|| invalid(format!("expected {label}")))
}

fn optional_string<'a>(args: &'a [HostArg<'a>], index: usize) -> Option<&'a str> {
    args.get(index).and_then(HostArg::as_str)
}

fn bytes<'a>(args: &'a [HostArg<'a>], index: usize, label: &str) -> Result<&'a [u8], HostError> {
    args.get(index)
        .and_then(HostArg::as_bytes)
        .ok_or_else(|| invalid(format!("expected {label}")))
}

fn number(args: &[HostArg<'_>], index: usize, label: &str) -> Result<f64, HostError> {
    match args.get(index) {
        Some(HostArg::Number(value)) if value.is_finite() => Ok(*value),
        _ => Err(invalid(format!("expected {label}"))),
    }
}

fn unsigned_range(
    args: &[HostArg<'_>],
    index: usize,
    label: &str,
    maximum: u32,
) -> Result<u32, HostError> {
    let value = number(args, index, label)?;
    if value < 0.0 || value.fract() != 0.0 || value > f64::from(maximum) {
        return Err(invalid(format!("{label} is outside the accepted range")));
    }
    Ok(value as u32)
}

fn unsigned_long(args: &[HostArg<'_>], index: usize, label: &str) -> Result<u32, HostError> {
    unsigned_range(args, index, label, u32::MAX)
}

fn optional_unsigned_long(
    args: &[HostArg<'_>],
    index: usize,
    label: &str,
) -> Result<Option<u32>, HostError> {
    match args.get(index) {
        Some(HostArg::Number(-1.0)) | Some(HostArg::Undefined) | None => Ok(None),
        _ => unsigned_long(args, index, label).map(Some),
    }
}

fn nullable_unsigned_long(
    args: &[HostArg<'_>],
    index: usize,
    label: &str,
) -> Result<Option<u32>, HostError> {
    match args.get(index) {
        Some(HostArg::Null | HostArg::Undefined) | None => Ok(None),
        _ => unsigned_long(args, index, label).map(Some),
    }
}

fn optional_flag(
    args: &[HostArg<'_>],
    index: usize,
    label: &str,
) -> Result<Option<u32>, HostError> {
    match args.get(index) {
        Some(HostArg::Number(-1.0)) | Some(HostArg::Undefined) | None => Ok(None),
        _ => unsigned_range(args, index, label, 1).map(Some),
    }
}

fn boolean(args: &[HostArg<'_>], index: usize, label: &str) -> Result<bool, HostError> {
    args.get(index)
        .and_then(HostArg::as_bool)
        .ok_or_else(|| invalid(format!("expected {label}")))
}

fn handle(args: &[HostArg<'_>], index: usize) -> Result<u64, HostError> {
    let value = number(args, index, "a CryptoKey handle")?;
    if value.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&value) {
        return Err(invalid("invalid CryptoKey handle"));
    }
    Ok(value as u64)
}

fn state(
    state: Option<&crate::task::RuntimeState>,
) -> Result<&crate::task::RuntimeState, HostError> {
    state.ok_or_else(|| HostError::Failed("OperationError: no runtime state".into()))
}

fn hash(args: &[HostArg<'_>], index: usize) -> Result<HashAlgorithm, HostError> {
    HashAlgorithm::parse(string(args, index, "a hash name")?).map_err(failed)
}

fn key_format(args: &[HostArg<'_>], index: usize) -> Result<KeyFormat, HostError> {
    match string(args, index, "a key format")? {
        "raw" => Ok(KeyFormat::Raw),
        "jwk" => Ok(KeyFormat::Jwk),
        "spki" => Ok(KeyFormat::Spki),
        "pkcs8" => Ok(KeyFormat::Pkcs8),
        format => Err(HostError::Failed(format!(
            "TypeError: key format {format} is not valid"
        ))),
    }
}

fn usages(value: &str) -> Result<Vec<KeyUsage>, HostError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|usage| KeyUsage::parse(usage).map_err(failed))
        .collect()
}

fn jwk_key_ops<'a>(args: &'a [HostArg<'a>]) -> Result<Option<Vec<&'a str>>, HostError> {
    let Some(count) = optional_unsigned_long(args, 10, "JWK key_ops count")? else {
        return Ok(None);
    };
    let count = count as usize;
    if count > subtle::MAX_JWK_KEY_OPS_COUNT {
        return Err(HostError::Failed(
            "DataError: JWK key_ops has too many entries".into(),
        ));
    }
    (0..count)
        .map(|index| string(args, 17 + index, "a JWK key_ops entry"))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn import_algorithm(args: &[HostArg<'_>], name_index: usize) -> Result<ImportAlgorithm, HostError> {
    match string(args, name_index, "an algorithm name")? {
        "HMAC" => Ok(ImportAlgorithm::Hmac {
            hash: hash(args, name_index + 1)?,
            length_bits: optional_unsigned_long(args, name_index + 2, "an HMAC length")?
                .map(|value| value as usize),
        }),
        "AES-GCM" => Ok(ImportAlgorithm::AesGcm),
        "HKDF" => Ok(ImportAlgorithm::Hkdf),
        "PBKDF2" => Ok(ImportAlgorithm::Pbkdf2),
        "ECDSA" => match optional_string(args, 16) {
            Some("P-256") => Ok(ImportAlgorithm::EcdsaP256),
            Some(curve) => Err(HostError::Failed(format!(
                "NotSupportedError: named curve {curve} is not supported"
            ))),
            None => Err(invalid("ECDSA namedCurve")),
        },
        "ED25519" => Ok(ImportAlgorithm::Ed25519),
        name => Err(HostError::Failed(format!(
            "NotSupportedError: key algorithm {name} is not supported"
        ))),
    }
}

fn generate_algorithm(args: &[HostArg<'_>]) -> Result<GenerateAlgorithm, HostError> {
    match string(args, 0, "an algorithm name")? {
        "HMAC" => Ok(GenerateAlgorithm::Hmac {
            hash: hash(args, 1)?,
            length_bits: optional_unsigned_long(args, 2, "an HMAC length")?
                .map(|value| value as usize),
        }),
        "AES-GCM" => Ok(GenerateAlgorithm::AesGcm {
            length_bits: unsigned_range(args, 2, "an AES-GCM length", u16::MAX.into())? as usize,
        }),
        "ECDSA" => match optional_string(args, 5) {
            Some("P-256") => Ok(GenerateAlgorithm::EcdsaP256),
            Some(curve) => Err(HostError::Failed(format!(
                "NotSupportedError: named curve {curve} is not supported"
            ))),
            None => Err(invalid("ECDSA namedCurve")),
        },
        "ED25519" => Ok(GenerateAlgorithm::Ed25519),
        name => Err(HostError::Failed(format!(
            "NotSupportedError: key algorithm {name} is not supported"
        ))),
    }
}

fn signature_algorithm(
    args: &[HostArg<'_>],
    name_index: usize,
) -> Result<SignatureAlgorithm, HostError> {
    match string(args, name_index, "a signature algorithm")? {
        "HMAC" => Ok(SignatureAlgorithm::Hmac),
        "ECDSA" => Ok(SignatureAlgorithm::Ecdsa {
            hash: hash(args, name_index + 1)?,
        }),
        "ED25519" => Ok(SignatureAlgorithm::Ed25519),
        name => Err(HostError::Failed(format!(
            "NotSupportedError: signature algorithm {name} is not supported"
        ))),
    }
}

fn derive_algorithm<'a>(
    args: &'a [HostArg<'a>],
    start: usize,
) -> Result<DeriveAlgorithm<'a>, HostError> {
    match string(args, start, "a derivation algorithm")? {
        "HKDF" => Ok(DeriveAlgorithm::Hkdf {
            hash: hash(args, start + 1)?,
            salt: bytes(args, start + 2, "HKDF salt")?,
            info: bytes(args, start + 3, "HKDF info")?,
        }),
        "PBKDF2" => {
            let iterations = unsigned_long(args, start + 4, "PBKDF2 iterations")?;
            Ok(DeriveAlgorithm::Pbkdf2 {
                hash: hash(args, start + 1)?,
                salt: bytes(args, start + 2, "PBKDF2 salt")?,
                iterations,
            })
        }
        name => Err(HostError::Failed(format!(
            "NotSupportedError: derivation algorithm {name} is not supported"
        ))),
    }
}

fn derived_algorithm(args: &[HostArg<'_>], start: usize) -> Result<DerivedKeyAlgorithm, HostError> {
    match string(args, start, "a derived key algorithm")? {
        "HMAC" => Ok(DerivedKeyAlgorithm::Hmac {
            hash: hash(args, start + 1)?,
            length_bits: optional_unsigned_long(args, start + 2, "an HMAC length")?
                .map(|value| value as usize),
        }),
        "AES-GCM" => Ok(DerivedKeyAlgorithm::AesGcm {
            length_bits: unsigned_range(args, start + 2, "an AES-GCM length", u16::MAX.into())?
                as usize,
        }),
        name => Err(HostError::Failed(format!(
            "NotSupportedError: derived key algorithm {name} is not supported"
        ))),
    }
}

fn with_key<T>(
    state: &crate::task::RuntimeState,
    handle: u64,
    f: impl FnOnce(&subtle::CryptoKey) -> subtle::Result<T>,
) -> Result<T, HostError> {
    state
        .with_crypto_key(handle, f)
        .ok_or_else(|| {
            HostError::Failed("InvalidAccessError: CryptoKey is released or unknown".into())
        })?
        .map_err(failed)
}

pub(crate) fn dispatch(
    op: u32,
    args: &[HostArg<'_>],
    runtime: Option<&crate::task::RuntimeState>,
) -> Option<Result<HostValue, HostError>> {
    if !(DIGEST..=DERIVE_KEY).contains(&op) {
        return None;
    }
    Some((|| {
        if op == DIGEST {
            return subtle::digest(hash(args, 0)?, bytes(args, 1, "digest data")?)
                .map(HostValue::Bytes)
                .map_err(failed);
        }
        let runtime = state(runtime)?;
        match op {
            IMPORT_KEY => {
                let format = key_format(args, 0)?;
                let algorithm = import_algorithm(args, 2)?;
                subtle::ensure_import_feature(algorithm).map_err(failed)?;
                let extractable = boolean(args, 5, "extractable")?;
                let usages = usages(string(args, 6, "key usages")?)?;
                let key = match format {
                    KeyFormat::Raw => subtle::import_raw_key(
                        bytes(args, 1, "raw key data")?,
                        algorithm,
                        extractable,
                        &usages,
                    ),
                    KeyFormat::Jwk => {
                        let ext = match optional_flag(args, 11, "JWK ext")? {
                            None => None,
                            Some(0) => Some(false),
                            Some(1) => Some(true),
                            _ => return Err(invalid("JWK ext must be boolean when present")),
                        };
                        let kty = string(args, 7, "JWK kty")?;
                        let k = optional_string(args, 1);
                        let alg = optional_string(args, 8);
                        let key_use = optional_string(args, 9);
                        let key_ops = jwk_key_ops(args)?;
                        let crv = optional_string(args, 12);
                        let x = optional_string(args, 13);
                        let y = optional_string(args, 14);
                        let d = optional_string(args, 15);
                        #[cfg(feature = "crypto")]
                        subtle::preflight_jwk_fields(
                            algorithm,
                            subtle::JwkFieldRefs {
                                kty,
                                k,
                                crv,
                                x,
                                y,
                                d,
                                alg,
                                key_use,
                            },
                        )
                        .map_err(failed)?;
                        #[cfg(feature = "crypto")]
                        subtle::preflight_jwk_key_ops(key_ops.as_deref()).map_err(failed)?;
                        let jwk = JsonWebKey {
                            kty: kty.into(),
                            k: k.map(str::to_owned),
                            crv: crv.map(str::to_owned),
                            x: x.map(str::to_owned),
                            y: y.map(str::to_owned),
                            d: d.map(str::to_owned),
                            alg: alg.map(str::to_owned),
                            key_use: key_use.map(str::to_owned),
                            key_ops: key_ops
                                .map(|ops| ops.into_iter().map(str::to_owned).collect()),
                            ext,
                        };
                        subtle::import_jwk_key(&jwk, algorithm, extractable, &usages)
                    }
                    KeyFormat::Spki => subtle::import_spki_key(
                        bytes(args, 1, "spki key data")?,
                        algorithm,
                        extractable,
                        &usages,
                    ),
                    KeyFormat::Pkcs8 => subtle::import_pkcs8_key(
                        bytes(args, 1, "pkcs8 key data")?,
                        algorithm,
                        extractable,
                        &usages,
                    ),
                }
                .map_err(failed)?;
                Ok(HostValue::Number(runtime.store_crypto_key(key) as f64))
            }
            EXPORT_KEY => {
                let format = key_format(args, 1)?;
                with_key(runtime, handle(args, 0)?, |key| {
                    subtle::export_key(format, key)
                })
                .map(|exported| match exported {
                    ExportedKey::Raw(bytes) => HostValue::Bytes(bytes),
                    ExportedKey::Pkcs8(bytes) | ExportedKey::Spki(bytes) => HostValue::Bytes(bytes),
                    ExportedKey::Jwk(jwk) => {
                        let encoded = if let Some(k) = jwk.k {
                            k
                        } else {
                            [jwk.x, jwk.y, jwk.d]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(".")
                        };
                        HostValue::Str(encoded)
                    }
                })
            }
            GENERATE_KEY => {
                let algorithm = generate_algorithm(args)?;
                let extractable = boolean(args, 3, "extractable")?;
                let usages = usages(string(args, 4, "key usages")?)?;
                if matches!(
                    algorithm,
                    GenerateAlgorithm::EcdsaP256 | GenerateAlgorithm::Ed25519
                ) {
                    let pair = subtle::generate_key_pair(algorithm, extractable, &usages)
                        .map_err(failed)?;
                    let public = runtime.store_crypto_key(pair.public_key);
                    let private = runtime.store_crypto_key(pair.private_key);
                    Ok(HostValue::Str(format!("{public},{private}")))
                } else {
                    let key =
                        subtle::generate_key(algorithm, extractable, &usages).map_err(failed)?;
                    Ok(HostValue::Number(runtime.store_crypto_key(key) as f64))
                }
            }
            SIGN => {
                let algorithm = signature_algorithm(args, 1)?;
                let data = bytes(args, 3, "data")?;
                with_key(runtime, handle(args, 0)?, |key| {
                    subtle::sign_with_algorithm(algorithm, key, data)
                })
                .map(HostValue::Bytes)
            }
            VERIFY => {
                let algorithm = signature_algorithm(args, 1)?;
                let signature = bytes(args, 3, "signature")?;
                let data = bytes(args, 4, "data")?;
                with_key(runtime, handle(args, 0)?, |key| {
                    subtle::verify_with_algorithm(algorithm, key, signature, data)
                })
                .map(HostValue::Bool)
            }
            ENCRYPT | DECRYPT => {
                let params = AesGcmParams {
                    iv: bytes(args, 1, "AES-GCM iv")?,
                    additional_data: bytes(args, 2, "AES-GCM additionalData")?,
                    tag_length_bits: unsigned_range(args, 3, "AES-GCM tagLength", u8::MAX.into())?
                        as usize,
                };
                let input = bytes(args, 4, "AES-GCM data")?;
                with_key(runtime, handle(args, 0)?, |key| {
                    if op == ENCRYPT {
                        subtle::encrypt(key, params, input)
                    } else {
                        subtle::decrypt(key, params, input)
                    }
                })
                .map(HostValue::Bytes)
            }
            DERIVE_BITS => {
                let algorithm = derive_algorithm(args, 1)?;
                let length = nullable_unsigned_long(args, 6, "derived bit length")?;
                with_key(runtime, handle(args, 0)?, |key| {
                    #[cfg(feature = "crypto")]
                    subtle::preflight_derive_access(algorithm, key, KeyUsage::DeriveBits)?;
                    let length = length.ok_or_else(|| {
                        subtle::Error::operation("deriveBits length must not be null")
                    })?;
                    subtle::derive_bits(algorithm, key, length as usize)
                })
                .map(HostValue::Bytes)
            }
            DERIVE_KEY => {
                let algorithm = derive_algorithm(args, 1)?;
                let derived = derived_algorithm(args, 6)?;
                let extractable = boolean(args, 9, "extractable")?;
                let usages = usages(string(args, 10, "key usages")?)?;
                let key = with_key(runtime, handle(args, 0)?, |key| {
                    subtle::derive_key(algorithm, key, derived, extractable, &usages)
                })?;
                Ok(HostValue::Number(runtime.store_crypto_key(key) as f64))
            }
            _ => unreachable!(),
        }
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_long_bounds_are_repeated_at_the_native_boundary() {
        assert_eq!(
            unsigned_long(&[HostArg::Number(f64::from(u32::MAX))], 0, "value").unwrap(),
            u32::MAX
        );
        for value in [-1.0, 1.5, 4_294_967_296.0, f64::NAN, f64::INFINITY] {
            assert!(unsigned_long(&[HostArg::Number(value)], 0, "value").is_err());
        }
    }

    #[test]
    fn key_format_is_a_case_sensitive_enum_at_the_native_boundary() {
        assert_eq!(
            key_format(&[HostArg::Str("raw")], 0).unwrap(),
            KeyFormat::Raw
        );
        for format in ["RAW", "unknown"] {
            assert_eq!(
                key_format(&[HostArg::Str(format)], 0)
                    .unwrap_err()
                    .to_string(),
                format!("TypeError: key format {format} is not valid")
            );
        }
    }

    #[test]
    fn nullable_unsigned_long_uses_enforce_range_conversion() {
        assert_eq!(
            nullable_unsigned_long(&[HostArg::Null], 0, "value").unwrap(),
            None
        );
        assert_eq!(
            nullable_unsigned_long(&[HostArg::Number(0.0)], 0, "value").unwrap(),
            Some(0)
        );
        assert_eq!(
            nullable_unsigned_long(&[HostArg::Undefined], 0, "value").unwrap(),
            None
        );
        assert_eq!(nullable_unsigned_long(&[], 0, "value").unwrap(), None);
        for value in [
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            4_294_967_296.0,
        ] {
            assert!(nullable_unsigned_long(&[HostArg::Number(value)], 0, "value").is_err());
        }
    }
}
