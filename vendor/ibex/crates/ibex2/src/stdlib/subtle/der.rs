//! Strict DER for the two asymmetric key families exposed by `crypto.subtle`.
//!
//! This is intentionally not a general ASN.1 implementation. Accepting a
//! broader grammar here would make key import less reviewable, not more useful.
//!
//! @ref LLP 0059.000#314-cryptosubtle--pure-ungated-author-required — key material crosses only through explicit import/export

use super::{Error, Result};

const SEQUENCE: u8 = 0x30;
const INTEGER: u8 = 0x02;
const BIT_STRING: u8 = 0x03;
const OCTET_STRING: u8 = 0x04;
const OID: u8 = 0x06;

const EC_PUBLIC_KEY_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
const P256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const ED25519_OID: &[u8] = &[0x2b, 0x65, 0x70];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Algorithm {
    EcdsaP256,
    Ed25519,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PrivateKey {
    pub algorithm: Algorithm,
    pub private: Vec<u8>,
    pub public: Option<Vec<u8>>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn at_end(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn byte(&mut self) -> Result<u8> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or_else(|| Error::data("truncated DER"))?;
        self.offset += 1;
        Ok(value)
    }

    fn length(&mut self) -> Result<usize> {
        let first = self.byte()?;
        if first < 0x80 {
            return Ok(first as usize);
        }
        if first == 0x80 {
            return Err(Error::data("indefinite DER lengths are forbidden"));
        }
        let width = (first & 0x7f) as usize;
        if width == 0 || width > std::mem::size_of::<usize>() {
            return Err(Error::data("invalid DER length"));
        }
        let first_length_byte = self.byte()?;
        if first_length_byte == 0 {
            return Err(Error::data("non-minimal DER length"));
        }
        let mut length = first_length_byte as usize;
        for _ in 1..width {
            length = length
                .checked_mul(256)
                .and_then(|value| value.checked_add(self.byte().ok()? as usize))
                .ok_or_else(|| Error::data("DER length overflow"))?;
        }
        if length < 128 {
            return Err(Error::data("non-minimal DER length"));
        }
        Ok(length)
    }

    fn value(&mut self, tag: u8) -> Result<&'a [u8]> {
        if self.byte()? != tag {
            return Err(Error::data("unexpected DER tag"));
        }
        let length = self.length()?;
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| Error::data("truncated DER value"))?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn nested<T>(
        &mut self,
        tag: u8,
        parse: impl FnOnce(&mut Reader<'a>) -> Result<T>,
    ) -> Result<T> {
        let value = self.value(tag)?;
        let mut nested = Reader::new(value);
        let result = parse(&mut nested)?;
        if !nested.at_end() {
            return Err(Error::data("trailing bytes inside DER value"));
        }
        Ok(result)
    }

    fn small_integer(&mut self) -> Result<u8> {
        let value = self.value(INTEGER)?;
        if value.len() != 1 || value[0] >= 0x80 {
            return Err(Error::data("invalid DER INTEGER"));
        }
        Ok(value[0])
    }

    fn oid(&mut self, expected: &[u8]) -> Result<()> {
        if self.value(OID)? != expected {
            return Err(Error::data("unexpected key algorithm OID"));
        }
        Ok(())
    }

    fn bit_string(&mut self, tag: u8) -> Result<&'a [u8]> {
        let value = self.value(tag)?;
        let (&unused, bytes) = value
            .split_first()
            .ok_or_else(|| Error::data("empty DER BIT STRING"))?;
        if unused != 0 {
            return Err(Error::data("DER BIT STRING has unused bits"));
        }
        Ok(bytes)
    }
}

