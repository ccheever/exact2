//! ECDSA P-256 for DPoP (LLP 1069.005 D1b): generate, sign (ES256, raw
//! `r‖s`), and JWK import and export, the four WebCrypto members a source
//! needs, plus "keep this key" under `secret.keep`. One implementation for
//! both languages: the Hermes executor's `crypto.subtle` calls these too.
//!
//! @ref LLP 1069.005 D1b — generating and signing draw a nonce, so each is a
//! counted read; importing and exporting are pure. The nonce is always the
//! OS's, never the agent's stream (the web's browser draws its own).

use exact_runner::{DataError, Store};
use p256::ecdsa::signature::RandomizedSigner;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::rand_core::{CryptoRng, RngCore};
use p256::EncodedPoint;

use super::platform;

/// One P-256 key: a private key (with its public half) or a public key.
#[derive(Clone)]
pub struct EcKey {
    secret: Option<SigningKey>,
    public: VerifyingKey,
    /// Whether [`EcKey::to_jwk`] may export it (a public key always may).
    pub extractable: bool,
}

impl std::fmt::Debug for EcKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the scalar.
        f.debug_struct("EcKey")
            .field("private", &self.secret.is_some())
            .field("extractable", &self.extractable)
            .finish()
    }
}

/// A generated or kept pair, as WebCrypto's `CryptoKeyPair`.
#[derive(Clone, Debug)]
pub struct EcKeyPair {
    /// The private key: signs.
    pub private: EcKey,
    /// The public key: always extractable.
    pub public: EcKey,
}

/// A P-256 JWK's members (RFC 7518 §6.2), base64url without padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jwk {
    /// The private scalar, for a private key.
    pub d: Option<String>,
    /// The public point's x.
    pub x: String,
    /// The public point's y.
    pub y: String,
}

impl Jwk {
    /// `{"kty":"EC","crv":"P-256","x":…,"y":…}`, with `d` for a private key.
    pub fn to_json(&self) -> String {
        let mut value = serde_json::json!({"kty": "EC", "crv": "P-256", "x": self.x, "y": self.y});
        if let Some(d) = &self.d {
            value["d"] = serde_json::Value::String(d.clone());
        }
        value.to_string()
    }

    /// A JWK object's text: `kty` `EC` and `crv` `P-256`, or refused by name.
    pub fn from_json(text: &str) -> Result<Jwk, DataError> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| bad(format!("importKey: a JWK: {e}")))?;
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        if field("kty").as_deref() != Some("EC") || field("crv").as_deref() != Some("P-256") {
            return Err(unsupported("importKey: only an EC P-256 JWK".into()));
        }
        match (field("x"), field("y")) {
            (Some(x), Some(y)) => Ok(Jwk {
                d: field("d"),
                x,
                y,
            }),
            _ => Err(bad("importKey: a JWK needs x and y".into())),
        }
    }
}

fn bad(message: String) -> DataError {
    DataError::BadArguments(message)
}

fn unsupported(message: String) -> DataError {
    DataError::BadArguments(format!("NotSupportedError: {message}"))
}

impl EcKey {
    /// Whether this is a private key.
    pub fn is_private(&self) -> bool {
        self.secret.is_some()
    }

    /// The JWK, pure. A private key created non-extractable refuses
    /// (`InvalidAccessError`, as on the web).
    pub fn to_jwk(&self) -> Result<Jwk, DataError> {
        if self.secret.is_some() && !self.extractable {
            return Err(bad(
                "InvalidAccessError: exportKey: the key is not extractable".into(),
            ));
        }
        Ok(self.jwk_unchecked())
    }

    fn jwk_unchecked(&self) -> Jwk {
        let point = self.public.to_encoded_point(false);
        Jwk {
            d: self.secret.as_ref().map(|s| base64url(&s.to_bytes())),
            x: base64url(point.x().map(|x| x.as_slice()).unwrap_or_default()),
            y: base64url(point.y().map(|y| y.as_slice()).unwrap_or_default()),
        }
    }

