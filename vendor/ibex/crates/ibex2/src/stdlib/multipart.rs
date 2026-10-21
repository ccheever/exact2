//! RFC 7578 `multipart/form-data`, over engine-independent Rust values.
//!
//! JavaScript keeps `Blob`, `File`, and `FormData` object state in its own
//! `ArrayBuffer`s. Only the final wire encoding crosses into this module. A
//! Rust consumer uses the same [`FormData`] value directly, without an engine
//! or a bindings handle table.
//!
//! @ref LLP 0057.000#l6--blob-file-formdata — JS shapes over bytes, multipart encoding in Rust

use std::{error::Error, fmt};

/// An ordered form entry list. Duplicate names are retained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormData {
    entries: Vec<FormDataEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormDataEntry {
    pub name: String,
    pub value: FormDataValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormDataValue {
    Text(String),
    File {
        bytes: Vec<u8>,
        filename: String,
        content_type: String,
    },
}

#[derive(Clone, Copy, Debug)]
struct EntryRef<'a> {
    name: &'a str,
    value: ValueRef<'a>,
}

#[derive(Clone, Copy, Debug)]
enum ValueRef<'a> {
    Text(&'a str),
    File {
        bytes: &'a [u8],
        filename: &'a str,
        content_type: &'a str,
    },
}

impl FormDataEntry {
    fn as_ref(&self) -> EntryRef<'_> {
        EntryRef {
            name: &self.name,
            value: match &self.value {
                FormDataValue::Text(value) => ValueRef::Text(value),
                FormDataValue::File {
                    bytes,
                    filename,
                    content_type,
                } => ValueRef::File {
                    bytes,
                    filename,
                    content_type,
                },
            },
        }
    }
}

/// The host ABI's entry list. Strings and file bytes remain borrowed from the
/// inbound [`crate::boundary::HostArg`] spans until the final payload is
/// written, so the ABI adds no intermediate file-byte copy.
// @ref LLP 0059.000#12-what-by-handle-requires — inbound ArrayBuffers remain borrowed for the host call
#[derive(Debug, Default)]
#[cfg(feature = "bindings")]
pub(crate) struct BorrowedFormData<'a> {
    entries: Vec<EntryRef<'a>>,
}

#[cfg(feature = "bindings")]
impl<'a> BorrowedFormData<'a> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn append_text(&mut self, name: &'a str, value: &'a str) {
        self.entries.push(EntryRef {
            name,
            value: ValueRef::Text(value),
        });
    }

    pub(crate) fn append_file(
        &mut self,
        name: &'a str,
        bytes: &'a [u8],
        filename: &'a str,
        content_type: &'a str,
    ) {
        self.entries.push(EntryRef {
            name,
            value: ValueRef::File {
                bytes,
                filename,
                content_type,
            },
        });
    }
}

