//! Immutable asset selection; decoded work stays in the raster workers.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Immutable named asset bytes supplied by an optional app adapter.
/// `Ok(None)` is a tombstone; an error refuses the current boot transaction.
pub type AssetResolver = Arc<dyn Fn(&str) -> Result<Option<Arc<[u8]>>, String> + Send + Sync>;

pub(super) enum ImageInput {
    /// Entry-zero files stay off the presenter thread.
    Path(PathBuf),
    /// A selected generation is verified before the boot fallback decision.
    Bytes(Arc<[u8]>),
}

/// One generation's asset resolver. Entry zero reads beneath the binary's
/// root; a selected update reads through its complete, verified asset resolver.
/// An absent selected name is a tombstone and never falls through to the
/// embedded root.
#[derive(Clone)]
pub struct Assets {
    root: PathBuf,
    selected: Option<AssetResolver>,
    refusal: Arc<Mutex<Option<String>>>,
}

impl Assets {
    /// Resolve entry zero beneath `root`.
    pub fn embedded(root: PathBuf) -> Assets {
        Assets {
            root,
            selected: None,
            refusal: Arc::new(Mutex::new(None)),
        }
    }

    /// Resolve the selected generation through its complete signed roster.
    pub fn selected(root: PathBuf, selected: AssetResolver) -> Assets {
        Assets {
            root,
            selected: Some(selected),
            refusal: Arc::new(Mutex::new(None)),
        }
    }

    /// The embedded asset root, retained for diagnostics and entry zero.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn is_selected(&self) -> bool {
        self.selected.is_some()
    }

    fn relative(name: &str) -> bool {
        !name.is_empty()
            && !name.starts_with('/')
            && !name.starts_with('\\')
            && !name.contains('\\')
            && !name.contains(':')
            && name
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
    }

    /// Immutable bytes for one relative asset name. A selected generation's
    /// absent name returns `None`, even when entry zero contains that name.
    pub fn read(&self, name: &str) -> Option<Arc<[u8]>> {
        match self.image_input(name)? {
            ImageInput::Path(path) => std::fs::read(path).ok().map(Arc::from),
            ImageInput::Bytes(bytes) => Some(bytes),
        }
    }

    /// A module's one-shot asset delivery; errors remain distinct from tombstones.
    pub fn read_asset(&self, name: &str) -> Result<Option<Arc<[u8]>>, String> {
        if !Self::relative(name) {
            return Err("invalid asset name".into());
        }
        if let Some(selected) = &self.selected {
            return selected(name);
        }
        let Some(path) = self.path(name) else {
            return Ok(None);
        };
        std::fs::read(path)
            .map(|b| Some(Arc::from(b)))
            .map_err(|e| e.to_string())
    }

    pub(super) fn image_input(&self, name: &str) -> Option<ImageInput> {
        // An `app:/tmp`, `app:/cache` or `app:/data` source is the app's own
        // file, read by the host (LLP 1069.002 D7): a picked photo's preview.
        if name.starts_with("app:/") {
            return crate::picker::resolve(name).map(ImageInput::Path);
        }
        // A `data:` URL, as a page's `<img>` takes one, to its bound (LLP 1011 §2).
        if name.starts_with("data:") {
            return data_url(name).map(|bytes| ImageInput::Bytes(Arc::from(bytes)));
        }
        if !Self::relative(name) {
            return None;
        }
        if let Some(selected) = &self.selected {
            return match selected(name) {
                Ok(Some(bytes)) => Some(ImageInput::Bytes(bytes)),
                Ok(None) => None,
                Err(reason) => {
                    let mut refusal = self.refusal.lock().unwrap_or_else(|e| e.into_inner());
                    refusal.get_or_insert(reason);
                    None
                }
            };
        }
        self.path(name).map(ImageInput::Path)
    }

    /// Take the first selected-file integrity refusal observed by this
    /// resolver. Hosts use a boot-time refusal to fall back transactionally;
    /// a later one is surfaced without retitling an already-running session.
    pub fn take_refusal(&self) -> Option<String> {
        self.refusal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    /// Where an entry-zero source resolves, for diagnostics and tests. A
    /// selected generation deliberately exposes bytes rather than raw paths.
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        if self.selected.is_some() || !Self::relative(name) {
            return None;
        }
        let root = self.root.canonicalize().ok()?;
        let path = root.join(name).canonicalize().ok()?;
        path.starts_with(&root).then_some(path)
    }
}

/// A `data:` URL's bytes (RFC 2397): base64 after `;base64`, else
/// percent-decoded; whitespace and escapes in base64 are forgiven, as
/// browsers forgive them. `None` past `exact_raster::MAX_DATA_URL_BYTES`.
pub(super) fn data_url(name: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    if name.len() > exact_raster::MAX_DATA_URL_BYTES {
        return None;
    }
    let (meta, body) = name.strip_prefix("data:")?.split_once(',')?;
    let (body, mut bytes, mut i) = (body.as_bytes(), Vec::new(), 0);
    while i < body.len() {
        let hex = body
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match hex
            .filter(|_| body[i] == b'%')
            .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            Some(byte) => (bytes.push(byte), i += 3),
            None => (bytes.push(body[i]), i += 1),
        };
    }
    if !meta.to_ascii_lowercase().ends_with(";base64") {
        return Some(bytes);
    }
    bytes.retain(|b| !b.is_ascii_whitespace() && *b != b'=');
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(bytes)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page's `<img>` takes these; so does every native host, to its bound.
    #[test]
    fn data_urls_decode_to_their_bytes_within_the_bound() {
        let assets = Assets::embedded(PathBuf::from("/nonexistent"));
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let Some(ImageInput::Bytes(bytes)) = assets.image_input(png) else {
            panic!("a base64 PNG is bytes")
        };
        assert!(bytes.starts_with(b"\x89PNG") && bytes.len() == 70);
        // Unpadded, wrapped and escaped base64; a percent-encoded SVG.
        assert_eq!(data_url("data:;base64,aGk%3D"), Some(b"hi".to_vec()));
        assert_eq!(data_url("data:;BASE64,aG\n k"), Some(b"hi".to_vec()));
        assert_eq!(
            data_url("data:image/svg+xml,%3Csvg%3E <"),
            Some(b"<svg> <".to_vec())
        );
        assert_eq!(data_url("data:image/png;base64"), None);
        let over = format!("data:,{}", "a".repeat(exact_raster::MAX_DATA_URL_BYTES));
        assert!(assets.image_input(&over).is_none());
    }
}
