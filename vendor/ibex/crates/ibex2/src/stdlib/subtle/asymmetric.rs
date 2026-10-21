//! ECDSA P-256 and Ed25519 over the vendored `ring` backend.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::signature::KeyPair as _;

use super::{
    der, preflight_jwk_fields, preflight_jwk_key_ops, validate_jwk_key_ops, validate_usages,
    validate_usages_borrowed, CryptoKey, CryptoKeyPair, Error, ExportedKey, GenerateAlgorithm,
    HashAlgorithm, ImportAlgorithm, JsonWebKey, JwkFieldRefs, KeyAlgorithm, KeyFormat, KeyType,
    KeyUsage, Result,
};

const MAX_P256_PKCS8_BYTES: usize = 160;
const MAX_ED25519_PKCS8_BYTES: usize = 96;

fn public_key(
    material: Vec<u8>,
    algorithm: KeyAlgorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
) -> CryptoKey {
    CryptoKey {
        material,
        public_material: None,
        key_type: KeyType::Public,
        algorithm,
        extractable,
        usages,
    }
}

fn private_key(
    private: Vec<u8>,
    public: Vec<u8>,
    algorithm: KeyAlgorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
) -> CryptoKey {
    CryptoKey {
        material: private,
        public_material: Some(public),
        key_type: KeyType::Private,
        algorithm,
        extractable,
        usages,
    }
}

fn validate_public(algorithm: KeyAlgorithm, public: &[u8]) -> Result<()> {
    match algorithm {
        KeyAlgorithm::EcdsaP256 => {
            if public.len() != 65 || public[0] != 0x04 {
                return Err(Error::data(
                    "P-256 public keys must be uncompressed 65-byte points",
                ));
            }
            let rng = ring::rand::SystemRandom::new();
            let private =
                ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &rng)
                    .map_err(|_| Error::operation("P-256 validation setup failed"))?;
            let public =
                ring::agreement::UnparsedPublicKey::new(&ring::agreement::ECDH_P256, public);
            ring::agreement::agree_ephemeral(private, &public, |_| ())
                .map_err(|_| Error::data("P-256 public point is not on the curve"))
        }
        KeyAlgorithm::Ed25519 => {
            if public.len() != 32 {
                Err(Error::data("Ed25519 public keys must be 32 bytes"))
            } else {
                Ok(())
            }
        }
        _ => Err(Error::invalid_access("key is not asymmetric")),
    }
}

fn validate_private(algorithm: KeyAlgorithm, private: &[u8], public: &[u8]) -> Result<()> {
    match algorithm {
        KeyAlgorithm::EcdsaP256 => ring::signature::EcdsaKeyPair::from_private_key_and_public_key(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            private,
            public,
            &ring::rand::SystemRandom::new(),
        )
        .map(|_| ())
        .map_err(|_| Error::data("P-256 private and public key components are invalid")),
        KeyAlgorithm::Ed25519 => {
            ring::signature::Ed25519KeyPair::from_seed_and_public_key(private, public)
                .map(|_| ())
                .map_err(|_| Error::data("Ed25519 private and public key components are invalid"))
        }
        _ => Err(Error::invalid_access("key is not asymmetric")),
    }
}

fn public_for_private(key: &CryptoKey) -> Result<&[u8]> {
    key.public_material
        .as_deref()
        .ok_or_else(|| Error::invalid_access("private key has no public component"))
}

fn decode_component(jwk: &JsonWebKey, name: &str, value: Option<&str>) -> Result<Vec<u8>> {
    let encoded = value.ok_or_else(|| Error::data(format!("JWK {name} is required")))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| Error::data(format!("JWK {name} is not unpadded base64url")))?;
    if URL_SAFE_NO_PAD.encode(&decoded) != encoded {
        return Err(Error::data(format!(
            "JWK {name} is not canonical unpadded base64url"
        )));
    }
    let _ = jwk;
    Ok(decoded)
}