fn parse_algorithm(reader: &mut Reader<'_>) -> Result<Algorithm> {
    reader.nested(SEQUENCE, |algorithm| {
        let oid = algorithm.value(OID)?;
        if oid == ED25519_OID {
            if !algorithm.at_end() {
                return Err(Error::data("Ed25519 parameters must be absent"));
            }
            return Ok(Algorithm::Ed25519);
        }
        if oid != EC_PUBLIC_KEY_OID {
            return Err(Error::data("unexpected key algorithm OID"));
        }
        algorithm.oid(P256_OID)?;
        Ok(Algorithm::EcdsaP256)
    })
}

pub(super) fn parse_spki(bytes: &[u8]) -> Result<(Algorithm, Vec<u8>)> {
    let mut reader = Reader::new(bytes);
    let result = reader.nested(SEQUENCE, |spki| {
        let algorithm = parse_algorithm(spki)?;
        let public = spki.bit_string(BIT_STRING)?.to_vec();
        match algorithm {
            Algorithm::EcdsaP256 if public.len() == 65 && public[0] == 0x04 => {}
            Algorithm::EcdsaP256 => {
                return Err(Error::data(
                    "P-256 public keys must be uncompressed 65-byte points",
                ))
            }
            Algorithm::Ed25519 if public.len() == 32 => {}
            Algorithm::Ed25519 => return Err(Error::data("Ed25519 public keys must be 32 bytes")),
        }
        Ok((algorithm, public))
    })?;
    if !reader.at_end() {
        return Err(Error::data("trailing bytes after SubjectPublicKeyInfo"));
    }
    Ok(result)
}

pub(super) fn parse_pkcs8(bytes: &[u8]) -> Result<PrivateKey> {
    let mut reader = Reader::new(bytes);
    let result = reader.nested(SEQUENCE, |pkcs8| {
        let version = pkcs8.small_integer()?;
        let algorithm = parse_algorithm(pkcs8)?;
        let private = pkcs8.value(OCTET_STRING)?;
        match algorithm {
            Algorithm::EcdsaP256 => {
                if version != 0 {
                    return Err(Error::data("P-256 PKCS#8 version must be 0"));
                }
                let mut inner = Reader::new(private);
                let (private, public) = inner.nested(SEQUENCE, |ec| {
                    if ec.small_integer()? != 1 {
                        return Err(Error::data("ECPrivateKey version must be 1"));
                    }
                    let private = ec.value(OCTET_STRING)?.to_vec();
                    if private.len() != 32 {
                        return Err(Error::data("P-256 private scalars must be 32 bytes"));
                    }
                    if ec.peek() == Some(0xa0) {
                        ec.nested(0xa0, |parameters| parameters.oid(P256_OID))?;
                    }
                    let public = if ec.peek() == Some(0xa1) {
                        let public =
                            ec.nested(0xa1, |public| Ok(public.bit_string(BIT_STRING)?.to_vec()))?;
                        if public.len() != 65 || public[0] != 0x04 {
                            return Err(Error::data(
                                "P-256 public keys must be uncompressed 65-byte points",
                            ));
                        }
                        Some(public)
                    } else {
                        None
                    };
                    Ok((private, public))
                })?;
                if !inner.at_end() {
                    return Err(Error::data("trailing bytes after ECPrivateKey"));
                }
                Ok(PrivateKey {
                    algorithm,
                    private,
                    public,
                })
            }
            Algorithm::Ed25519 => {
                if version > 1 {
                    return Err(Error::data("unsupported Ed25519 PKCS#8 version"));
                }
                let mut wrapped = Reader::new(private);
                let seed = wrapped.value(OCTET_STRING)?.to_vec();
                if !wrapped.at_end() || seed.len() != 32 {
                    return Err(Error::data("Ed25519 PKCS#8 must contain a 32-byte seed"));
                }
                let public = if version == 1 {
                    let public = pkcs8.bit_string(0x81)?.to_vec();
                    if public.len() != 32 {
                        return Err(Error::data("Ed25519 public keys must be 32 bytes"));
                    }
                    Some(public)
                } else {
                    None
                };
                Ok(PrivateKey {
                    algorithm,
                    private: seed,
                    public,
                })
            }
        }
    })?;
    if !reader.at_end() {
        return Err(Error::data("trailing bytes after PrivateKeyInfo"));
    }
    Ok(result)
}

