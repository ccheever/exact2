//! Pure Windows drive-path grammar; no host filesystem access.
//! @ref LLP 1027.001#proposed-windows-native-filesystem-grant-integration — one lexical authority.
use std::io;

fn refuse(message: &str) -> io::Error {
    io::Error::other(message)
}

/// A qualified local-drive spelling, independent of the compilation host.
/// Execution is a separate capability owned by the Windows filesystem adapter.
#[derive(Clone, Debug)]
pub struct WindowsPath {
    pub drive: u8,
    pub parts: Vec<String>,
}
impl WindowsPath {
    /// Validate a native leaf before either grant admission or an NT open.
    pub fn validate_component(value: &str) -> io::Result<()> {
        let base = value
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
            || ["COM", "LPT"].iter().any(|prefix| {
                base.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            });
        if value.is_empty()
            || value == "."
            || value == ".."
            || value.contains(['/', '\\', ':', '\0', '*', '?', '"', '<', '>', '|'])
            || value.chars().any(char::is_control)
            || value.ends_with(['.', ' '])
            || device
        {
            return Err(refuse("not a native filesystem name"));
        }
        if value.encode_utf16().count() > 255 {
            return Err(refuse("filesystem name too long"));
        }
        Ok(())
    }

    /// Grants reject traversal; requests may normalize it before admission.
    pub fn parse(path: &str, normalize: bool) -> io::Result<Self> {
        let path = path.strip_prefix(r"\\?\").unwrap_or(path);
        let bytes = path.as_bytes();
        if bytes.len() < 3
            || !bytes[0].is_ascii_alphabetic()
            || bytes[1] != b':'
            || !matches!(bytes[2], b'/' | b'\\')
        {
            return Err(refuse("path must be a fully qualified local drive path"));
        }
        let mut parts = Vec::new();
        for part in path[3..].split(['/', '\\']) {
            match part {
                "" => continue,
                "." if normalize => continue,
                ".." if normalize => {
                    if parts.pop().is_none() {
                        return Err(refuse("path escapes its drive root"));
                    }
                }
                _ => {
                    Self::validate_component(part)?;
                    parts.push(part.to_owned());
                }
            }
        }
        Ok(Self {
            drive: bytes[0].to_ascii_uppercase(),
            parts,
        })
    }
    pub fn spelling(&self) -> String {
        self.prefix(self.parts.len())
    }
    pub fn prefix(&self, length: usize) -> String {
        format!("{}:/{}", self.drive as char, self.parts[..length].join("/"))
    }
    pub(crate) fn grant_components(self) -> Vec<String> {
        let mut result = vec![format!("win:{}", self.drive as char)];
        result.extend(self.parts);
        result
    }
}