fn validate_jwk_metadata(
    jwk: &JsonWebKey,
    algorithm: KeyAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<()> {
    let (kty, crv, algorithms) = match algorithm {
        KeyAlgorithm::EcdsaP256 => ("EC", "P-256", &["ES256"][..]),
        KeyAlgorithm::Ed25519 => ("OKP", "Ed25519", &["Ed25519", "EdDSA"][..]),
        _ => return Err(Error::invalid_access("key is not asymmetric")),
    };
    if jwk.kty != kty || jwk.crv.as_deref() != Some(crv) {
        return Err(Error::data(
            "JWK kty or crv does not match the requested algorithm",
        ));
    }
    if let Some(alg) = &jwk.alg {
        if !algorithms.contains(&alg.as_str()) {
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
    if jwk.key_use.as_deref().is_some_and(|value| value != "sig") {
        return Err(Error::data("JWK use must be sig"));
    }
    validate_jwk_key_ops(jwk.key_ops.as_deref(), usages)?;
    Ok(())
}

fn preflight_component_usages(private: bool, usages: &[KeyUsage]) -> Result<()> {
    let allowed = if private {
        &[KeyUsage::Sign][..]
    } else {
        &[KeyUsage::Verify][..]
    };
    validate_usages_borrowed(usages, allowed)?;
    if private && usages.is_empty() {
        return Err(Error::syntax("private keys must have at least one usage"));
    }
    Ok(())
}

fn preflight_der_length(format: &str, algorithm: ImportAlgorithm, len: usize) -> Result<()> {
    let valid = match (format, algorithm) {
        ("spki", ImportAlgorithm::EcdsaP256) => len == 91,
        ("spki", ImportAlgorithm::Ed25519) => len == 44,
        ("pkcs8", ImportAlgorithm::EcdsaP256) => len <= MAX_P256_PKCS8_BYTES,
        ("pkcs8", ImportAlgorithm::Ed25519) => len <= MAX_ED25519_PKCS8_BYTES,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::data(format!(
            "{format} key data exceeds the requested algorithm's size"
        )))
    }
}

fn import_components(
    algorithm: KeyAlgorithm,
    private: Option<Vec<u8>>,
    public: Vec<u8>,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let (key_type, allowed) = if private.is_some() {
        (KeyType::Private, &[KeyUsage::Sign][..])
    } else {
        (KeyType::Public, &[KeyUsage::Verify][..])
    };
    let usages = validate_usages(usages, allowed)?;
    if key_type == KeyType::Private && usages.is_empty() {
        return Err(Error::syntax("private keys must have at least one usage"));
    }
    validate_public(algorithm.clone(), &public)?;
    if let Some(private) = private {
        validate_private(algorithm.clone(), &private, &public)?;
        Ok(private_key(private, public, algorithm, extractable, usages))
    } else {
        Ok(public_key(public, algorithm, extractable, usages))
    }
}

pub(super) fn import_raw(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let algorithm = match algorithm {
        ImportAlgorithm::EcdsaP256 => KeyAlgorithm::EcdsaP256,
        ImportAlgorithm::Ed25519 => KeyAlgorithm::Ed25519,
        _ => {
            return Err(Error::invalid_access(
                "raw asymmetric import needs an asymmetric algorithm",
            ))
        }
    };
    preflight_component_usages(false, usages)?;
    validate_public(algorithm.clone(), material)?;
    import_components(algorithm, None, material.to_vec(), extractable, usages)
}

pub(super) fn import_spki(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let expected = match algorithm {
        ImportAlgorithm::EcdsaP256 => (der::Algorithm::EcdsaP256, KeyAlgorithm::EcdsaP256),
        ImportAlgorithm::Ed25519 => (der::Algorithm::Ed25519, KeyAlgorithm::Ed25519),
        _ => return Err(Error::unsupported("spki requires an asymmetric algorithm")),
    };
    preflight_component_usages(false, usages)?;
    preflight_der_length("spki", algorithm, material.len())?;
    let (parsed, public) = der::parse_spki(material)?;
    if parsed != expected.0 {
        return Err(Error::data(
            "SPKI algorithm does not match the requested algorithm",
        ));
    }
    import_components(expected.1, None, public, extractable, usages)
}

pub(super) fn import_pkcs8(
    material: &[u8],
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let expected = match algorithm {
        ImportAlgorithm::EcdsaP256 => (der::Algorithm::EcdsaP256, KeyAlgorithm::EcdsaP256),
        ImportAlgorithm::Ed25519 => (der::Algorithm::Ed25519, KeyAlgorithm::Ed25519),
        _ => return Err(Error::unsupported("pkcs8 requires an asymmetric algorithm")),
    };
    preflight_component_usages(true, usages)?;
    preflight_der_length("pkcs8", algorithm, material.len())?;
    let parsed = der::parse_pkcs8(material)?;
    if parsed.algorithm != expected.0 {
        return Err(Error::data(
            "PKCS#8 algorithm does not match the requested algorithm",
        ));
    }
    let public = match (expected.1.clone(), parsed.public) {
        (KeyAlgorithm::Ed25519, None) => {
            ring::signature::Ed25519KeyPair::from_seed_unchecked(&parsed.private)
                .map_err(|_| Error::data("invalid Ed25519 seed"))?
                .public_key()
                .as_ref()
                .to_vec()
        }
        (KeyAlgorithm::EcdsaP256, None) => {
            return Err(Error::unsupported(
                "P-256 PKCS#8 public key must be present in the private-key structure",
            ))
        }
        (_, Some(public)) => public,
        _ => return Err(Error::data("private key has no public component")),
    };
    import_components(
        expected.1,
        Some(parsed.private),
        public,
        extractable,
        usages,
    )
}

pub(super) fn import_jwk(
    jwk: &JsonWebKey,
    algorithm: ImportAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKey> {
    let import_algorithm = algorithm;
    let algorithm = match algorithm {
        ImportAlgorithm::EcdsaP256 => KeyAlgorithm::EcdsaP256,
        ImportAlgorithm::Ed25519 => KeyAlgorithm::Ed25519,
        _ => return Err(Error::invalid_access("JWK is not asymmetric")),
    };
    preflight_jwk_fields(
        import_algorithm,
        JwkFieldRefs {
            kty: &jwk.kty,
            k: jwk.k.as_deref(),
            crv: jwk.crv.as_deref(),
            x: jwk.x.as_deref(),
            y: jwk.y.as_deref(),
            d: jwk.d.as_deref(),
            alg: jwk.alg.as_deref(),
            key_use: jwk.key_use.as_deref(),
        },
    )?;
    let key_ops = jwk
        .key_ops
        .as_ref()
        .map(|ops| ops.iter().map(String::as_str).collect::<Vec<_>>());
    preflight_jwk_key_ops(key_ops.as_deref())?;
    let key_type = if jwk.d.is_some() {
        KeyType::Private
    } else {
        KeyType::Public
    };
    preflight_component_usages(key_type == KeyType::Private, usages)?;
    validate_jwk_metadata(jwk, algorithm.clone(), extractable, usages)?;
    let private = jwk
        .d
        .as_deref()
        .map(|value| decode_component(jwk, "d", Some(value)))
        .transpose()?;
    let x = decode_component(jwk, "x", jwk.x.as_deref())?;
    let public = match algorithm {
        KeyAlgorithm::EcdsaP256 => {
            let y = decode_component(jwk, "y", jwk.y.as_deref())?;
            if x.len() != 32 || y.len() != 32 {
                return Err(Error::data("P-256 JWK coordinates must be 32 bytes"));
            }
            [&[0x04], x.as_slice(), y.as_slice()].concat()
        }
        KeyAlgorithm::Ed25519 => {
            if x.len() != 32 || jwk.y.is_some() {
                return Err(Error::data(
                    "Ed25519 JWK x must be 32 bytes and y must be absent",
                ));
            }
            x
        }
        _ => unreachable!(),
    };
    if private.as_ref().is_some_and(|private| private.len() != 32) {
        return Err(Error::data("private JWK component must be 32 bytes"));
    }
    import_components(algorithm, private, public, extractable, usages)
}

pub(super) fn export_key(format: KeyFormat, key: &CryptoKey) -> Result<ExportedKey> {
    let (algorithm, crv, kty, alg) = match key.algorithm {
        KeyAlgorithm::EcdsaP256 => (der::Algorithm::EcdsaP256, "P-256", "EC", "ES256"),
        KeyAlgorithm::Ed25519 => (der::Algorithm::Ed25519, "Ed25519", "OKP", "Ed25519"),
        _ => return Err(Error::invalid_access("key is not asymmetric")),
    };
    let public = if key.key_type == KeyType::Private {
        public_for_private(key)?
    } else {
        &key.material
    };
    match format {
        KeyFormat::Raw if key.key_type == KeyType::Public => Ok(ExportedKey::Raw(public.to_vec())),
        KeyFormat::Spki if key.key_type == KeyType::Public => {
            Ok(ExportedKey::Spki(der::encode_spki(algorithm, public)))
        }
        KeyFormat::Pkcs8 if key.key_type == KeyType::Private => Ok(ExportedKey::Pkcs8(
            der::encode_pkcs8(algorithm, &key.material, public),
        )),
        KeyFormat::Jwk => {
            let (x, y) = match key.algorithm {
                KeyAlgorithm::EcdsaP256 => (
                    URL_SAFE_NO_PAD.encode(&public[1..33]),
                    Some(URL_SAFE_NO_PAD.encode(&public[33..65])),
                ),
                KeyAlgorithm::Ed25519 => (URL_SAFE_NO_PAD.encode(public), None),
                _ => unreachable!(),
            };
            Ok(ExportedKey::Jwk(JsonWebKey {
                kty: kty.into(),
                k: None,
                crv: Some(crv.into()),
                x: Some(x),
                y,
                d: (key.key_type == KeyType::Private)
                    .then(|| URL_SAFE_NO_PAD.encode(&key.material)),
                alg: Some(alg.into()),
                key_use: None,
                key_ops: Some(key.usages.iter().map(|usage| usage.name().into()).collect()),
                ext: Some(key.extractable),
            }))
        }
        KeyFormat::Raw => Err(Error::invalid_access("raw export requires a public key")),
        KeyFormat::Spki => Err(Error::invalid_access("spki export requires a public key")),
        KeyFormat::Pkcs8 => Err(Error::invalid_access("pkcs8 export requires a private key")),
    }
}

pub(super) fn generate_pair(
    algorithm: GenerateAlgorithm,
    extractable: bool,
    usages: &[KeyUsage],
) -> Result<CryptoKeyPair> {
    let usages = validate_usages(usages, &[KeyUsage::Sign, KeyUsage::Verify])?;
    if !usages.contains(&KeyUsage::Sign) {
        return Err(Error::syntax(
            "generated private keys need at least one usage",
        ));
    }
    let private_usages = usages
        .iter()
        .copied()
        .filter(|usage| *usage == KeyUsage::Sign)
        .collect();
    let public_usages = usages
        .iter()
        .copied()
        .filter(|usage| *usage == KeyUsage::Verify)
        .collect();
    let rng = ring::rand::SystemRandom::new();
    let (algorithm, private, public) = match algorithm {
        GenerateAlgorithm::EcdsaP256 => {
            let document = ring::signature::EcdsaKeyPair::generate_pkcs8(
                &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                &rng,
            )
            .map_err(|_| Error::operation("P-256 key generation failed"))?;
            let parsed = der::parse_pkcs8(document.as_ref())?;
            (
                KeyAlgorithm::EcdsaP256,
                parsed.private,
                parsed
                    .public
                    .expect("ring P-256 PKCS#8 includes public key"),
            )
        }
        GenerateAlgorithm::Ed25519 => {
            let document = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|_| Error::operation("Ed25519 key generation failed"))?;
            let parsed = der::parse_pkcs8(document.as_ref())?;
            (
                KeyAlgorithm::Ed25519,
                parsed.private,
                parsed
                    .public
                    .expect("ring Ed25519 PKCS#8 v2 includes public key"),
            )
        }
        _ => {
            return Err(Error::invalid_access(
                "algorithm does not generate a key pair",
            ))
        }
    };
    Ok(CryptoKeyPair {
        public_key: public_key(public.clone(), algorithm.clone(), true, public_usages),
        private_key: private_key(private, public, algorithm, extractable, private_usages),
    })
}

fn ecdsa_signing_algorithm(
    hash: HashAlgorithm,
) -> Result<&'static ring::signature::EcdsaSigningAlgorithm> {
    match hash {
        HashAlgorithm::Sha256 => Ok(&ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING),
        HashAlgorithm::Sha384 | HashAlgorithm::Sha512 => Err(Error::unsupported(
            "P-256 signing supports only SHA-256 with the unmodified ring backend",
        )),
    }
}

fn ecdsa_verification_algorithm(
    hash: HashAlgorithm,
) -> Result<&'static ring::signature::EcdsaVerificationAlgorithm> {
    match hash {
        HashAlgorithm::Sha256 => Ok(&ring::signature::ECDSA_P256_SHA256_FIXED),
        HashAlgorithm::Sha384 | HashAlgorithm::Sha512 => Err(Error::unsupported(
            "P-256 verification supports only SHA-256 with the unmodified ring backend",
        )),
    }
}

