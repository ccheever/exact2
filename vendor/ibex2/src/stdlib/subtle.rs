//! Engine-free WebCrypto algorithms and owned secret keys.
//!
//! The JavaScript binding stores these values behind runtime-owned handles;
//! only an explicit export of an extractable key exposes its material.
//!
//! @ref LLP 0059.000#314-cryptosubtle--pure-ungated-author-required — opaque keys and the L2 algorithm set

#[cfg(feature = "crypto")]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::fmt;

#[cfg(feature = "crypto-asymmetric")]
mod asymmetric;
#[cfg(feature = "crypto-asymmetric")]
mod der;

/// PBKDF2 remains a synchronous pure operation under LLP 0059.000 §1.1, so
/// bound the work admitted by one host call instead of allowing an unbounded
/// iteration count to monopolize the runtime thread.
pub const MAX_PBKDF2_ITERATIONS: u32 = 1_000_000;
/// PBKDF2's combined synchronous-work ceiling is measured in one PRF call or
/// one hash-block of salt input per derived block. The salt ceiling keeps the
/// first PRF call from hiding unbounded work outside the iteration count.
pub const MAX_PBKDF2_SALT_BYTES: usize = 125_000;
/// HKDF hashes `info` once for every output block. Count that expansion work,
/// plus the salt and input-key work of extract, in hash compression blocks so
/// large caller inputs cannot multiply into an unbounded synchronous call.
pub const MAX_HKDF_COMPRESSION_BLOCKS: usize = 65_536;

/// Bound caller-selected secret/output sizes so a pure synchronous host call
/// cannot turn an integer parameter into an effectively unbounded allocation.
/// Values are bits because that is the unit WebCrypto exposes.
pub const MAX_DERIVED_BITS: usize = 1_000_000;
pub const MAX_HMAC_KEY_BITS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorName {
    NotSupportedError,
    InvalidAccessError,
    DataError,
    OperationError,
    SyntaxError,
}

impl ErrorName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotSupportedError => "NotSupportedError",
            Self::InvalidAccessError => "InvalidAccessError",
            Self::DataError => "DataError",
            Self::OperationError => "OperationError",
            Self::SyntaxError => "SyntaxError",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub name: ErrorName,
    pub message: String,
}

impl Error {
    fn new(name: ErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }

    fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorName::NotSupportedError, message)
    }

    #[cfg(feature = "crypto")]
    fn invalid_access(message: impl Into<String>) -> Self {
        Self::new(ErrorName::InvalidAccessError, message)
    }

    #[cfg(feature = "crypto")]
    fn data(message: impl Into<String>) -> Self {
        Self::new(ErrorName::DataError, message)
    }

    pub(crate) fn operation(message: impl Into<String>) -> Self {
        Self::new(ErrorName::OperationError, message)
    }

    fn syntax(message: impl Into<String>) -> Self {
        Self::new(ErrorName::SyntaxError, message)
    }

    pub fn feature_unavailable() -> Self {
        Self::unsupported("crypto.subtle was omitted from this build")
    }

    pub fn asymmetric_feature_unavailable() -> Self {
        Self::unsupported(
            "asymmetric crypto algorithms were omitted from this build; enable crypto-asymmetric",
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name.as_str(), self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sha256 => "SHA-256",
            Self::Sha384 => "SHA-384",
            Self::Sha512 => "SHA-512",
        }
    }

    pub const fn output_bits(self) -> usize {
        match self {
            Self::Sha256 => 256,
            Self::Sha384 => 384,
            Self::Sha512 => 512,
        }
    }

    #[cfg(feature = "crypto")]
    const fn output_bytes(self) -> usize {
        self.output_bits() / 8
    }

    #[cfg(feature = "crypto")]
    const fn compression_block_bytes(self) -> usize {
        match self {
            Self::Sha256 => 64,
            Self::Sha384 | Self::Sha512 => 128,
        }
    }

    #[cfg(feature = "crypto")]
    const fn hmac_default_bits(self) -> usize {
        match self {
            Self::Sha256 => 512,
            Self::Sha384 | Self::Sha512 => 1024,
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name.to_ascii_uppercase().as_str() {
            "SHA-256" => Ok(Self::Sha256),
            "SHA-384" => Ok(Self::Sha384),
            "SHA-512" => Ok(Self::Sha512),
            _ => Err(Error::unsupported(format!(
                "digest algorithm {name} is not supported"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum KeyUsage {
    Encrypt,
    Decrypt,
    WrapKey,
    UnwrapKey,
    Sign,
    Verify,
    DeriveKey,
    DeriveBits,
}

impl KeyUsage {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Encrypt => "encrypt",
            Self::Decrypt => "decrypt",
            Self::WrapKey => "wrapKey",
            Self::UnwrapKey => "unwrapKey",
            Self::Sign => "sign",
            Self::Verify => "verify",
            Self::DeriveKey => "deriveKey",
            Self::DeriveBits => "deriveBits",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "encrypt" => Ok(Self::Encrypt),
            "decrypt" => Ok(Self::Decrypt),
            "wrapKey" => Ok(Self::WrapKey),
            "unwrapKey" => Ok(Self::UnwrapKey),
            "sign" => Ok(Self::Sign),
            "verify" => Ok(Self::Verify),
            "deriveKey" => Ok(Self::DeriveKey),
            "deriveBits" => Ok(Self::DeriveBits),
            _ => Err(Error::syntax(format!("unknown key usage {value}"))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyAlgorithm {
    Hmac {
        hash: HashAlgorithm,
        length_bits: usize,
    },
    AesGcm {
        length_bits: usize,
    },
    Hkdf,
    Pbkdf2,
    EcdsaP256,
    Ed25519,
}

impl KeyAlgorithm {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Hmac { .. } => "HMAC",
            Self::AesGcm { .. } => "AES-GCM",
            Self::Hkdf => "HKDF",
            Self::Pbkdf2 => "PBKDF2",
            Self::EcdsaP256 => "ECDSA",
            Self::Ed25519 => "Ed25519",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportAlgorithm {
    Hmac {
        hash: HashAlgorithm,
        length_bits: Option<usize>,
    },
    AesGcm,
    Hkdf,
    Pbkdf2,
    EcdsaP256,
    Ed25519,
}

pub(crate) fn ensure_import_feature(algorithm: ImportAlgorithm) -> Result<()> {
    if matches!(
        algorithm,
        ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519
    ) {
        #[cfg(not(feature = "crypto-asymmetric"))]
        return Err(Error::asymmetric_feature_unavailable());
    }
    #[cfg(not(feature = "crypto"))]
    return Err(Error::feature_unavailable());
    #[cfg(feature = "crypto")]
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerateAlgorithm {
    Hmac {
        hash: HashAlgorithm,
        length_bits: Option<usize>,
    },
    AesGcm {
        length_bits: usize,
    },
    EcdsaP256,
    Ed25519,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureAlgorithm {
    Hmac,
    Ecdsa { hash: HashAlgorithm },
    Ed25519,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedKeyAlgorithm {
    Hmac {
        hash: HashAlgorithm,
        length_bits: Option<usize>,
    },
    AesGcm {
        length_bits: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DeriveAlgorithm<'a> {
    Hkdf {
        hash: HashAlgorithm,
        salt: &'a [u8],
        info: &'a [u8],
    },
    Pbkdf2 {
        hash: HashAlgorithm,
        salt: &'a [u8],
        iterations: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct AesGcmParams<'a> {
    pub iv: &'a [u8],
    pub additional_data: &'a [u8],
    pub tag_length_bits: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyFormat {
    Raw,
    Jwk,
    Pkcs8,
    Spki,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonWebKey {
    pub kty: String,
    pub k: Option<String>,
    pub crv: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
    pub d: Option<String>,
    pub alg: Option<String>,
    pub key_use: Option<String>,
    pub key_ops: Option<Vec<String>>,
    pub ext: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportedKey {
    Raw(Vec<u8>),
    Pkcs8(Vec<u8>),
    Spki(Vec<u8>),
    Jwk(JsonWebKey),
}

#[derive(Debug)]
pub struct CryptoKeyPair {
    pub public_key: CryptoKey,
    pub private_key: CryptoKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    any(not(feature = "crypto"), not(feature = "crypto-asymmetric")),
    allow(dead_code)
)]
enum KeyType {
    Secret,
    Public,
    Private,
}

/// An owned secret key. Debug output deliberately excludes material.
pub struct CryptoKey {
    material: Vec<u8>,
    public_material: Option<Vec<u8>>,
    key_type: KeyType,
    algorithm: KeyAlgorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
}

impl fmt::Debug for CryptoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CryptoKey")
            .field("type", &self.key_type())
            .field("algorithm", &self.algorithm)
            .field("extractable", &self.extractable)
            .field("usages", &self.usages)
            .finish_non_exhaustive()
    }
}

impl Drop for CryptoKey {
    fn drop(&mut self) {
        self.material.fill(0);
        if let Some(public) = &mut self.public_material {
            public.fill(0);
        }
    }
}

impl CryptoKey {
    pub const fn key_type(&self) -> &'static str {
        match self.key_type {
            KeyType::Secret => "secret",
            KeyType::Public => "public",
            KeyType::Private => "private",
        }
    }

    pub const fn extractable(&self) -> bool {
        self.extractable
    }

    pub const fn algorithm(&self) -> &KeyAlgorithm {
        &self.algorithm
    }

    pub fn usages(&self) -> &[KeyUsage] {
        &self.usages
    }

    #[cfg(feature = "crypto")]
    fn permits(&self, usage: KeyUsage) -> bool {
        self.usages.contains(&usage)
    }
}

#[cfg(feature = "crypto")]
fn validate_usages_borrowed(usages: &[KeyUsage], allowed: &[KeyUsage]) -> Result<()> {
    for usage in usages {
        if !allowed.contains(usage) {
            return Err(Error::syntax(format!(
                "{} is not a valid usage for this key",
                usage.name()
            )));
        }
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn validate_usages(usages: &[KeyUsage], allowed: &[KeyUsage]) -> Result<Vec<KeyUsage>> {
    validate_usages_borrowed(usages, allowed)?;
    let mut normalized = Vec::with_capacity(usages.len());
    for usage in usages {
        if !normalized.contains(usage) {
            normalized.push(*usage);
        }
    }
    Ok(normalized)
}

#[cfg(feature = "crypto")]
fn require_usage(key: &CryptoKey, usage: KeyUsage) -> Result<()> {
    if key.permits(usage) {
        Ok(())
    } else {
        Err(Error::invalid_access(format!(
            "key does not allow {}",
            usage.name()
        )))
    }
}

#[cfg(feature = "crypto")]
fn jwk_algorithm(algorithm: &KeyAlgorithm) -> &'static str {
    match algorithm {
        KeyAlgorithm::Hmac {
            hash: HashAlgorithm::Sha256,
            ..
        } => "HS256",
        KeyAlgorithm::Hmac {
            hash: HashAlgorithm::Sha384,
            ..
        } => "HS384",
        KeyAlgorithm::Hmac {
            hash: HashAlgorithm::Sha512,
            ..
        } => "HS512",
        KeyAlgorithm::AesGcm { length_bits: 128 } => "A128GCM",
        KeyAlgorithm::AesGcm { length_bits: 256 } => "A256GCM",
        _ => "",
    }
}

#[cfg(feature = "crypto")]
fn validate_jwk_key_ops(key_ops: Option<&[String]>, usages: &[KeyUsage]) -> Result<()> {
    if let Some(ops) = key_ops {
        for (index, op) in ops.iter().enumerate() {
            KeyUsage::parse(op).map_err(|_| Error::data("JWK key_ops is invalid"))?;
            if ops[..index].iter().any(|earlier| earlier == op) {
                return Err(Error::data("JWK key_ops contains a duplicate"));
            }
        }
        if usages
            .iter()
            .any(|usage| !ops.iter().any(|op| op == usage.name()))
        {
            return Err(Error::data(
                "JWK key_ops does not contain every requested usage",
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn validate_jwk(
    jwk: &JsonWebKey,
    expected_alg: &str,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<()> {
    if jwk.kty != "oct" {
        return Err(Error::data("JWK kty must be oct"));
    }
    if let Some(alg) = &jwk.alg {
        if alg != expected_alg {
            return Err(Error::data(
                "JWK alg does not match the requested algorithm",
            ));
        }
    }
    if extractable && jwk.ext == Some(false) {
        return Err(Error::data(
            "a non-extractable JWK cannot be imported as extractable",
        ));
    }
    validate_jwk_key_ops(jwk.key_ops.as_deref(), usages)?;
    if let Some(key_use) = &jwk.key_use {
        let expected = if expected_alg.starts_with("HS") {
            "sig"
        } else {
            "enc"
        };
        if key_use != expected {
            return Err(Error::data(
                "JWK use is inconsistent with the requested algorithm",
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn validate_aes_length(length_bits: usize) -> Result<()> {
    match length_bits {
        128 | 256 => Ok(()),
        192 => Err(Error::unsupported("AES-GCM-192 is not supported by ring")),
        _ => Err(Error::data("AES-GCM keys must be 128 or 256 bits")),
    }
}

#[cfg(feature = "crypto")]
fn validate_hmac_generated_length(length_bits: usize) -> Result<()> {
    if length_bits == 0 {
        return Err(Error::operation("HMAC key length must be positive"));
    }
    if length_bits > MAX_HMAC_KEY_BITS {
        return Err(Error::operation(format!(
            "HMAC key length exceeds the per-call limit of {MAX_HMAC_KEY_BITS} bits"
        )));
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn validate_hmac_import_length(
    material_bytes: usize,
    requested_bits: Option<usize>,
) -> Result<usize> {
    if material_bytes == 0 {
        return Err(Error::data("HMAC key data must not be empty"));
    }
    let available = material_bytes
        .checked_mul(8)
        .ok_or_else(|| Error::operation("HMAC key data is too large"))?;
    let length_bits = requested_bits.unwrap_or(available);
    if length_bits > MAX_HMAC_KEY_BITS {
        return Err(Error::operation(format!(
            "HMAC key length exceeds the per-call limit of {MAX_HMAC_KEY_BITS} bits"
        )));
    }
    if length_bits == 0 || length_bits > available || length_bits <= available - 8 {
        return Err(Error::data(
            "HMAC length does not describe the supplied key data",
        ));
    }
    Ok(length_bits)
}

#[cfg(feature = "crypto")]
fn preflight_secret_import(
    material_bytes: usize,
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<()> {
    match algorithm {
        ImportAlgorithm::Hmac { length_bits, .. } => {
            validate_hmac_import_length(material_bytes, length_bits)?;
            validate_usages_borrowed(usages, &[KeyUsage::Sign, KeyUsage::Verify])?;
        }
        ImportAlgorithm::AesGcm => {
            let length_bits = material_bytes
                .checked_mul(8)
                .ok_or_else(|| Error::operation("AES-GCM key data is too large"))?;
            validate_aes_length(length_bits)?;
            validate_usages_borrowed(
                usages,
                &[
                    KeyUsage::Encrypt,
                    KeyUsage::Decrypt,
                    KeyUsage::WrapKey,
                    KeyUsage::UnwrapKey,
                ],
            )?;
        }
        ImportAlgorithm::Hkdf => {
            if extractable {
                return Err(Error::syntax("HKDF base keys must be non-extractable"));
            }
            validate_usages_borrowed(usages, &[KeyUsage::DeriveKey, KeyUsage::DeriveBits])?;
        }
        ImportAlgorithm::Pbkdf2 => {
            if extractable {
                return Err(Error::syntax("PBKDF2 base keys must be non-extractable"));
            }
            validate_usages_borrowed(usages, &[KeyUsage::DeriveKey, KeyUsage::DeriveBits])?;
        }
        ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519 => {
            return Err(Error::invalid_access(
                "asymmetric key material must use an asymmetric importer",
            ));
        }
    }
    if usages.is_empty() {
        return Err(Error::syntax(
            "secret keys must have at least one permitted usage",
        ));
    }
    Ok(())
}

#[cfg(feature = "crypto")]
const MAX_JWK_METADATA_BYTES: usize = 64;
#[cfg(feature = "crypto")]
const MAX_JWK_KEY_OPS_BYTES: usize = 128;
pub(crate) const MAX_JWK_KEY_OPS_COUNT: usize = 8;

/// Bound borrowed JWK members before the ABI constructs owned strings. The
/// decoder performs exact semantic checks after this allocation preflight.
#[cfg(feature = "crypto")]
pub(crate) struct JwkFieldRefs<'a> {
    pub(crate) kty: &'a str,
    pub(crate) k: Option<&'a str>,
    pub(crate) crv: Option<&'a str>,
    pub(crate) x: Option<&'a str>,
    pub(crate) y: Option<&'a str>,
    pub(crate) d: Option<&'a str>,
    pub(crate) alg: Option<&'a str>,
    pub(crate) key_use: Option<&'a str>,
}

#[cfg(feature = "crypto")]
pub(crate) fn preflight_jwk_key_ops(key_ops: Option<&[&str]>) -> Result<()> {
    let Some(ops) = key_ops else {
        return Ok(());
    };
    if ops.len() > MAX_JWK_KEY_OPS_COUNT {
        return Err(Error::data("JWK key_ops has too many entries"));
    }
    let total = ops.iter().try_fold(0usize, |total, op| {
        total
            .checked_add(op.len())
            .and_then(|total| total.checked_add(1))
    });
    if ops.iter().any(|op| op.len() > MAX_JWK_KEY_OPS_BYTES)
        || total.is_none_or(|total| total > MAX_JWK_KEY_OPS_BYTES)
    {
        return Err(Error::data("JWK key_ops is too large"));
    }
    Ok(())
}

#[cfg(feature = "crypto")]
pub(crate) fn preflight_jwk_fields(
    algorithm: ImportAlgorithm,
    fields: JwkFieldRefs<'_>,
) -> Result<()> {
    let JwkFieldRefs {
        kty,
        k,
        crv,
        x,
        y,
        d,
        alg,
        key_use,
    } = fields;
    for (label, value) in [
        ("kty", Some(kty)),
        ("crv", crv),
        ("alg", alg),
        ("use", key_use),
    ] {
        if value.is_some_and(|value| value.len() > MAX_JWK_METADATA_BYTES) {
            return Err(Error::data(format!("JWK {label} is too large")));
        }
    }
    let component_limit = match algorithm {
        ImportAlgorithm::Hmac { .. } => MAX_HMAC_KEY_BITS.div_ceil(6),
        ImportAlgorithm::AesGcm | ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519 => 43,
        ImportAlgorithm::Hkdf | ImportAlgorithm::Pbkdf2 => 0,
    };
    for (label, value) in [("k", k), ("x", x), ("y", y), ("d", d)] {
        if value.is_some_and(|value| value.len() > component_limit) {
            let message = format!("JWK {label} exceeds the requested algorithm's size limit");
            return Err(if matches!(algorithm, ImportAlgorithm::Hmac { .. }) {
                Error::operation(message)
            } else {
                Error::data(message)
            });
        }
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn truncate_hmac_material(material: &mut Vec<u8>, length_bits: usize) {
    material.truncate(length_bits.div_ceil(8));
    let retained = length_bits % 8;
    if retained != 0 {
        let mask = u8::MAX << (8 - retained);
        *material
            .last_mut()
            .expect("a positive HMAC bit length retains one byte") &= mask;
    }
}

#[cfg(feature = "crypto")]
fn import_material(
    mut material: Vec<u8>,
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let (algorithm, usages) = match algorithm {
        ImportAlgorithm::Hmac { hash, length_bits } => {
            let length_bits = validate_hmac_import_length(material.len(), length_bits)?;
            truncate_hmac_material(&mut material, length_bits);
            (
                KeyAlgorithm::Hmac { hash, length_bits },
                validate_usages(usages, &[KeyUsage::Sign, KeyUsage::Verify])?,
            )
        }
        ImportAlgorithm::AesGcm => {
            let length_bits = material.len() * 8;
            validate_aes_length(length_bits)?;
            (
                KeyAlgorithm::AesGcm { length_bits },
                validate_usages(
                    usages,
                    &[
                        KeyUsage::Encrypt,
                        KeyUsage::Decrypt,
                        KeyUsage::WrapKey,
                        KeyUsage::UnwrapKey,
                    ],
                )?,
            )
        }
        ImportAlgorithm::Hkdf => {
            if extractable {
                return Err(Error::syntax("HKDF base keys must be non-extractable"));
            }
            (
                KeyAlgorithm::Hkdf,
                validate_usages(usages, &[KeyUsage::DeriveKey, KeyUsage::DeriveBits])?,
            )
        }
        ImportAlgorithm::Pbkdf2 => {
            if extractable {
                return Err(Error::syntax("PBKDF2 base keys must be non-extractable"));
            }
            (
                KeyAlgorithm::Pbkdf2,
                validate_usages(usages, &[KeyUsage::DeriveKey, KeyUsage::DeriveBits])?,
            )
        }
        ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519 => {
            return Err(Error::invalid_access(
                "asymmetric key material must use an asymmetric importer",
            ))
        }
    };
    if usages.is_empty() {
        return Err(Error::syntax(
            "secret keys must have at least one permitted usage",
        ));
    }
    Ok(CryptoKey {
        material,
        public_material: None,
        key_type: KeyType::Secret,
        algorithm,
        extractable,
        usages,
    })
}

pub fn import_raw_key(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (material, extractable, usages);
        if matches!(
            algorithm,
            ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519
        ) {
            Err(Error::asymmetric_feature_unavailable())
        } else {
            Err(Error::feature_unavailable())
        }
    }
    #[cfg(feature = "crypto")]
    {
        if matches!(
            algorithm,
            ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519
        ) {
            #[cfg(feature = "crypto-asymmetric")]
            return asymmetric::import_raw(material, algorithm, extractable, usages);
            #[cfg(not(feature = "crypto-asymmetric"))]
            return Err(Error::asymmetric_feature_unavailable());
        }
        preflight_secret_import(material.len(), algorithm, extractable, usages)?;
        import_material(material.to_vec(), algorithm, extractable, usages)
    }
}

pub fn import_spki_key(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (material, algorithm, extractable, usages);
        Err(Error::asymmetric_feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        #[cfg(feature = "crypto-asymmetric")]
        return asymmetric::import_spki(material, algorithm, extractable, usages);
        #[cfg(not(feature = "crypto-asymmetric"))]
        {
            let _ = (material, algorithm, extractable, usages);
            Err(Error::asymmetric_feature_unavailable())
        }
    }
}

pub fn import_pkcs8_key(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (material, algorithm, extractable, usages);
        Err(Error::asymmetric_feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        #[cfg(feature = "crypto-asymmetric")]
        return asymmetric::import_pkcs8(material, algorithm, extractable, usages);
        #[cfg(not(feature = "crypto-asymmetric"))]
        {
            let _ = (material, algorithm, extractable, usages);
            Err(Error::asymmetric_feature_unavailable())
        }
    }
}

pub fn import_jwk_key(
    jwk: &JsonWebKey,
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (jwk, extractable, usages);
        if matches!(
            algorithm,
            ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519
        ) {
            Err(Error::asymmetric_feature_unavailable())
        } else {
            Err(Error::feature_unavailable())
        }
    }
    #[cfg(feature = "crypto")]
    {
        if matches!(algorithm, ImportAlgorithm::Hkdf | ImportAlgorithm::Pbkdf2) {
            return Err(Error::unsupported("HKDF and PBKDF2 accept raw keys only"));
        }
        if matches!(
            algorithm,
            ImportAlgorithm::EcdsaP256 | ImportAlgorithm::Ed25519
        ) {
            #[cfg(feature = "crypto-asymmetric")]
            return asymmetric::import_jwk(jwk, algorithm, extractable, usages);
            #[cfg(not(feature = "crypto-asymmetric"))]
            return Err(Error::asymmetric_feature_unavailable());
        }
        let encoded = jwk
            .k
            .as_deref()
            .ok_or_else(|| Error::data("JWK k is required"))?;
        match algorithm {
            ImportAlgorithm::Hmac { length_bits, .. } => {
                if let Some(length_bits) = length_bits {
                    if length_bits > MAX_HMAC_KEY_BITS {
                        return Err(Error::operation(format!(
                            "HMAC key length exceeds the per-call limit of {MAX_HMAC_KEY_BITS} bits"
                        )));
                    }
                }
                if encoded.len() > MAX_HMAC_KEY_BITS.div_ceil(6) {
                    return Err(Error::operation(format!(
                        "HMAC key length exceeds the per-call limit of {MAX_HMAC_KEY_BITS} bits"
                    )));
                }
            }
            ImportAlgorithm::AesGcm if encoded.len() > 43 => {
                return Err(Error::data("AES-GCM JWK has an invalid key length"));
            }
            _ => {}
        }
        let expected = match algorithm {
            ImportAlgorithm::Hmac { hash, .. } => match hash {
                HashAlgorithm::Sha256 => "HS256",
                HashAlgorithm::Sha384 => "HS384",
                HashAlgorithm::Sha512 => "HS512",
            },
            ImportAlgorithm::AesGcm => match encoded.len() {
                22 => "A128GCM",
                32 => return Err(Error::unsupported("AES-GCM-192 is not supported by ring")),
                43 => "A256GCM",
                _ => return Err(Error::data("AES-GCM JWK has an invalid key length")),
            },
            _ => unreachable!(),
        };
        validate_jwk(jwk, expected, extractable, usages)?;
        let material = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| Error::data("JWK k is not unpadded base64url"))?;
        if URL_SAFE_NO_PAD.encode(&material) != encoded {
            return Err(Error::data("JWK k is not canonical unpadded base64url"));
        }
        preflight_secret_import(material.len(), algorithm, extractable, usages)?;
        import_material(material, algorithm, extractable, usages)
    }
}

pub fn export_key(format: KeyFormat, key: &CryptoKey) -> Result<ExportedKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (format, key);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        if !key.extractable {
            return Err(Error::invalid_access("key is not extractable"));
        }
        if matches!(
            key.algorithm,
            KeyAlgorithm::EcdsaP256 | KeyAlgorithm::Ed25519
        ) {
            #[cfg(feature = "crypto-asymmetric")]
            return asymmetric::export_key(format, key);
            #[cfg(not(feature = "crypto-asymmetric"))]
            return Err(Error::asymmetric_feature_unavailable());
        }
        if matches!(key.algorithm, KeyAlgorithm::Hkdf | KeyAlgorithm::Pbkdf2) {
            return Err(Error::unsupported(
                "derivation base keys cannot be exported",
            ));
        }
        Ok(match format {
            KeyFormat::Raw => ExportedKey::Raw(key.material.clone()),
            KeyFormat::Jwk => ExportedKey::Jwk(JsonWebKey {
                kty: "oct".into(),
                k: Some(URL_SAFE_NO_PAD.encode(&key.material)),
                crv: None,
                x: None,
                y: None,
                d: None,
                alg: Some(jwk_algorithm(&key.algorithm).into()),
                key_use: None,
                key_ops: Some(key.usages.iter().map(|usage| usage.name().into()).collect()),
                ext: Some(key.extractable),
            }),
            KeyFormat::Pkcs8 | KeyFormat::Spki => {
                return Err(Error::unsupported(
                    "secret keys cannot be exported as pkcs8 or spki",
                ))
            }
        })
    }
}

pub fn generate_key(
    algorithm: GenerateAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, extractable, usages);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        if usages.is_empty() {
            return Err(Error::syntax(
                "generated secret keys need at least one usage",
            ));
        }
        let (import, length_bits) = match algorithm {
            GenerateAlgorithm::Hmac { hash, length_bits } => {
                let length = length_bits.unwrap_or_else(|| hash.hmac_default_bits());
                validate_hmac_generated_length(length)?;
                (
                    ImportAlgorithm::Hmac {
                        hash,
                        length_bits: Some(length),
                    },
                    length,
                )
            }
            GenerateAlgorithm::AesGcm { length_bits } => {
                validate_aes_length(length_bits)?;
                (ImportAlgorithm::AesGcm, length_bits)
            }
            GenerateAlgorithm::EcdsaP256 | GenerateAlgorithm::Ed25519 => {
                #[cfg(not(feature = "crypto-asymmetric"))]
                return Err(Error::asymmetric_feature_unavailable());
                #[cfg(feature = "crypto-asymmetric")]
                return Err(Error::invalid_access(
                    "asymmetric algorithms return a key pair; use generate_key_pair",
                ));
            }
        };
        // Validate usage syntax before entropy or key-material allocation.
        match import {
            ImportAlgorithm::Hmac { .. } => {
                validate_usages(usages, &[KeyUsage::Sign, KeyUsage::Verify])?;
            }
            ImportAlgorithm::AesGcm => {
                validate_usages(
                    usages,
                    &[
                        KeyUsage::Encrypt,
                        KeyUsage::Decrypt,
                        KeyUsage::WrapKey,
                        KeyUsage::UnwrapKey,
                    ],
                )?;
            }
            _ => unreachable!(),
        }
        let mut material = vec![0; length_bits.div_ceil(8)];
        super::crypto::fill_random(&mut material)
            .map_err(|error| Error::operation(error.to_string()))?;
        import_material(material, import, extractable, usages)
    }
}

pub fn generate_key_pair(
    algorithm: GenerateAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKeyPair> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, extractable, usages);
        Err(Error::asymmetric_feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        #[cfg(feature = "crypto-asymmetric")]
        return asymmetric::generate_pair(algorithm, extractable, usages);
        #[cfg(not(feature = "crypto-asymmetric"))]
        {
            let _ = (algorithm, extractable, usages);
            Err(Error::asymmetric_feature_unavailable())
        }
    }
}

pub fn digest(hash: HashAlgorithm, data: &[u8]) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (hash, data);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        let algorithm = match hash {
            HashAlgorithm::Sha256 => &ring::digest::SHA256,
            HashAlgorithm::Sha384 => &ring::digest::SHA384,
            HashAlgorithm::Sha512 => &ring::digest::SHA512,
        };
        Ok(ring::digest::digest(algorithm, data).as_ref().to_vec())
    }
}

#[cfg(feature = "crypto")]
fn hmac_algorithm(hash: HashAlgorithm) -> ring::hmac::Algorithm {
    match hash {
        HashAlgorithm::Sha256 => ring::hmac::HMAC_SHA256,
        HashAlgorithm::Sha384 => ring::hmac::HMAC_SHA384,
        HashAlgorithm::Sha512 => ring::hmac::HMAC_SHA512,
    }
}

pub fn sign(key: &CryptoKey, data: &[u8]) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (key, data);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        require_usage(key, KeyUsage::Sign)?;
        let KeyAlgorithm::Hmac { hash, .. } = key.algorithm else {
            return Err(Error::invalid_access("sign requires an HMAC key"));
        };
        let key = ring::hmac::Key::new(hmac_algorithm(hash), &key.material);
        Ok(ring::hmac::sign(&key, data).as_ref().to_vec())
    }
}

pub fn sign_with_algorithm(
    algorithm: SignatureAlgorithm,
    key: &CryptoKey,
    data: &[u8],
) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, key, data);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        if algorithm == SignatureAlgorithm::Hmac {
            sign(key, data)
        } else {
            #[cfg(feature = "crypto-asymmetric")]
            return asymmetric::sign(algorithm, key, data);
            #[cfg(not(feature = "crypto-asymmetric"))]
            return Err(Error::asymmetric_feature_unavailable());
        }
    }
}

pub fn verify(key: &CryptoKey, signature: &[u8], data: &[u8]) -> Result<bool> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (key, signature, data);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        require_usage(key, KeyUsage::Verify)?;
        let KeyAlgorithm::Hmac { hash, .. } = key.algorithm else {
            return Err(Error::invalid_access("verify requires an HMAC key"));
        };
        let key = ring::hmac::Key::new(hmac_algorithm(hash), &key.material);
        Ok(ring::hmac::verify(&key, data, signature).is_ok())
    }
}

pub fn verify_with_algorithm(
    algorithm: SignatureAlgorithm,
    key: &CryptoKey,
    signature: &[u8],
    data: &[u8],
) -> Result<bool> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, key, signature, data);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        if algorithm == SignatureAlgorithm::Hmac {
            verify(key, signature, data)
        } else {
            #[cfg(feature = "crypto-asymmetric")]
            return asymmetric::verify(algorithm, key, signature, data);
            #[cfg(not(feature = "crypto-asymmetric"))]
            return Err(Error::asymmetric_feature_unavailable());
        }
    }
}

#[cfg(feature = "crypto")]
fn aes_key(key: &CryptoKey) -> Result<ring::aead::LessSafeKey> {
    let KeyAlgorithm::AesGcm { length_bits } = key.algorithm else {
        return Err(Error::invalid_access("AES-GCM requires an AES-GCM key"));
    };
    let algorithm = match length_bits {
        128 => &ring::aead::AES_128_GCM,
        256 => &ring::aead::AES_256_GCM,
        _ => return Err(Error::invalid_access("unsupported AES-GCM key length")),
    };
    ring::aead::UnboundKey::new(algorithm, &key.material)
        .map(ring::aead::LessSafeKey::new)
        .map_err(|_| Error::operation("AES-GCM key setup failed"))
}

#[cfg(feature = "crypto")]
fn aes_nonce(params: AesGcmParams<'_>) -> Result<ring::aead::Nonce> {
    if params.tag_length_bits != 128 {
        return Err(Error::operation("ring supports only 128-bit AES-GCM tags"));
    }
    ring::aead::Nonce::try_assume_unique_for_key(params.iv)
        .map_err(|_| Error::operation("ring supports only 96-bit AES-GCM IVs"))
}

pub fn encrypt(key: &CryptoKey, params: AesGcmParams<'_>, plaintext: &[u8]) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (key, params, plaintext);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        require_usage(key, KeyUsage::Encrypt)?;
        let key = aes_key(key)?;
        let nonce = aes_nonce(params)?;
        let mut output = plaintext.to_vec();
        key.seal_in_place_append_tag(
            nonce,
            ring::aead::Aad::from(params.additional_data),
            &mut output,
        )
        .map_err(|_| Error::operation("AES-GCM encryption failed"))?;
        Ok(output)
    }
}

pub fn decrypt(key: &CryptoKey, params: AesGcmParams<'_>, ciphertext: &[u8]) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (key, params, ciphertext);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        require_usage(key, KeyUsage::Decrypt)?;
        let key = aes_key(key)?;
        let nonce = aes_nonce(params)?;
        let mut output = ciphertext.to_vec();
        let plaintext = key
            .open_in_place(
                nonce,
                ring::aead::Aad::from(params.additional_data),
                &mut output,
            )
            .map_err(|_| Error::operation("AES-GCM authentication failed"))?;
        let length = plaintext.len();
        output.truncate(length);
        Ok(output)
    }
}

#[cfg(feature = "crypto")]
pub(crate) fn preflight_derive_access(
    algorithm: DeriveAlgorithm<'_>,
    base_key: &CryptoKey,
    usage: KeyUsage,
) -> Result<()> {
    require_usage(base_key, usage)?;
    match algorithm {
        DeriveAlgorithm::Hkdf { .. } if base_key.algorithm != KeyAlgorithm::Hkdf => {
            Err(Error::invalid_access("HKDF requires an HKDF base key"))
        }
        DeriveAlgorithm::Pbkdf2 { .. } if base_key.algorithm != KeyAlgorithm::Pbkdf2 => {
            Err(Error::invalid_access("PBKDF2 requires a PBKDF2 base key"))
        }
        _ => Ok(()),
    }
}

#[cfg(feature = "crypto")]
fn validate_pbkdf2_work(
    hash: HashAlgorithm,
    salt_len: usize,
    iterations: u32,
    length_bits: usize,
) -> Result<()> {
    if iterations == 0 {
        return Err(Error::operation("PBKDF2 iterations must be non-zero"));
    }
    if iterations > MAX_PBKDF2_ITERATIONS {
        return Err(Error::operation(format!(
            "PBKDF2 iterations exceed the per-call limit of {MAX_PBKDF2_ITERATIONS}"
        )));
    }
    if salt_len > MAX_PBKDF2_SALT_BYTES {
        return Err(Error::operation(format!(
            "PBKDF2 salt exceeds the per-call limit of {MAX_PBKDF2_SALT_BYTES} bytes"
        )));
    }

    let output_bytes = length_bits / 8;
    let output_blocks = output_bytes.div_ceil(hash.output_bytes());
    let salt_bytes = salt_len
        .checked_add(4)
        .ok_or_else(|| Error::operation("PBKDF2 salt length overflow"))?;
    let salt_blocks = salt_bytes.div_ceil(hash.compression_block_bytes()).max(1);
    let per_output_block = u64::from(iterations)
        .checked_add(
            u64::try_from(salt_blocks)
                .map_err(|_| Error::operation("PBKDF2 salt work overflow"))?,
        )
        .ok_or_else(|| Error::operation("PBKDF2 work overflow"))?;
    let work = u64::try_from(output_blocks)
        .ok()
        .and_then(|blocks| blocks.checked_mul(per_output_block))
        .ok_or_else(|| Error::operation("PBKDF2 work overflow"))?;
    let maximum_salt_blocks = (MAX_PBKDF2_SALT_BYTES + 4)
        .div_ceil(hash.compression_block_bytes())
        .max(1);
    let maximum = u64::from(MAX_PBKDF2_ITERATIONS)
        + u64::try_from(maximum_salt_blocks).expect("the fixed PBKDF2 salt ceiling fits in u64");
    if work > maximum {
        return Err(Error::operation(format!(
            "PBKDF2 combined work {work} exceeds the per-call limit of {maximum} for {}",
            hash.name()
        )));
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn compression_blocks(input_bytes: usize, block_bytes: usize, label: &str) -> Result<usize> {
    input_bytes
        .checked_add(block_bytes - 1)
        .map(|bytes| (bytes / block_bytes).max(1))
        .ok_or_else(|| Error::operation(format!("{label} length overflow")))
}

#[cfg(feature = "crypto")]
fn validate_hkdf_work(
    hash: HashAlgorithm,
    salt_len: usize,
    key_len: usize,
    info_len: usize,
    length_bits: usize,
) -> Result<()> {
    let block_bytes = hash.compression_block_bytes();
    let extract = compression_blocks(salt_len, block_bytes, "HKDF salt")?
        .checked_add(compression_blocks(
            key_len,
            block_bytes,
            "HKDF key material",
        )?)
        .ok_or_else(|| Error::operation("HKDF extract work overflow"))?;
    let expand_input = info_len
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(hash.output_bytes()))
        .ok_or_else(|| Error::operation("HKDF info length overflow"))?;
    let expand_per_output = compression_blocks(expand_input, block_bytes, "HKDF info")?;
    let output_blocks = (length_bits / 8).div_ceil(hash.output_bytes());
    let expand = output_blocks
        .checked_mul(expand_per_output)
        .ok_or_else(|| Error::operation("HKDF expand work overflow"))?;
    let work = extract
        .checked_add(expand)
        .ok_or_else(|| Error::operation("HKDF combined work overflow"))?;
    if work > MAX_HKDF_COMPRESSION_BLOCKS {
        return Err(Error::operation(format!(
            "HKDF combined work {work} exceeds the per-call limit of {MAX_HKDF_COMPRESSION_BLOCKS} compression blocks for {}",
            hash.name()
        )));
    }
    Ok(())
}

#[cfg(feature = "crypto")]
fn derive_material(
    algorithm: DeriveAlgorithm<'_>,
    base_key: &CryptoKey,
    usage: KeyUsage,
    length_bits: usize,
) -> Result<Vec<u8>> {
    preflight_derive_access(algorithm, base_key, usage)?;
    if !length_bits.is_multiple_of(8) {
        return Err(Error::operation("derived length must be a multiple of 8"));
    }
    if length_bits > MAX_DERIVED_BITS {
        return Err(Error::operation(format!(
            "derived length exceeds the per-call limit of {MAX_DERIVED_BITS} bits"
        )));
    }

    // Validate the algorithm, key type, and all algorithm-specific work
    // limits before allocating the caller-selected output buffer.
    match algorithm {
        DeriveAlgorithm::Hkdf { hash, salt, info } => {
            let maximum = 255 * hash.output_bits();
            if length_bits > maximum {
                return Err(Error::operation(format!(
                    "HKDF output exceeds 255 times the {hash_name} length",
                    hash_name = hash.name()
                )));
            }
            validate_hkdf_work(
                hash,
                salt.len(),
                base_key.material.len(),
                info.len(),
                length_bits,
            )?;
        }
        DeriveAlgorithm::Pbkdf2 {
            hash,
            salt,
            iterations,
        } => {
            validate_pbkdf2_work(hash, salt.len(), iterations, length_bits)?;
        }
    }

    let mut output = vec![0; length_bits / 8];
    match algorithm {
        // A zero-length HKDF result is empty, and computing it would still
        // hash all of `info` once inside ring. The work budget counts no
        // expand blocks for it, so it must never reach the backend.
        DeriveAlgorithm::Hkdf { .. } if output.is_empty() => {}
        DeriveAlgorithm::Hkdf { hash, salt, info } => {
            let algorithm = match hash {
                HashAlgorithm::Sha256 => ring::hkdf::HKDF_SHA256,
                HashAlgorithm::Sha384 => ring::hkdf::HKDF_SHA384,
                HashAlgorithm::Sha512 => ring::hkdf::HKDF_SHA512,
            };
            struct Length(usize);
            impl ring::hkdf::KeyType for Length {
                fn len(&self) -> usize {
                    self.0
                }
            }
            let salt = ring::hkdf::Salt::new(algorithm, salt);
            let prk = salt.extract(&base_key.material);
            let info = [info];
            prk.expand(&info, Length(output.len()))
                .and_then(|okm| okm.fill(&mut output))
                .map_err(|_| Error::operation("HKDF requested too much output"))?;
        }
        DeriveAlgorithm::Pbkdf2 {
            hash,
            salt,
            iterations,
        } => {
            let iterations = std::num::NonZeroU32::new(iterations)
                .expect("PBKDF2 iterations were validated before allocation");
            let algorithm = match hash {
                HashAlgorithm::Sha256 => ring::pbkdf2::PBKDF2_HMAC_SHA256,
                HashAlgorithm::Sha384 => ring::pbkdf2::PBKDF2_HMAC_SHA384,
                HashAlgorithm::Sha512 => ring::pbkdf2::PBKDF2_HMAC_SHA512,
            };
            ring::pbkdf2::derive(algorithm, iterations, salt, &base_key.material, &mut output);
        }
    }
    Ok(output)
}

pub fn derive_bits(
    algorithm: DeriveAlgorithm<'_>,
    base_key: &CryptoKey,
    length_bits: usize,
) -> Result<Vec<u8>> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, base_key, length_bits);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    derive_material(algorithm, base_key, KeyUsage::DeriveBits, length_bits)
}

pub fn derive_key(
    algorithm: DeriveAlgorithm<'_>,
    base_key: &CryptoKey,
    derived: DerivedKeyAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (algorithm, base_key, derived, extractable, usages);
        Err(Error::feature_unavailable())
    }
    #[cfg(feature = "crypto")]
    {
        if usages.is_empty() {
            return Err(Error::syntax("derived secret keys need at least one usage"));
        }
        let (import, length_bits) = match derived {
            DerivedKeyAlgorithm::Hmac { hash, length_bits } => {
                let length_bits = length_bits.unwrap_or_else(|| hash.hmac_default_bits());
                validate_hmac_generated_length(length_bits)?;
                (
                    ImportAlgorithm::Hmac {
                        hash,
                        length_bits: Some(length_bits),
                    },
                    length_bits,
                )
            }
            DerivedKeyAlgorithm::AesGcm { length_bits } => {
                validate_aes_length(length_bits)?;
                (ImportAlgorithm::AesGcm, length_bits)
            }
        };
        // Refuse invalid derived-key usages before performing the KDF.
        match import {
            ImportAlgorithm::Hmac { .. } => {
                validate_usages(usages, &[KeyUsage::Sign, KeyUsage::Verify])?;
            }
            ImportAlgorithm::AesGcm => {
                validate_usages(
                    usages,
                    &[
                        KeyUsage::Encrypt,
                        KeyUsage::Decrypt,
                        KeyUsage::WrapKey,
                        KeyUsage::UnwrapKey,
                    ],
                )?;
            }
            _ => unreachable!(),
        }
        let material = derive_material(algorithm, base_key, KeyUsage::DeriveKey, length_bits)?;
        import_material(material, import, extractable, usages)
    }
}

#[cfg(test)]
#[cfg(feature = "crypto")]
mod tests {
    use super::*;

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(text, 16).unwrap()
            })
            .collect()
    }

    #[test]
    fn fips_180_4_digests() {
        assert_eq!(
            digest(HashAlgorithm::Sha256, b"abc").unwrap(),
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(digest(HashAlgorithm::Sha384, b"abc").unwrap(), hex("cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"));
        assert_eq!(digest(HashAlgorithm::Sha512, b"abc").unwrap(), hex("ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"));
    }

    #[test]
    fn rfc_4231_hmac_sha256_case_one() {
        let key = import_raw_key(
            &[0x0b; 20],
            ImportAlgorithm::Hmac {
                hash: HashAlgorithm::Sha256,
                length_bits: None,
            },
            false,
            &[KeyUsage::Sign, KeyUsage::Verify],
        )
        .unwrap();
        let signature = sign(&key, b"Hi There").unwrap();
        assert_eq!(
            signature,
            hex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
        );
        assert!(verify(&key, &signature, b"Hi There").unwrap());
        assert!(!verify(&key, &signature, b"not it").unwrap());
    }

    #[test]
    fn rfc_4231_hmac_sha384_and_sha512_case_one() {
        for (hash, expected) in [
            (
                HashAlgorithm::Sha384,
                "afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59cfaea9ea9076ede7f4af152e8b2fa9cb6",
            ),
            (
                HashAlgorithm::Sha512,
                "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
            ),
        ] {
            let key = import_raw_key(
                &[0x0b; 20],
                ImportAlgorithm::Hmac {
                    hash,
                    length_bits: None,
                },
                false,
                &[KeyUsage::Sign],
            )
            .unwrap();
            assert_eq!(sign(&key, b"Hi There").unwrap(), hex(expected));
        }
    }

    #[test]
    fn non_octet_hmac_import_masks_and_signs_with_retained_bits() {
        let key = import_raw_key(
            &[0xff; 20],
            ImportAlgorithm::Hmac {
                hash: HashAlgorithm::Sha256,
                length_bits: Some(155),
            },
            true,
            &[KeyUsage::Sign],
        )
        .unwrap();
        assert_eq!(
            key.algorithm(),
            &KeyAlgorithm::Hmac {
                hash: HashAlgorithm::Sha256,
                length_bits: 155,
            }
        );
        let mut expected_key = vec![0xff; 20];
        expected_key[19] = 0xe0;
        assert_eq!(
            export_key(KeyFormat::Raw, &key).unwrap(),
            ExportedKey::Raw(expected_key)
        );
        assert_eq!(
            sign(&key, b"ibex").unwrap(),
            hex("09dc61ba3ab858026005afe0e64a7e864f4be1256bedefce0e16d78dd5a571ae")
        );
        assert_eq!(
            import_raw_key(
                &[0xff; 20],
                ImportAlgorithm::Hmac {
                    hash: HashAlgorithm::Sha256,
                    length_bits: Some(152),
                },
                true,
                &[KeyUsage::Sign],
            )
            .unwrap_err()
            .name,
            ErrorName::DataError
        );
    }

    #[test]
    fn nist_gcm_empty_plaintext_case() {
        let key = import_raw_key(
            &[0; 16],
            ImportAlgorithm::AesGcm,
            false,
            &[KeyUsage::Encrypt, KeyUsage::Decrypt],
        )
        .unwrap();
        let params = AesGcmParams {
            iv: &[0; 12],
            additional_data: &[],
            tag_length_bits: 128,
        };
        let encrypted = encrypt(&key, params, &[]).unwrap();
        assert_eq!(encrypted, hex("58e2fccefa7e3061367f1d57a4e7455a"));
        assert_eq!(decrypt(&key, params, &encrypted).unwrap(), b"");

        let key = import_raw_key(
            &[0; 32],
            ImportAlgorithm::AesGcm,
            false,
            &[KeyUsage::Encrypt],
        )
        .unwrap();
        assert_eq!(
            encrypt(&key, params, &[]).unwrap(),
            hex("530f8afbc74536b9a963b4f1c4cb738b")
        );
        assert_eq!(
            import_raw_key(&[0; 24], ImportAlgorithm::AesGcm, false, &[])
                .unwrap_err()
                .name,
            ErrorName::NotSupportedError
        );
    }

    #[test]
    fn rfc_5869_hkdf_sha256_case_one() {
        let key = import_raw_key(
            &[0x0b; 22],
            ImportAlgorithm::Hkdf,
            false,
            &[KeyUsage::DeriveBits],
        )
        .unwrap();
        let output = derive_bits(
            DeriveAlgorithm::Hkdf {
                hash: HashAlgorithm::Sha256,
                salt: &hex("000102030405060708090a0b0c"),
                info: &hex("f0f1f2f3f4f5f6f7f8f9"),
            },
            &key,
            42 * 8,
        )
        .unwrap();
        assert_eq!(output, hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"));
    }

    #[test]
    fn zero_length_hkdf_never_reaches_the_backend() {
        let key = import_raw_key(
            b"key",
            ImportAlgorithm::Hkdf,
            false,
            &[KeyUsage::DeriveBits],
        )
        .unwrap();
        // Over the budget at one output block, so a zero-length request that
        // still invoked ring would hash all of it uncounted.
        let info = vec![0x5a; MAX_HKDF_COMPRESSION_BLOCKS * 64];
        let hkdf = || DeriveAlgorithm::Hkdf {
            hash: HashAlgorithm::Sha256,
            salt: b"",
            info: &info,
        };
        assert_eq!(derive_bits(hkdf(), &key, 0).unwrap(), Vec::<u8>::new());
        assert_eq!(
            derive_bits(hkdf(), &key, 256).unwrap_err().name,
            ErrorName::OperationError
        );
    }

    #[test]
    fn hkdf_combined_work_is_bounded_for_bits_and_keys() {
        const WORK_BUDGET: usize = MAX_HKDF_COMPRESSION_BLOCKS;
        let key = import_raw_key(
            b"key",
            ImportAlgorithm::Hkdf,
            false,
            &[KeyUsage::DeriveBits, KeyUsage::DeriveKey],
        )
        .unwrap();

        // Empty salt and the three-byte key each account for one extract
        // compression block. At 255 output blocks, 257 expand blocks per
        // output block is the greatest whole-block cost below the budget.
        let expand_blocks = (WORK_BUDGET - 2) / 255;
        for hash in [HashAlgorithm::Sha256, HashAlgorithm::Sha512] {
            let maximum_info =
                expand_blocks * hash.compression_block_bytes() - 1 - hash.output_bytes();
            let info = vec![0x5a; maximum_info];
            derive_bits(
                DeriveAlgorithm::Hkdf {
                    hash,
                    salt: b"",
                    info: &info,
                },
                &key,
                255 * hash.output_bits(),
            )
            .unwrap();

            let info = vec![0x5a; maximum_info + 1];
            assert_eq!(
                derive_bits(
                    DeriveAlgorithm::Hkdf {
                        hash,
                        salt: b"",
                        info: &info,
                    },
                    &key,
                    255 * hash.output_bits(),
                )
                .unwrap_err()
                .name,
                ErrorName::OperationError
            );
        }

        // deriveKey requests only one SHA-256 output block, so crossing the
        // same work budget takes an info value of about 4 MiB.
        let info = vec![0x5a; (WORK_BUDGET - 2) * 64 - 32];
        assert_eq!(
            derive_key(
                DeriveAlgorithm::Hkdf {
                    hash: HashAlgorithm::Sha256,
                    salt: b"",
                    info: &info,
                },
                &key,
                DerivedKeyAlgorithm::Hmac {
                    hash: HashAlgorithm::Sha256,
                    length_bits: Some(256),
                },
                false,
                &[KeyUsage::Sign],
            )
            .unwrap_err()
            .name,
            ErrorName::OperationError
        );
    }

    #[test]
    fn pbkdf2_hmac_sha256_vector() {
        let key = import_raw_key(
            b"password",
            ImportAlgorithm::Pbkdf2,
            false,
            &[KeyUsage::DeriveBits],
        )
        .unwrap();
        let output = derive_bits(
            DeriveAlgorithm::Pbkdf2 {
                hash: HashAlgorithm::Sha256,
                salt: b"salt",
                iterations: 1,
            },
            &key,
            256,
        )
        .unwrap();
        assert_eq!(
            output,
            hex("120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
        );
    }

    #[test]
    fn caller_sized_work_is_bounded_before_backend_calls() {
        assert_eq!(
            generate_key(
                GenerateAlgorithm::Hmac {
                    hash: HashAlgorithm::Sha256,
                    length_bits: Some(MAX_HMAC_KEY_BITS + 8),
                },
                true,
                &[KeyUsage::Sign],
            )
            .unwrap_err()
            .name,
            ErrorName::OperationError
        );

        let hkdf = import_raw_key(
            b"key",
            ImportAlgorithm::Hkdf,
            false,
            &[KeyUsage::DeriveBits, KeyUsage::DeriveKey],
        )
        .unwrap();
        let hkdf_params = DeriveAlgorithm::Hkdf {
            hash: HashAlgorithm::Sha256,
            salt: b"",
            info: b"",
        };
        assert_eq!(
            derive_bits(hkdf_params, &hkdf, 255 * 256 + 8)
                .unwrap_err()
                .name,
            ErrorName::OperationError
        );
        assert_eq!(
            derive_key(
                hkdf_params,
                &hkdf,
                DerivedKeyAlgorithm::Hmac {
                    hash: HashAlgorithm::Sha256,
                    length_bits: Some(MAX_HMAC_KEY_BITS + 8),
                },
                true,
                &[KeyUsage::Sign],
            )
            .unwrap_err()
            .name,
            ErrorName::OperationError
        );

        let pbkdf2 = import_raw_key(
            b"password",
            ImportAlgorithm::Pbkdf2,
            false,
            &[KeyUsage::DeriveBits],
        )
        .unwrap();
        for iterations in [0, MAX_PBKDF2_ITERATIONS + 1] {
            assert_eq!(
                derive_bits(
                    DeriveAlgorithm::Pbkdf2 {
                        hash: HashAlgorithm::Sha256,
                        salt: b"salt",
                        iterations,
                    },
                    &pbkdf2,
                    8,
                )
                .unwrap_err()
                .name,
                ErrorName::OperationError
            );
        }
        assert_eq!(
            derive_bits(
                DeriveAlgorithm::Pbkdf2 {
                    hash: HashAlgorithm::Sha256,
                    salt: b"salt",
                    iterations: 1,
                },
                &pbkdf2,
                MAX_DERIVED_BITS + 8,
            )
            .unwrap_err()
            .name,
            ErrorName::OperationError
        );

        for hash in [
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha384,
            HashAlgorithm::Sha512,
        ] {
            validate_pbkdf2_work(
                hash,
                MAX_PBKDF2_SALT_BYTES,
                MAX_PBKDF2_ITERATIONS,
                hash.output_bits(),
            )
            .unwrap();
            assert_eq!(
                validate_pbkdf2_work(
                    hash,
                    MAX_PBKDF2_SALT_BYTES,
                    MAX_PBKDF2_ITERATIONS,
                    hash.output_bits() + 8,
                )
                .unwrap_err()
                .name,
                ErrorName::OperationError
            );
        }

        assert_eq!(
            derive_key(
                hkdf_params,
                &hkdf,
                DerivedKeyAlgorithm::Hmac {
                    hash: HashAlgorithm::Sha256,
                    length_bits: Some(17),
                },
                true,
                &[KeyUsage::Sign],
            )
            .unwrap_err()
            .name,
            ErrorName::OperationError
        );
    }

    #[test]
    fn jwk_round_trip_and_nonextractable_rules() {
        let key = import_raw_key(
            &[7; 32],
            ImportAlgorithm::Hmac {
                hash: HashAlgorithm::Sha256,
                length_bits: None,
            },
            true,
            &[KeyUsage::Sign],
        )
        .unwrap();
        let ExportedKey::Jwk(jwk) = export_key(KeyFormat::Jwk, &key).unwrap() else {
            panic!()
        };
        let again = import_jwk_key(
            &jwk,
            ImportAlgorithm::Hmac {
                hash: HashAlgorithm::Sha256,
                length_bits: None,
            },
            true,
            &[KeyUsage::Sign],
        )
        .unwrap();
        assert_eq!(
            export_key(KeyFormat::Raw, &again).unwrap(),
            ExportedKey::Raw(vec![7; 32])
        );
        let base = import_raw_key(
            b"password",
            ImportAlgorithm::Pbkdf2,
            false,
            &[KeyUsage::DeriveBits],
        )
        .unwrap();
        assert_eq!(
            export_key(KeyFormat::Raw, &base).unwrap_err().name,
            ErrorName::InvalidAccessError
        );
        assert_eq!(
            import_raw_key(
                b"password",
                ImportAlgorithm::Pbkdf2,
                true,
                &[KeyUsage::DeriveBits]
            )
            .unwrap_err()
            .name,
            ErrorName::SyntaxError
        );
    }
}