    /// A key from a JWK, pure: private when it carries `d` (checked against
    /// `x` and `y`), else public (always extractable).
    pub fn from_jwk(jwk: &Jwk, extractable: bool) -> Result<EcKey, DataError> {
        let coordinate = |name: &str, text: &str| -> Result<[u8; 32], DataError> {
            unbase64url(text)
                .and_then(|b| <[u8; 32]>::try_from(b).ok())
                .ok_or_else(|| {
                    bad(format!(
                        "importKey: the JWK's {name} is not 32 bytes of base64url"
                    ))
                })
        };
        let (x, y) = (coordinate("x", &jwk.x)?, coordinate("y", &jwk.y)?);
        let point = EncodedPoint::from_affine_coordinates(&x.into(), &y.into(), false);
        let public = VerifyingKey::from_encoded_point(&point)
            .map_err(|_| bad("importKey: the JWK's point is not on P-256".into()))?;
        let Some(d) = &jwk.d else {
            return Ok(EcKey {
                secret: None,
                public,
                extractable: true,
            });
        };
        let secret = SigningKey::from_bytes(&coordinate("d", d)?.into())
            .map_err(|_| bad("importKey: the JWK's d is not a P-256 scalar".into()))?;
        if *secret.verifying_key() != public {
            return Err(bad(
                "importKey: the JWK's d does not match its x and y".into()
            ));
        }
        Ok(EcKey {
            secret: Some(secret),
            public,
            extractable,
        })
    }

    /// The public half of a private key (or the key itself).
    pub fn public_key(&self) -> EcKey {
        EcKey {
            secret: None,
            public: self.public,
            extractable: true,
        }
    }

    /// Whether `signature` (raw `r‖s`) is this key's over `data`. For tests
    /// that cross-check executors; no source verifies a DPoP proof (D1b).
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        use p256::ecdsa::signature::Verifier;
        Signature::from_slice(signature).is_ok_and(|s| self.public.verify(data, &s).is_ok())
    }
}

/// The OS's entropy as an RNG; a failure is remembered and refuses the
/// operation afterwards (and the bytes it produced are never used).
struct Os(Option<String>);

impl RngCore for Os {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        if let Err(e) = platform::fill(dest) {
            dest.fill(0);
            self.0 = Some(e);
        }
    }
    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> Result<(), p256::elliptic_curve::rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for Os {}

fn entropy_failed(api: &str, error: String) -> DataError {
    DataError::Unavailable(format!("{api}: OS randomness unavailable: {error}"))
}

/// `generateKey({name:"ECDSA", namedCurve:"P-256"}, extractable, …)`: a
/// fresh pair from the OS's entropy, and a counted read (D2).
pub fn generate_p256(store: &Store, extractable: bool) -> Result<EcKeyPair, DataError> {
    let mut rng = Os(None);
    let secret = SigningKey::random(&mut rng);
    if let Some(e) = rng.0 {
        return Err(entropy_failed("generateKey", e));
    }
    store.observe_entropy();
    let public = *secret.verifying_key();
    Ok(EcKeyPair {
        private: EcKey {
            secret: Some(secret),
            public,
            extractable,
        },
        public: EcKey {
            secret: None,
            public,
            extractable: true,
        },
    })
}

/// `sign({name:"ECDSA", hash:"SHA-256"}, key, data)`: ES256 as raw `r‖s`,
/// with a random nonce (hedged, so the same key and data give a different
/// signature each time, as on the web), and a counted read (D2).
pub fn sign_es256(store: &Store, key: &EcKey, data: &[u8]) -> Result<[u8; 64], DataError> {
    let Some(secret) = &key.secret else {
        return Err(bad(
            "InvalidAccessError: sign: a public key does not sign".into()
        ));
    };
    let mut rng = Os(None);
    let signature: Signature = secret.sign_with_rng(&mut rng, data);
    if let Some(e) = rng.0 {
        return Err(entropy_failed("sign", e));
    }
    store.observe_entropy();
    Ok(signature.to_bytes().into())
}

/// Keep `pair` under the `secret.keep <name>` grant (LLP 1069.005 D1b): its
/// JWK, `d` included whatever its extractability, written by Rust so the
/// private scalar never enters a source's heap. On the web a Rust source
/// can't keep a key the browser protects (a wasm scalar is readable by the
/// page), so there it refuses rather than write a JWK to `localStorage`.
pub fn keep_key(store: &mut Store, name: &str, pair: &EcKeyPair) -> Result<(), DataError> {
    if cfg!(target_arch = "wasm32") {
        return Err(DataError::Unavailable(
            "keep_key: a Rust source on the web cannot keep a key the browser protects yet (LLP 1069.005 D1b)".into(),
        ));
    }
    if !pair.private.is_private() {
        return Err(bad("keep_key: the pair has no private key".into()));
    }
    store
        .set(name, &pair.private.jwk_unchecked().to_json())
        .map_err(|e| DataError::Unavailable(format!("keep_key: {e:?}")))
}

