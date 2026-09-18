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

    pub(super) fn image_input(&self, name: &str) -> Option<ImageInput> {
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