fn push_length(output: &mut Vec<u8>, length: usize) {
    if length < 128 {
        output.push(length as u8);
    } else {
        debug_assert!(length <= 255);
        output.extend_from_slice(&[0x81, length as u8]);
    }
}

fn push_tlv(output: &mut Vec<u8>, tag: u8, value: &[u8]) {
    output.push(tag);
    push_length(output, value.len());
    output.extend_from_slice(value);
}

fn encoded_algorithm(algorithm: Algorithm) -> Vec<u8> {
    let mut value = Vec::new();
    match algorithm {
        Algorithm::EcdsaP256 => {
            push_tlv(&mut value, OID, EC_PUBLIC_KEY_OID);
            push_tlv(&mut value, OID, P256_OID);
        }
        Algorithm::Ed25519 => push_tlv(&mut value, OID, ED25519_OID),
    }
    let mut output = Vec::new();
    push_tlv(&mut output, SEQUENCE, &value);
    output
}

pub(super) fn encode_spki(algorithm: Algorithm, public: &[u8]) -> Vec<u8> {
    let mut value = encoded_algorithm(algorithm);
    let mut bits = Vec::with_capacity(public.len() + 1);
    bits.push(0);
    bits.extend_from_slice(public);
    push_tlv(&mut value, BIT_STRING, &bits);
    let mut output = Vec::new();
    push_tlv(&mut output, SEQUENCE, &value);
    output
}

