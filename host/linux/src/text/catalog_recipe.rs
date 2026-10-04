//! Explicit fresh-generation import. No mutation of an accepted old catalog.
use super::transfer::TransferError;
use super::*;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CATALOG: AtomicU64 = AtomicU64::new(1);

/// Metadata/Arc snapshot only: no font reads, parsing, or FontSystem creation.
pub(crate) struct CatalogSnapshot {
    constructor: fontdb::Database,
    final_db: fontdb::Database,
    locale: String,
    families: Vec<FamilyChoice>,
    declared: HashMap<(u16, u16, bool), fontdb::ID>,
    sans: String,
    cwd: PathBuf,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CaptureCost {
    pub faces: usize,
    pub sources: usize,
    pub file_reads: usize,
    pub file_bytes: usize,
    pub binary_bytes_copied: usize,
}
pub(super) struct Recipe {
    pub label: u64,
    constructor: fontdb::Database,
    final_db: fontdb::Database,
    locale: String,
    families: Vec<FamilyChoice>,
    declared: HashMap<(u16, u16, bool), fontdb::ID>,
    sans: String,
    pub remap: HashMap<fontdb::ID, fontdb::ID>,
    pub cost: CaptureCost,
}
impl CatalogSnapshot {
    pub(super) fn capture(catalog: &catalog::Catalog) -> Result<Self, TransferError> {
        Ok(Self {
            constructor: catalog.constructor_db.clone(),
            final_db: catalog.fonts.db().clone(),
            locale: catalog.fonts.locale().to_owned(),
            families: catalog.families.clone(),
            declared: catalog.declared_faces.clone(),
            sans: catalog.sans.clone(),
            cwd: std::env::current_dir().map_err(|_| TransferError::FontCapture)?,
        })
    }
    pub(super) fn prepare(self) -> Result<Arc<Recipe>, TransferError> {
        let initial: Vec<_> = self.constructor.faces().collect();
        let final_faces: Vec<_> = self.final_db.faces().collect();
        // Normal Catalog construction only appends faces. Removing/replacing
        // initial members cannot be replayed by silently changing their cache.
        if initial.len() > final_faces.len()
            || initial
                .iter()
                .zip(&final_faces)
                .any(|(a, b)| !same_face(a, b))
        {
            return Err(TransferError::UnrepresentableCatalog);
        }
        let mut sources = HashMap::new();
        let mut cost = CaptureCost::default();
        let mut remap = HashMap::new();
        let mut constructor = fontdb::Database::new();
        copy_generics(&self.constructor, &mut constructor);
        for face in initial {
            let imported = import_face(face, &self.cwd, &mut sources, &mut cost)?;
            remap.insert(face.id, constructor.push_face_info(imported));
        }
        let mut final_db = constructor.clone();
        for face in final_faces.iter().skip(self.constructor.len()) {
            let imported = import_face(face, &self.cwd, &mut sources, &mut cost)?;
            remap.insert(face.id, final_db.push_face_info(imported));
        }
        copy_generics(&self.final_db, &mut final_db);
        // FontMatchKey breaks otherwise equal matches by ID. Preserve that
        // ordering as well as database iteration order, or refuse atomically.
        let mut old_order: Vec<_> = remap.keys().copied().collect();
        old_order.sort();
        if old_order.windows(2).any(|w| remap[&w[0]] >= remap[&w[1]]) {
            return Err(TransferError::UnrepresentableCatalog);
        }
        let declared = self
            .declared
            .into_iter()
            .map(|(k, id)| {
                remap
                    .get(&id)
                    .copied()
                    .map(|new| (k, new))
                    .ok_or(TransferError::UnrepresentableCatalog)
            })
            .collect::<Result<_, _>>()?;
        cost.faces = final_db.len();
        cost.sources = sources.len();
        // One final metadata check detects replacement during multi-source
        // capture too. It never reads font bytes a second time.
        for source in sources.values() {
            for path in &source.paths {
                if source.file_identity != Some(path_identity(path)?) {
                    return Err(TransferError::FontCapture);
                }
            }
        }
        let label = NEXT_CATALOG
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| TransferError::CatalogExhausted)?;
        Ok(Arc::new(Recipe {
            label,
            constructor,
            final_db,
            locale: self.locale,
            families: self.families,
            declared,
            sans: self.sans,
            remap,
            cost,
        }))
    }
}
#[derive(Clone, Hash, PartialEq, Eq)]
enum SourceKey {
    File(u64, u64),
    Binary(usize),
}
struct Captured {
    bytes: Arc<dyn AsRef<[u8]> + Send + Sync>,
    indices: HashSet<u32>,
    file_identity: Option<FileIdentity>,
    paths: Vec<PathBuf>,
}
// Identity comes from the opened file, including Windows volume/file indices.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
#[cfg(unix)]
fn identity(file: &std::fs::File) -> Result<FileIdentity, TransferError> {
    let m = file.metadata().map_err(|_| TransferError::FontCapture)?;
    use std::os::unix::fs::MetadataExt;
    Ok(FileIdentity {
        dev: m.dev(),
        ino: m.ino(),
        len: m.len(),
        mtime: m.mtime(),
        mtime_ns: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_ns: m.ctime_nsec(),
    })
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn identity(file: &std::fs::File) -> Result<FileIdentity, TransferError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the borrowed File keeps its handle alive; info is writable.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(TransferError::FontCapture);
    }
    Ok(FileIdentity {
        dev: info.dwVolumeSerialNumber as u64,
        ino: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        len: ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64,
        mtime: info.ftLastWriteTime.dwHighDateTime as i64,
        mtime_ns: info.ftLastWriteTime.dwLowDateTime as i64,
        ctime: info.ftCreationTime.dwHighDateTime as i64,
        ctime_ns: info.ftCreationTime.dwLowDateTime as i64,
    })
}
fn path_identity(path: &Path) -> Result<FileIdentity, TransferError> {
    let file = crate::file::open_regular(path).map_err(|_| TransferError::FontCapture)?;
    identity(&file)
}
fn import_face(
    face: &fontdb::FaceInfo,
    cwd: &Path,
    sources: &mut HashMap<SourceKey, Captured>,
    cost: &mut CaptureCost,
) -> Result<fontdb::FaceInfo, TransferError> {
    let path = match &face.source {
        fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
            Some(if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            })
        }
        _ => None,
    };
    let observed = path.as_ref().map(|p| path_identity(p)).transpose()?;
    let key = match observed {
        Some(id) => SourceKey::File(id.dev, id.ino),
        None => {
            let fontdb::Source::Binary(bytes) = &face.source else {
                unreachable!()
            };
            SourceKey::Binary(Arc::as_ptr(bytes) as *const () as usize)
        }
    };
    if !sources.contains_key(&key) {
        let bytes = match &path {
            None => {
                let fontdb::Source::Binary(bytes) = &face.source else {
                    unreachable!()
                };
                let bytes = bytes.as_ref().as_ref().to_vec();
                cost.binary_bytes_copied += bytes.len();
                bytes
            }
            Some(path) => {
                // A captured regular path may have been replaced by a FIFO.
                // Never wait for its writer before checking the opened inode.
                let mut file =
                    crate::file::open_regular(path).map_err(|_| TransferError::FontCapture)?;
                let before = identity(&file)?;
                if Some(before) != observed {
                    return Err(TransferError::FontCapture);
                }
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)
                    .map_err(|_| TransferError::FontCapture)?;
                cost.file_reads += 1;
                cost.file_bytes += bytes.len();
                #[cfg(test)]
                after_read(path);
                let after = identity(&file)?;
                let named = path_identity(path)?;
                if before != after || after != named || bytes.len() as u64 != before.len {
                    return Err(TransferError::FontCapture);
                }
                bytes
            }
        };
        let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
        // Parse this backing ONCE, validate all referenced collection indices.
        // Authored aliases/style metadata are retained from the snapshot.
        let mut parsed = fontdb::Database::new();
        let ids = parsed.load_font_source(fontdb::Source::Binary(bytes.clone()));
        if ids.is_empty() {
            return Err(TransferError::FontCapture);
        }
        let indices = parsed.faces().map(|f| f.index).collect();
        sources.insert(
            key.clone(),
            Captured {
                bytes,
                indices,
                file_identity: observed,
                paths: Vec::new(),
            },
        );
    }
    let captured = sources.get_mut(&key).unwrap();
    if captured.file_identity != observed || !captured.indices.contains(&face.index) {
        return Err(TransferError::FontCapture);
    }
    // Distinct path names (including hardlinks) share one byte capture, but
    // every observed name is checked again before publishing the generation.
    if let Some(path) = path {
        if !captured.paths.contains(&path) {
            captured.paths.push(path);
        }
    }
    let mut imported = face.clone();
    imported.id = fontdb::ID::dummy();
    imported.source = fontdb::Source::Binary(captured.bytes.clone());
    Ok(imported)
}
fn same_face(a: &fontdb::FaceInfo, b: &fontdb::FaceInfo) -> bool {
    let source = match (&a.source, &b.source) {
        (fontdb::Source::Binary(a), fontdb::Source::Binary(b)) => Arc::ptr_eq(a, b),
        (
            fontdb::Source::File(a) | fontdb::Source::SharedFile(a, _),
            fontdb::Source::File(b) | fontdb::Source::SharedFile(b, _),
        ) => a == b,
        _ => false,
    };
    source
        && a.id == b.id
        && a.index == b.index
        && a.families == b.families
        && a.post_script_name == b.post_script_name
        && a.style == b.style
        && a.weight == b.weight
        && a.stretch == b.stretch
        && a.monospaced == b.monospaced
}
fn copy_generics(from: &fontdb::Database, to: &mut fontdb::Database) {
    to.set_serif_family(from.family_name(&fontdb::Family::Serif));
    to.set_sans_serif_family(from.family_name(&fontdb::Family::SansSerif));
    to.set_cursive_family(from.family_name(&fontdb::Family::Cursive));
    to.set_fantasy_family(from.family_name(&fontdb::Family::Fantasy));
    to.set_monospace_family(from.family_name(&fontdb::Family::Monospace));
}
impl Recipe {
    pub fn fonts(&self) -> FontSystem {
        #[cfg(test)]
        super::transfer::work::add(|n| n.catalog_builds += 1);
        let mut fonts =
            FontSystem::new_with_locale_and_db(self.locale.clone(), self.constructor.clone());
        *fonts.db_mut() = self.final_db.clone();
        fonts
    }
    pub fn attach(&self, fonts: FontSystem) -> catalog::Catalog {
        let mut catalog = catalog::Catalog::with_fonts(fonts);
        catalog.constructor_db = self.constructor.clone();
        catalog.families = self.families.clone();
        catalog.declared_faces = self.declared.clone();
        catalog.sans = self.sans.clone();
        catalog
    }
    pub fn catalog(&self) -> catalog::Catalog {
        self.attach(self.fonts())
    }
}
#[cfg(test)]
type AfterRead = Option<Box<dyn FnOnce(&Path)>>;
#[cfg(test)]
thread_local! { pub(super) static AFTER_READ: RefCell<AfterRead> = const { RefCell::new(None) }; }
#[cfg(test)]
fn after_read(path: &Path) {
    AFTER_READ.with(|f| {
        if let Some(f) = f.borrow_mut().take() {
            f(path)
        }
    });
}