impl FormData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append_text(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.entries.push(FormDataEntry {
            name: name.into(),
            value: FormDataValue::Text(value.into()),
        });
    }

    pub fn append_file(
        &mut self,
        name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
        filename: impl Into<String>,
        content_type: impl Into<String>,
    ) {
        self.entries.push(FormDataEntry {
            name: name.into(),
            value: FormDataValue::File {
                bytes: bytes.into(),
                filename: filename.into(),
                content_type: content_type.into(),
            },
        });
    }

    pub fn entries(&self) -> &[FormDataEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A complete multipart payload and the boundary its Content-Type must name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedMultipart {
    boundary: String,
    bytes: Vec<u8>,
}

impl EncodedMultipart {
    /// Generate a boundary from the operating system CSPRNG and encode `form`.
    pub fn new(form: &FormData) -> Result<Self, MultipartError> {
        Self::with_boundary(form, generate_boundary()?)
    }

    /// Encode with a caller-supplied boundary. This is public for deterministic
    /// protocol tests and consumers that have already generated a boundary.
    /// Boundaries must use RFC 2046 `bchars`, be 1–70 bytes, and not end in a
    /// space. [`Self::content_type`] always quotes the validated value.
    pub fn with_boundary(
        form: &FormData,
        boundary: impl Into<String>,
    ) -> Result<Self, MultipartError> {
        let boundary = boundary.into();
        validate_boundary(&boundary)?;
        let length = encoded_len(form.entries().iter().map(FormDataEntry::as_ref), &boundary)?;
        let mut bytes = Vec::with_capacity(length);
        write_form(
            &mut bytes,
            form.entries().iter().map(FormDataEntry::as_ref),
            &boundary,
        )?;
        debug_assert_eq!(bytes.len(), length);
        Ok(Self { boundary, bytes })
    }

    #[cfg(feature = "bindings")]
    pub(crate) fn with_borrowed_boundary(
        form: &BorrowedFormData<'_>,
        boundary: &str,
    ) -> Result<Self, MultipartError> {
        validate_boundary(boundary)?;
        let length = encoded_len(form.entries.iter().copied(), boundary)?;
        let mut bytes = Vec::with_capacity(length);
        write_form(&mut bytes, form.entries.iter().copied(), boundary)?;
        debug_assert_eq!(bytes.len(), length);
        Ok(Self {
            boundary: boundary.to_owned(),
            bytes,
        })
    }

    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    pub fn content_type(&self) -> String {
        // RFC 2046 bchars exclude quote and backslash, so a validated value is
        // safe to quote without another escaping rule.
        format!("multipart/form-data; boundary=\"{}\"", self.boundary)
    }

    /// The exact number of bytes that [`Self::into_bytes`] returns.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultipartError {
    Entropy(String),
    InvalidBoundary,
    InvalidContentType,
    LengthOverflow,
}

impl fmt::Display for MultipartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entropy(error) => write!(f, "multipart boundary entropy failed: {error}"),
            Self::InvalidBoundary => f.write_str("invalid multipart boundary"),
            Self::InvalidContentType => f.write_str("invalid multipart file content type"),
            Self::LengthOverflow => f.write_str("multipart body length overflow"),
        }
    }
}

impl Error for MultipartError {}

/// A boundary with 128 random bits. The fixed ASCII prefix is diagnostic; the
/// unpredictable suffix is what prevents a part body from choosing a collision.
pub fn generate_boundary() -> Result<String, MultipartError> {
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random)
        .map_err(|error| MultipartError::Entropy(error.to_string()))?;
    let mut boundary = String::with_capacity(10 + random.len() * 2);
    boundary.push_str("----ibex2-");
    for byte in random {
        use fmt::Write as _;
        write!(&mut boundary, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(boundary)
}

fn validate_boundary(boundary: &str) -> Result<(), MultipartError> {
    // RFC 2046's bcharsnospace plus internal spaces, at most 70 characters.
    let valid = !boundary.is_empty()
        && boundary.len() <= 70
        && !boundary.ends_with(' ')
        && boundary.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'\''
                        | b'('
                        | b')'
                        | b'+'
                        | b'_'
                        | b','
                        | b'-'
                        | b'.'
                        | b'/'
                        | b':'
                        | b'='
                        | b'?'
                        | b' '
                )
        });
    valid.then_some(()).ok_or(MultipartError::InvalidBoundary)
}

fn normalized_newlines(value: &str) -> Vec<u8> {
    let source = value.as_bytes();
    let mut result = Vec::with_capacity(source.len());
    let mut index = 0;
    while index < source.len() {
        match source[index] {
            b'\r' => {
                result.extend_from_slice(b"\r\n");
                index += usize::from(source.get(index + 1) == Some(&b'\n')) + 1;
            }
            b'\n' => {
                result.extend_from_slice(b"\r\n");
                index += 1;
            }
            byte => {
                result.push(byte);
                index += 1;
            }
        }
    }
    result
}

fn escaped_parameter(value: &str) -> Vec<u8> {
    let normalized = normalized_newlines(value);
    let extra = normalized
        .iter()
        .filter(|byte| matches!(byte, b'\r' | b'\n' | b'"'))
        .count()
        * 2;
    let mut escaped = Vec::with_capacity(normalized.len() + extra);
    for byte in normalized {
        match byte {
            b'\r' => escaped.extend_from_slice(b"%0D"),
            b'\n' => escaped.extend_from_slice(b"%0A"),
            b'"' => escaped.extend_from_slice(b"%22"),
            byte => escaped.push(byte),
        }
    }
    escaped
}