pub(super) fn sign(
    algorithm: super::SignatureAlgorithm,
    key: &CryptoKey,
    data: &[u8],
) -> Result<Vec<u8>> {
    super::require_usage(key, KeyUsage::Sign)?;
    if key.key_type != KeyType::Private {
        return Err(Error::invalid_access("sign requires a private key"));
    }
    match (algorithm, &key.algorithm) {
        (super::SignatureAlgorithm::Ecdsa { hash }, KeyAlgorithm::EcdsaP256) => {
            let pair = ring::signature::EcdsaKeyPair::from_private_key_and_public_key(
                ecdsa_signing_algorithm(hash)?,
                &key.material,
                public_for_private(key)?,
                &ring::rand::SystemRandom::new(),
            )
            .map_err(|_| Error::operation("P-256 key setup failed"))?;
            pair.sign(&ring::rand::SystemRandom::new(), data)
                .map(|signature| signature.as_ref().to_vec())
                .map_err(|_| Error::operation("ECDSA signing failed"))
        }
        (super::SignatureAlgorithm::Ed25519, KeyAlgorithm::Ed25519) => {
            let pair = ring::signature::Ed25519KeyPair::from_seed_and_public_key(
                &key.material,
                public_for_private(key)?,
            )
            .map_err(|_| Error::operation("Ed25519 key setup failed"))?;
            Ok(pair.sign(data).as_ref().to_vec())
        }
        _ => Err(Error::invalid_access(
            "signature algorithm does not match the key",
        )),
    }
}