/// The pair kept under `name`, its private key non-extractable, or `None`
/// when nothing is kept. Reading the store is a counted read.
pub fn kept_key(store: &Store, name: &str) -> Result<Option<EcKeyPair>, DataError> {
    if cfg!(target_arch = "wasm32") {
        return Err(DataError::Unavailable(
            "kept_key: a Rust source on the web cannot keep a key the browser protects yet (LLP 1069.005 D1b)".into(),
        ));
    }
    let Some(text) = store.get(name) else {
        return Ok(None);
    };
    let jwk = Jwk::from_json(text)?;
    let private = EcKey::from_jwk(&jwk, false)?;
    if !private.is_private() {
        return Err(bad(format!("kept_key: {name} holds no private key")));
    }
    let public = private.public_key();
    Ok(Some(EcKeyPair { private, public }))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding (RFC 4648 §5), as JWK and JWS spell bytes.
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The bytes of unpadded base64url text, or `None`.
pub fn unbase64url(text: &str) -> Option<Vec<u8>> {
    if text.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes() {
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRANTS: &str = "secret.keep dpop\n";

    #[test]
    fn base64url_round_trips_every_length() {
        for n in 0..70u8 {
            let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37)).collect();
            assert_eq!(unbase64url(&base64url(&bytes)).unwrap(), bytes);
        }
        assert_eq!(base64url(b"\xfb\xff"), "-_8");
        assert!(unbase64url("a").is_none() && unbase64url("a+==").is_none());
    }

    #[test]
    fn generate_and_sign_are_reads_and_verify_under_the_public_jwk() {
        let store = Store::new(GRANTS, []);
        let pair = generate_p256(&store, true).unwrap();
        let a = sign_es256(&store, &pair.private, b"proof").unwrap();
        let b = sign_es256(&store, &pair.private, b"proof").unwrap();
        assert_ne!(a, b, "a random nonce, as on the web");
        assert_eq!(store.entropy_draws(), 3);
        let public = EcKey::from_jwk(&pair.public.to_jwk().unwrap(), true).unwrap();
        assert!(public.verify(b"proof", &a) && public.verify(b"proof", &b));
        assert!(!public.verify(b"other", &a));
        // Import and export are pure.
        let jwk = pair.private.to_jwk().unwrap();
        let again = EcKey::from_jwk(&jwk, false).unwrap();
        assert_eq!(again.jwk_unchecked(), jwk);
        assert_eq!(store.entropy_draws(), 3);
        assert!(again.to_jwk().is_err(), "non-extractable refuses export");
        assert!(sign_es256(&store, &public, b"x").is_err());
    }

    /// RFC 7515 Appendix A.3's ES256 key and signature: the external vector
    /// that a signature this module did not make verifies here, and that
    /// the JWK spelling is the RFC's.
    #[test]
    fn rfc_7515_a3_verifies() {
        let jwk = Jwk {
            d: Some("jpsQnnGQmL-YBIffH1136cspYG6-0iY7X1fCE9-E9LI".into()),
            x: "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU".into(),
            y: "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0".into(),
        };
        let key = EcKey::from_jwk(&jwk, true).unwrap();
        assert_eq!(key.to_jwk().unwrap(), jwk);
        let input = b"eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ";
        let signature = unbase64url("DtEhU3ljbEg8L38VWAfUAqOyKAM6-Xx-F4GawxaepmXFCgfTjDxw5djxLa8ISlSApmWQxfKTUJqPP3-Kg6NU1Q").unwrap();
        assert!(key.public_key().verify(input, &signature));
        let mut bad = jwk.clone();
        bad.d = Some(base64url(&[1; 32]));
        assert!(EcKey::from_jwk(&bad, true).is_err(), "d must match x and y");
    }

    #[test]
    fn a_kept_key_comes_back_non_extractable_and_signs() {
        let mut store = Store::new(GRANTS, []);
        let pair = generate_p256(&store, false).unwrap();
        keep_key(&mut store, "dpop", &pair).unwrap();
        let kept = kept_key(&store, "dpop").unwrap().unwrap();
        assert!(kept.private.to_jwk().is_err());
        assert_eq!(kept.public.to_jwk().unwrap(), pair.public.to_jwk().unwrap());
        let sig = sign_es256(&store, &kept.private, b"m").unwrap();
        assert!(pair.public.verify(b"m", &sig));
        assert!(kept_key(&store, "other").unwrap().is_none());
        assert!(keep_key(&mut store, "undeclared", &pair).is_err());
        assert!(
            Jwk::from_json(r#"{"kty":"EC","crv":"P-384","x":"a","y":"b"}"#)
                .map_err(|e| format!("{e:?}"))
                .unwrap_err()
                .contains("NotSupportedError")
        );
    }
}