fn file_content_type(value: &str) -> Result<&str, MultipartError> {
    if value.is_empty() {
        return Ok("application/octet-stream");
    }
    value
        .bytes()
        .all(|byte| (0x20..=0x7e).contains(&byte))
        .then_some(value)
        .ok_or(MultipartError::InvalidContentType)
}

fn checked_add(total: &mut usize, amount: usize) -> Result<(), MultipartError> {
    *total = total
        .checked_add(amount)
        .ok_or(MultipartError::LengthOverflow)?;
    Ok(())
}

fn encoded_len<'a>(
    entries: impl IntoIterator<Item = EntryRef<'a>>,
    boundary: &str,
) -> Result<usize, MultipartError> {
    let mut length = 0usize;
    for entry in entries {
        checked_add(&mut length, 2 + boundary.len() + 2)?; // --boundary CRLF
        checked_add(
            &mut length,
            b"Content-Disposition: form-data; name=\"".len(),
        )?;
        checked_add(&mut length, escaped_parameter(entry.name).len())?;
        match entry.value {
            ValueRef::Text(value) => {
                checked_add(&mut length, b"\"\r\n\r\n".len())?;
                checked_add(&mut length, normalized_newlines(value).len())?;
            }
            ValueRef::File {
                bytes,
                filename,
                content_type,
            } => {
                let content_type = file_content_type(content_type)?;
                checked_add(&mut length, b"\"; filename=\"".len())?;
                checked_add(&mut length, escaped_parameter(filename).len())?;
                checked_add(&mut length, b"\"\r\nContent-Type: ".len())?;
                checked_add(&mut length, content_type.len())?;
                checked_add(&mut length, b"\r\n\r\n".len())?;
                checked_add(&mut length, bytes.len())?;
            }
        }
        checked_add(&mut length, 2)?; // trailing CRLF
    }
    checked_add(&mut length, 2 + boundary.len() + 4)?; // --boundary-- CRLF
    Ok(length)
}