pub(super) fn verify(
    algorithm: super::SignatureAlgorithm,
    key: &CryptoKey,
    signature: &[u8],
    data: &[u8],
) -> Result<bool> {
    super::require_usage(key, KeyUsage::Verify)?;
    if key.key_type != KeyType::Public {
        return Err(Error::invalid_access("verify requires a public key"));
    }
    let result = match (algorithm, &key.algorithm) {
        (super::SignatureAlgorithm::Ecdsa { hash }, KeyAlgorithm::EcdsaP256) => {
            ring::signature::UnparsedPublicKey::new(
                ecdsa_verification_algorithm(hash)?,
                &key.material,
            )
            .verify(data, signature)
        }
        (super::SignatureAlgorithm::Ed25519, KeyAlgorithm::Ed25519) => {
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &key.material)
                .verify(data, signature)
        }
        _ => {
            return Err(Error::invalid_access(
                "signature algorithm does not match the key",
            ))
        }
    };
    Ok(result.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::subtle::{
        export_key as export_public_key, import_jwk_key, KeyFormat, SignatureAlgorithm,
    };

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc_8032_ed25519_vector_one() {
        let seed = hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let public = hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let expected = hex(concat!(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
            "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        ));
        let private = import_components(
            KeyAlgorithm::Ed25519,
            Some(seed),
            public.clone(),
            true,
            &[KeyUsage::Sign],
        )
        .unwrap();
        let public = import_components(
            KeyAlgorithm::Ed25519,
            None,
            public,
            true,
            &[KeyUsage::Verify],
        )
        .unwrap();
        assert_eq!(
            sign(SignatureAlgorithm::Ed25519, &private, b"").unwrap(),
            expected
        );
        assert!(verify(SignatureAlgorithm::Ed25519, &public, &expected, b"").unwrap());
        assert!(!verify(SignatureAlgorithm::Ed25519, &public, &expected, b"x").unwrap());
    }

    #[test]
    fn generation_requires_a_private_key_usage() {
        for algorithm in [GenerateAlgorithm::EcdsaP256, GenerateAlgorithm::Ed25519] {
            for usages in [&[KeyUsage::Verify][..], &[]] {
                assert_eq!(
                    generate_pair(algorithm, true, usages).unwrap_err().name,
                    super::super::ErrorName::SyntaxError
                );
            }
        }
    }

    #[test]
    fn rfc_6979_p256_sha256_and_stock_ring_hash_refusals() {
        let public = hex(concat!(
            "04",
            "60fed4ba255a9d31c961eb74c6356d68c049b8923b61fa6ce669622e60f29fb6",
            "7903fe1008b8bc99a41ae9e95628bc64f2f1b20c2d7e9f5177a3c294d4462299"
        ));
        let key = import_components(
            KeyAlgorithm::EcdsaP256,
            None,
            public,
            true,
            &[KeyUsage::Verify],
        )
        .unwrap();
        let signature = hex(concat!(
            "efd48b2aacb6a8fd1140dd9cd45e81d69d2c877b56aaf991c34d0ea84eaf3716",
            "f7cb1c942d657c41d436c7a1b6e29f65f3e900dbb9aff4064dc4ab2f843acda8"
        ));
        assert!(verify(
            SignatureAlgorithm::Ecdsa {
                hash: HashAlgorithm::Sha256
            },
            &key,
            &signature,
            b"sample"
        )
        .unwrap());
        for hash in [HashAlgorithm::Sha384, HashAlgorithm::Sha512] {
            assert_eq!(
                verify(
                    SignatureAlgorithm::Ecdsa { hash },
                    &key,
                    &signature,
                    b"sample"
                )
                .unwrap_err()
                .name,
                super::super::ErrorName::NotSupportedError
            );
        }
    }

    #[test]
    fn generated_pairs_sign_verify_and_round_trip_der() {
        for algorithm in [GenerateAlgorithm::EcdsaP256, GenerateAlgorithm::Ed25519] {
            let pair = generate_pair(algorithm, true, &[KeyUsage::Sign, KeyUsage::Verify]).unwrap();
            let signature_algorithm = match algorithm {
                GenerateAlgorithm::EcdsaP256 => SignatureAlgorithm::Ecdsa {
                    hash: HashAlgorithm::Sha256,
                },
                GenerateAlgorithm::Ed25519 => SignatureAlgorithm::Ed25519,
                _ => unreachable!(),
            };
            let signature = sign(signature_algorithm, &pair.private_key, b"ibex").unwrap();
            assert!(verify(signature_algorithm, &pair.public_key, &signature, b"ibex").unwrap());

            let ExportedKey::Pkcs8(pkcs8) =
                export_public_key(KeyFormat::Pkcs8, &pair.private_key).unwrap()
            else {
                panic!()
            };
            let ExportedKey::Spki(spki) =
                export_public_key(KeyFormat::Spki, &pair.public_key).unwrap()
            else {
                panic!()
            };
            let import = match algorithm {
                GenerateAlgorithm::EcdsaP256 => ImportAlgorithm::EcdsaP256,
                GenerateAlgorithm::Ed25519 => ImportAlgorithm::Ed25519,
                _ => unreachable!(),
            };
            let private = import_pkcs8(&pkcs8, import, true, &[KeyUsage::Sign]).unwrap();
            let public = import_spki(&spki, import, true, &[KeyUsage::Verify]).unwrap();
            let signature = sign(signature_algorithm, &private, b"again").unwrap();
            assert!(verify(signature_algorithm, &public, &signature, b"again").unwrap());
        }
    }

    #[test]
    fn p256_pkcs8_requires_an_embedded_public_key() {
        let mut pkcs8 = hex(concat!(
            "3041020100301306072a8648ce3d020106082a8648ce3d030107",
            "042730250201010420"
        ));
        pkcs8.extend_from_slice(&[5; 32]);
        let error =
            import_pkcs8(&pkcs8, ImportAlgorithm::EcdsaP256, true, &[KeyUsage::Sign]).unwrap_err();
        assert_eq!(error.name, super::super::ErrorName::NotSupportedError);
        assert!(error.message.contains("public key must be present"));
    }

    #[test]
    fn malformed_jwks_are_rejected() {
        let base = JsonWebKey {
            kty: "OKP".into(),
            k: None,
            crv: Some("Ed25519".into()),
            x: Some(URL_SAFE_NO_PAD.encode([7; 32])),
            y: None,
            d: None,
            alg: Some("Ed25519".into()),
            key_use: Some("sig".into()),
            key_ops: Some(vec!["verify".into()]),
            ext: Some(true),
        };
        for jwk in [
            JsonWebKey {
                kty: "EC".into(),
                ..base.clone()
            },
            JsonWebKey {
                crv: Some("X25519".into()),
                ..base.clone()
            },
            JsonWebKey {
                x: Some("not+base64".into()),
                ..base.clone()
            },
            JsonWebKey {
                key_ops: Some(vec!["verify".into(), "verify".into()]),
                ..base.clone()
            },
            JsonWebKey {
                alg: Some("Ed448".into()),
                ..base.clone()
            },
            JsonWebKey {
                key_use: Some("enc".into()),
                ..base.clone()
            },
            JsonWebKey {
                key_ops: Some(Vec::new()),
                ..base.clone()
            },
            JsonWebKey {
                ext: Some(false),
                ..base.clone()
            },
        ] {
            assert_eq!(
                import_jwk_key(&jwk, ImportAlgorithm::Ed25519, true, &[KeyUsage::Verify])
                    .unwrap_err()
                    .name,
                super::super::ErrorName::DataError
            );
        }
    }
}
