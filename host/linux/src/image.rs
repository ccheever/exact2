//! Images: sources resolved under the asset root, PNG decoded off the main
//! thread, sizes reported to the kernel (LLP 1011 §4's policy, in Rust).
//!
//! @ref LLP 1015 §4; LLP 1011 §4
//!
//! A relative source resolves under `EXACT_ASSETS` (the current directory
//! otherwise) and must stay inside it; `http(s)` waits for an out-of-process
//! resource (`QUEUE.md`, ibex2) and does not load; anything else does not
//! load. A load that completes for an older generation, or for a node that
//! is gone, is dropped. The old picture and its size stay until the new one
//! has loaded, as a browser keeps showing the old `src`.

use exact_kernel::{Kernel, NodeType, PropId, ViewId};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tiny_skia::{IntSize, Pixmap};

/// Immutable named asset bytes supplied by an optional app adapter.
/// `Ok(None)` is a tombstone; an error refuses the current boot transaction.
pub type AssetResolver = Arc<dyn Fn(&str) -> Result<Option<Arc<[u8]>>, String> + Send + Sync>;

struct Loaded {
    view: ViewId,
    generation: u64,
    source: String,
    pixels: Option<(Vec<u8>, u32, u32)>,
}

enum ImageInput {
    /// Entry-zero and legacy activation files stay off the presenter thread.
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

    fn image_input(&self, name: &str) -> Option<ImageInput> {
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

/// Every image node's source, picture, and load in flight.
pub struct Images {
    assets: Assets,
    /// Legacy activation overlays. Activation still uses the pre-generation
    /// core API; launch-selected assets never enter this map.
    overrides: BTreeMap<String, PathBuf>,
    sources: BTreeMap<ViewId, String>,
    generation: BTreeMap<ViewId, u64>,
    /// Decoded pictures, premultiplied RGBA.
    pub bitmaps: BTreeMap<ViewId, Rc<Pixmap>>,
    /// Sources loaded since launch and their pixel sizes (smoke reporting).
    pub loaded: Vec<(String, (u32, u32))>,
    next: u64,
    pending: usize,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
}

/// A size reported to the kernel: the view and its intrinsic size, or
/// `None` when the source did not load.
pub type Report = (ViewId, Option<(f32, f32)>);

impl Images {
    /// With the asset root.
    pub fn new(assets: PathBuf) -> Images {
        Images::with_assets(Assets::embedded(assets))
    }

    /// With one generation-scoped asset resolver.
    pub(crate) fn with_assets(assets: Assets) -> Images {
        let (tx, rx) = channel();
        Images {
            assets,
            overrides: BTreeMap::new(),
            sources: BTreeMap::new(),
            generation: BTreeMap::new(),
            bitmaps: BTreeMap::new(),
            loaded: Vec::new(),
            next: 0,
            pending: 0,
            tx,
            rx,
        }
    }

    /// Where a source resolves, as a page resolves `src`: a relative path
    /// under the asset root and never outside it; `None` otherwise.
    pub fn resolve(&self, source: &str) -> Option<PathBuf> {
        if source.contains("://") || source.starts_with('/') {
            return None;
        }
        self.overrides
            .get(source)
            .cloned()
            .or_else(|| self.assets.path(source))
    }

    fn input(&self, source: &str) -> Option<ImageInput> {
        self.overrides
            .get(source)
            .cloned()
            .map(ImageInput::Path)
            .or_else(|| self.assets.image_input(source))
    }

    /// After a commit: start a load for every image node whose source is
    /// new, forget the nodes that are gone. Sources that cannot load are
    /// reported at once.
    pub fn sync(&mut self, kernel: &Kernel, live: &[ViewId]) -> Vec<Report> {
        let mut reports = Vec::new();
        let mut seen = Vec::new();
        for id in live {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            if node.node_type != NodeType::Image {
                continue;
            }
            seen.push(*id);
            let source = node
                .props
                .str(PropId::ImageSource)
                .unwrap_or("")
                .to_string();
            if self.sources.get(id) == Some(&source) {
                continue;
            }
            self.sources.insert(*id, source.clone());
            self.next += 1;
            let generation = self.next;
            self.generation.insert(*id, generation);
            if source.is_empty() {
                self.bitmaps.remove(id);
                reports.push((*id, None));
                continue;
            }
            let Some(input) = self.input(&source) else {
                eprintln!("exact: image {source} is not a loadable source");
                self.bitmaps.remove(id);
                reports.push((*id, None));
                continue;
            };
            let tx = self.tx.clone();
            let view = *id;
            self.pending += 1;
            std::thread::spawn(move || {
                let pixels = match input {
                    ImageInput::Path(path) => {
                        std::fs::read(path).ok().and_then(|bytes| decode(&bytes))
                    }
                    ImageInput::Bytes(bytes) => decode(&bytes),
                };
                let _ = tx.send(Loaded {
                    view,
                    generation,
                    source,
                    pixels,
                });
            });
        }
        let gone: Vec<ViewId> = self
            .sources
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.sources.remove(&id);
            self.generation.remove(&id);
            self.bitmaps.remove(&id);
        }
        reports
    }

    /// Loads that arrived: keep each current one's picture and report its
    /// size; a stale or failed one clears nothing but reports the failure.
    pub fn poll(&mut self) -> Vec<Report> {
        let mut reports = Vec::new();
        while let Ok(l) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            if self.generation.get(&l.view) != Some(&l.generation) {
                continue;
            }
            match l.pixels {
                Some((data, w, h)) => {
                    if let Some(px) = IntSize::from_wh(w, h).and_then(|s| Pixmap::from_vec(data, s))
                    {
                        self.bitmaps.insert(l.view, Rc::new(px));
                        self.loaded.push((l.source, (w, h)));
                        reports.push((l.view, Some((w as f32, h as f32))));
                        continue;
                    }
                    reports.push((l.view, None));
                }
                None => {
                    eprintln!("exact: image {} did not load", l.source);
                    self.bitmaps.remove(&l.view);
                    reports.push((l.view, None));
                }
            }
        }
        reports
    }