fn write_form<'a>(
    output: &mut Vec<u8>,
    entries: impl IntoIterator<Item = EntryRef<'a>>,
    boundary: &str,
) -> Result<(), MultipartError> {
    for entry in entries {
        output.extend_from_slice(b"--");
        output.extend_from_slice(boundary.as_bytes());
        output.extend_from_slice(b"\r\nContent-Disposition: form-data; name=\"");
        output.extend_from_slice(&escaped_parameter(entry.name));
        match entry.value {
            ValueRef::Text(value) => {
                output.extend_from_slice(b"\"\r\n\r\n");
                output.extend_from_slice(&normalized_newlines(value));
            }
            ValueRef::File {
                bytes,
                filename,
                content_type,
            } => {
                output.extend_from_slice(b"\"; filename=\"");
                output.extend_from_slice(&escaped_parameter(filename));
                output.extend_from_slice(b"\"\r\nContent-Type: ");
                output.extend_from_slice(file_content_type(content_type)?.as_bytes());
                output.extend_from_slice(b"\r\n\r\n");
                output.extend_from_slice(bytes);
            }
        }
        output.extend_from_slice(b"\r\n");
    }
    output.extend_from_slice(b"--");
    output.extend_from_slice(boundary.as_bytes());
    output.extend_from_slice(b"--\r\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_answer_preserves_order_headers_and_bytes() {
        let mut form = FormData::new();
        form.append_text("alpha", "one\ntwo");
        form.append_file("upload", vec![0, 1, 0xff], "a.txt", "text/plain");
        form.append_text("alpha", "last");
        let encoded = EncodedMultipart::with_boundary(&form, "fixed-boundary").unwrap();
        let expected = b"--fixed-boundary\r\n\
Content-Disposition: form-data; name=\"alpha\"\r\n\r\n\
one\r\ntwo\r\n\
--fixed-boundary\r\n\
Content-Disposition: form-data; name=\"upload\"; filename=\"a.txt\"\r\n\
Content-Type: text/plain\r\n\r\n\
\x00\x01\xff\r\n\
--fixed-boundary\r\n\
Content-Disposition: form-data; name=\"alpha\"\r\n\r\n\
last\r\n\
--fixed-boundary--\r\n";
        assert_eq!(encoded.as_bytes(), expected);
        assert_eq!(encoded.len(), expected.len());
        assert_eq!(
            encoded.content_type(),
            "multipart/form-data; boundary=\"fixed-boundary\""
        );
    }

    #[test]
    fn names_and_filenames_follow_the_html_escaping_algorithm() {
        let mut form = FormData::new();
        form.append_file("na\"me\rline\n雪", b"x".to_vec(), "fi\"le\r\n雪.txt", "");
        let encoded = EncodedMultipart::with_boundary(&form, "b").unwrap();
        let text = String::from_utf8(encoded.into_bytes()).unwrap();
        assert!(text.contains("name=\"na%22me%0D%0Aline%0D%0A雪\""));
        assert!(text.contains("filename=\"fi%22le%0D%0A雪.txt\""));
        assert!(text.contains("Content-Type: application/octet-stream\r\n"));
        assert!(
            !text.contains("%E9%9B%AA"),
            "non-ASCII is UTF-8, not escaped"
        );
    }

    #[test]
    fn computed_length_matches_every_encoding_shape() {
        for form in [
            FormData::new(),
            {
                let mut form = FormData::new();
                form.append_text("a\r\nb", "x\ry\nz");
                form
            },
            {
                let mut form = FormData::new();
                form.append_file("雪", vec![0; 257], "x.bin", "application/octet-stream");
                form
            },
        ] {
            let encoded = EncodedMultipart::with_boundary(&form, "length-test").unwrap();
            assert_eq!(encoded.len(), encoded.as_bytes().len());
        }
    }

    #[test]
    fn generated_boundaries_are_rfc_safe_and_not_reused() {
        let first = generate_boundary().unwrap();
        let second = generate_boundary().unwrap();
        validate_boundary(&first).unwrap();
        validate_boundary(&second).unwrap();
        assert_ne!(first, second);
        assert_eq!(first.len(), 42);
    }

    #[test]
    fn caller_boundaries_are_rfc_2046_bchars_of_length_one_through_seventy() {
        let form = FormData::new();
        let all_bchars = "AZaz09'()+_,-./:=? internal space";
        let encoded = EncodedMultipart::with_boundary(&form, all_bchars).unwrap();
        assert_eq!(
            encoded.content_type(),
            format!("multipart/form-data; boundary=\"{all_bchars}\"")
        );
        assert!(EncodedMultipart::with_boundary(&form, "x".repeat(70)).is_ok());

        for invalid in [
            String::new(),
            "x".repeat(71),
            "trailing ".into(),
            "quote\"".into(),
            "back\\slash".into(),
            "star*".into(),
            "line\r\nbreak".into(),
            "snow-雪".into(),
        ] {
            assert_eq!(
                EncodedMultipart::with_boundary(&form, invalid),
                Err(MultipartError::InvalidBoundary)
            );
        }
    }

    #[test]
    #[cfg(feature = "bindings")]
    fn borrowed_form_data_writes_directly_from_the_inbound_file_slice() {
        let file_bytes = [0, 1, 0xff];
        let mut borrowed = BorrowedFormData::new();
        borrowed.append_text("alpha", "one\ntwo");
        borrowed.append_file("upload", &file_bytes, "a.txt", "text/plain");
        let borrowed_file = match borrowed.entries[1].value {
            ValueRef::File { bytes, .. } => bytes,
            ValueRef::Text(_) => panic!("file entry became text"),
        };
        assert_eq!(
            borrowed_file.as_ptr(),
            file_bytes.as_ptr(),
            "the borrowed form must retain the inbound span, not a copied Vec"
        );

        let mut owned = FormData::new();
        owned.append_text("alpha", "one\ntwo");
        owned.append_file("upload", file_bytes.to_vec(), "a.txt", "text/plain");
        assert_eq!(
            EncodedMultipart::with_borrowed_boundary(&borrowed, "fixed-boundary").unwrap(),
            EncodedMultipart::with_boundary(&owned, "fixed-boundary").unwrap()
        );
    }
}