pub(super) fn encode_pkcs8(algorithm: Algorithm, private: &[u8], public: &[u8]) -> Vec<u8> {
    let mut value = Vec::new();
    push_tlv(&mut value, INTEGER, &[0]);
    value.extend_from_slice(&encoded_algorithm(algorithm));
    match algorithm {
        Algorithm::EcdsaP256 => {
            let mut ec = Vec::new();
            push_tlv(&mut ec, INTEGER, &[1]);
            push_tlv(&mut ec, OCTET_STRING, private);
            let mut bits = Vec::with_capacity(public.len() + 1);
            bits.push(0);
            bits.extend_from_slice(public);
            let mut public_field = Vec::new();
            push_tlv(&mut public_field, BIT_STRING, &bits);
            push_tlv(&mut ec, 0xa1, &public_field);
            let mut ec_sequence = Vec::new();
            push_tlv(&mut ec_sequence, SEQUENCE, &ec);
            push_tlv(&mut value, OCTET_STRING, &ec_sequence);
        }
        Algorithm::Ed25519 => {
            let mut seed = Vec::new();
            push_tlv(&mut seed, OCTET_STRING, private);
            push_tlv(&mut value, OCTET_STRING, &seed);
        }
    }
    let mut output = Vec::new();
    push_tlv(&mut output, SEQUENCE, &value);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_spki_encodings_round_trip() {
        let p256 = [
            vec![0x30, 0x59, 0x30, 0x13, 0x06, 0x07],
            EC_PUBLIC_KEY_OID.to_vec(),
            vec![0x06, 0x08],
            P256_OID.to_vec(),
            vec![0x03, 0x42, 0x00, 0x04],
            vec![0; 64],
        ]
        .concat();
        let point = [&[0x04][..], &[0; 64][..]].concat();
        assert_eq!(encode_spki(Algorithm::EcdsaP256, &point), p256);
        assert_eq!(parse_spki(&p256).unwrap().0, Algorithm::EcdsaP256);

        let ed = [
            vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03],
            ED25519_OID.to_vec(),
            vec![0x03, 0x21, 0x00],
            vec![7; 32],
        ]
        .concat();
        assert_eq!(encode_spki(Algorithm::Ed25519, &[7; 32]), ed);
        assert_eq!(parse_spki(&ed).unwrap(), (Algorithm::Ed25519, vec![7; 32]));
    }

    #[test]
    fn fixed_pkcs8_encodings_round_trip() {
        let ed = [
            vec![0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03],
            ED25519_OID.to_vec(),
            vec![0x04, 0x22, 0x04, 0x20],
            vec![9; 32],
        ]
        .concat();
        assert_eq!(encode_pkcs8(Algorithm::Ed25519, &[9; 32], &[]), ed);
        assert_eq!(
            parse_pkcs8(&ed).unwrap(),
            PrivateKey {
                algorithm: Algorithm::Ed25519,
                private: vec![9; 32],
                public: None,
            }
        );

        let private = [3; 32];
        let public = [&[0x04][..], &[4; 64][..]].concat();
        let encoded = encode_pkcs8(Algorithm::EcdsaP256, &private, &public);
        let expected = [
            vec![0x30, 0x81, 0x87, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07],
            EC_PUBLIC_KEY_OID.to_vec(),
            vec![0x06, 0x08],
            P256_OID.to_vec(),
            vec![0x04, 0x6d, 0x30, 0x6b, 0x02, 0x01, 0x01, 0x04, 0x20],
            private.to_vec(),
            vec![0xa1, 0x44, 0x03, 0x42, 0x00],
            public.clone(),
        ]
        .concat();
        assert_eq!(encoded, expected);
        assert_eq!(
            parse_pkcs8(&encoded).unwrap(),
            PrivateKey {
                algorithm: Algorithm::EcdsaP256,
                private: private.to_vec(),
                public: Some(public),
            }
        );

        let private_only = [
            vec![
                0x30, 0x41, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d,
                0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04, 0x27,
                0x30, 0x25, 0x02, 0x01, 0x01, 0x04, 0x20,
            ],
            vec![5; 32],
        ]
        .concat();
        assert_eq!(
            parse_pkcs8(&private_only).unwrap(),
            PrivateKey {
                algorithm: Algorithm::EcdsaP256,
                private: vec![5; 32],
                public: None,
            }
        );
    }

    #[test]
    fn rejects_non_der_and_wrong_structures() {
        let public = [7; 32];
        let valid = encode_spki(Algorithm::Ed25519, &public);
        for malformed in [
            [&valid[..], &[0]].concat(),
            [vec![0x30, 0x80], valid[2..].to_vec()].concat(),
            [vec![0x30, 0x81, valid[1]], valid[2..].to_vec()].concat(),
        ] {
            assert_eq!(
                parse_spki(&malformed).unwrap_err().name,
                super::super::ErrorName::DataError
            );
        }

        let mut wrong_oid = valid.clone();
        wrong_oid[8] ^= 1;
        assert!(parse_spki(&wrong_oid).is_err());

        let mut unused_bits = valid;
        unused_bits[11] = 1;
        assert!(parse_spki(&unused_bits).is_err());

        let point = [&[0x04][..], &[1; 64][..]].concat();
        let mut ec_spki = encode_spki(Algorithm::EcdsaP256, &point);
        ec_spki[13] = SEQUENCE;
        assert!(parse_spki(&ec_spki).is_err(), "explicit curve parameters");

        let mut compressed = encode_spki(Algorithm::EcdsaP256, &point);
        compressed[26] = 0x02;
        assert!(parse_spki(&compressed).is_err(), "compressed point");

        let pkcs8 = encode_pkcs8(Algorithm::EcdsaP256, &[3; 32], &point);
        assert!(parse_pkcs8(&[pkcs8.as_slice(), &[0]].concat()).is_err());
        let mut wrong_curve = pkcs8.clone();
        wrong_curve[25] ^= 1;
        assert!(parse_pkcs8(&wrong_curve).is_err());
        let mut inner_trailing = pkcs8;
        inner_trailing[1] += 1;
        inner_trailing[28] += 1;
        inner_trailing[30] += 1;
        inner_trailing.push(0);
        assert!(parse_pkcs8(&inner_trailing).is_err());
    }
}