    /// Whether a load is in flight.
    pub fn pending(&self) -> bool {
        self.pending > 0
    }

    /// Block until every load in flight has arrived, or `timeout` passes;
    /// the reports.
    pub fn wait(&mut self, timeout: Duration) -> Vec<Report> {
        let started = Instant::now();
        let mut reports = Vec::new();
        while self.pending > 0 && started.elapsed() < timeout {
            match self
                .rx
                .recv_timeout(timeout.saturating_sub(started.elapsed()))
            {
                Ok(l) => {
                    // Put it back through the one path that interprets it.
                    let _ = self.tx.send(l);
                    reports.extend(self.poll());
                }
                Err(_) => break,
            }
        }
        reports
    }

    /// A restart: every picture goes.
    pub fn reset(&mut self) {
        self.sources.clear();
        self.generation.clear();
        self.bitmaps.clear();
        self.loaded.clear();
    }

    /// The asset root.
    pub fn assets(&self) -> &Path {
        self.assets.root()
    }

    /// Every file under `dir` stands in for its relative name during the
    /// legacy activation path. The generation-token activation API will
    /// replace this adapter together with its commit-before-accept behavior.
    pub fn use_overrides(&mut self, dir: &Path) {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(dir) {
                    self.overrides
                        .insert(rel.to_string_lossy().into_owned(), path.clone());
                }
            }
        }
    }
}

/// Decode a PNG completely into premultiplied RGBA; `None` when the bytes
/// are not a PNG (or have no pixels).
pub fn decode(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 {
        return None;
    }
    let src = &buf[..info.buffer_size()];
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    let push = |out: &mut Vec<u8>, r: u8, g: u8, b: u8, a: u8| {
        let p = |c: u8| (c as u32 * a as u32 / 255) as u8;
        out.extend_from_slice(&[p(r), p(g), p(b), a]);
    };
    match info.color_type {
        png::ColorType::Rgba => {
            for px in src.chunks_exact(4) {
                push(&mut out, px[0], px[1], px[2], px[3]);
            }
        }
        png::ColorType::Rgb => {
            for px in src.chunks_exact(3) {
                push(&mut out, px[0], px[1], px[2], 255);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for px in src.chunks_exact(2) {
                push(&mut out, px[0], px[0], px[0], px[1]);
            }
        }
        png::ColorType::Grayscale => {
            for px in src {
                push(&mut out, *px, *px, *px, 255);
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some((out, w, h))
}
