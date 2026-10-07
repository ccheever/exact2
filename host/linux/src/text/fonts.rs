//! The faces text lays out with (LLP 1085.000 §4, G7): found in the system's
//! font directories and `EXACT_FONTS` without fontconfig, registered with
//! fontique as faces described ahead of time, their files mapped only when a
//! shape first reads them.
//!
//! Every face is a memory source over a lazily mapped file, never a path
//! source: the face's blob then names its file (the Canvas host's `FONT` op
//! needs it, LLP 1076 §3.3), and two catalogs or threads that share a blob
//! share one mapping, so what was measured is what is drawn.
use fontique::{
    Blob, Collection, CollectionOptions, FontInfo, FontStyle, FontWeight, FontWidth, GenericFamily,
    SourceId, SourceInfo, SourceKind,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// A font file, mapped the first time its bytes are read. A file that cannot
/// be mapped reads as empty, which no face parses: fallback goes on.
pub(super) struct MappedFile {
    path: Arc<Path>,
    map: OnceLock<Option<memmap2::Mmap>>,
}

impl AsRef<[u8]> for MappedFile {
    fn as_ref(&self) -> &[u8] {
        self.map
            .get_or_init(|| map(&self.path))
            .as_deref()
            .unwrap_or(&[])
    }
}

#[allow(unsafe_code)]
fn map(path: &Path) -> Option<memmap2::Mmap> {
    let file = crate::file::open_regular(path).ok()?;
    // SAFETY: the mapping is read-only and private to this process. A font
    // file rewritten in place while mapped is outside what any text engine
    // can defend against (fontdb, which this replaces, mapped them too);
    // replacing it by rename, as package managers do, leaves this mapping on
    // the old inode.
    unsafe { memmap2::Mmap::map(&file) }.ok()
}

/// A face's file as a blob, not yet mapped.
pub(super) fn file_blob(path: Arc<Path>) -> Blob<u8> {
    Blob::new(Arc::new(MappedFile {
        path,
        map: OnceLock::new(),
    }))
}

/// One face as a font directory lists it: enough to match it without
/// opening its file.
#[derive(Clone, Debug)]
pub(super) struct Described {
    pub family: String,
    pub path: Arc<Path>,
    pub index: u32,
    pub width: FontWidth,
    pub style: FontStyle,
    pub weight: FontWeight,
    pub axes: Vec<fontique::AxisInfo>,
    pub charmap: fontique::CharmapIndex,
}

impl Described {
    /// The described face over `blob` (the one blob of its file).
    pub fn info(&self, source: &SourceInfo) -> FontInfo {
        FontInfo::from_parts(
            source.clone(),
            self.index,
            self.width,
            self.style,
            self.weight,
            &self.axes,
            self.charmap,
        )
    }
}

/// The faces installed when the process first lays out text: the system's
/// font directories and `EXACT_FONTS`. Read once; every catalog copies it.
pub(super) struct Installed {
    pub collection: Collection,
    /// Each file's blob id and path, for the Canvas host (LLP 1076 §3.3).
    pub files: HashMap<u64, Arc<Path>>,
    /// What fontconfig's own files name for the generic families, else
    /// fontdb's defaults (the host's sans, serif and monospace choices
    /// start from them, as they did over fontdb).
    pub generics: Generics,
}

/// The generic families a platform's configuration names.
#[derive(Clone, Debug)]
pub(super) struct Generics {
    pub sans: String,
    pub serif: String,
    pub mono: String,
}

impl Default for Generics {
    fn default() -> Self {
        Self {
            sans: "Arial".into(),
            serif: "Times New Roman".into(),
            mono: "Courier New".into(),
        }
    }
}

/// An empty collection: no system font API, no shared store.
pub(super) fn empty_collection() -> Collection {
    Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    })
}

/// The process's installed faces.
pub(super) fn installed() -> &'static Installed {
    static INSTALLED: OnceLock<Installed> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let (mut dirs, generics) = system_dirs();
        if let Ok(dir) = std::env::var("EXACT_FONTS") {
            dirs.push(PathBuf::from(dir));
        }
        let mut registry = Registry::default();
        for dir in &dirs {
            registry.add(super::font_cache::load(dir));
        }
        if registry.faces == 0 {
            eprintln!("exact: no fonts found; text will not shape (set EXACT_FONTS to a directory of .ttf files)");
        }
        Installed {
            collection: registry.collection,
            files: registry.files,
            generics,
        }
    })
}

/// Faces being registered: one blob per file, faces grouped by family.
pub(super) struct Registry {
    pub collection: Collection,
    pub files: HashMap<u64, Arc<Path>>,
    pub sources: HashMap<Arc<Path>, SourceInfo>,
    pub faces: usize,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            collection: empty_collection(),
            files: HashMap::new(),
            sources: HashMap::new(),
            faces: 0,
        }
    }
}

impl Registry {
    /// The one source of `path`'s blob.
    pub fn source(&mut self, path: &Arc<Path>) -> SourceInfo {
        if let Some(source) = self.sources.get(path) {
            return source.clone();
        }
        let blob = file_blob(path.clone());
        self.files.insert(blob.id(), path.clone());
        let source = SourceInfo::new(SourceId::new(), SourceKind::Memory(blob));
        self.sources.insert(path.clone(), source.clone());
        source
    }

    /// Register a directory's faces, each family's together. A file listed
    /// by two directories (a link) registers once.
    pub fn add(&mut self, faces: Vec<Described>) {
        let before: std::collections::HashSet<Arc<Path>> = self.sources.keys().cloned().collect();
        let mut families: Vec<(String, Vec<FontInfo>)> = Vec::new();
        for face in faces {
            if before.contains(&face.path) {
                continue;
            }
            let source = self.source(&face.path);
            let info = face.info(&source);
            match families.iter_mut().find(|(n, _)| *n == face.family) {
                Some((_, v)) => v.push(info),
                None => families.push((face.family, vec![info])),
            }
        }
        for (family, fonts) in families {
            self.faces += fonts.len();
            self.collection.register_described(&family, fonts);
        }
    }
}

/// The directories fontdb scanned for system fonts, and the generic families
/// fontconfig's files name (Linux; the defaults elsewhere).
fn system_dirs() -> (Vec<PathBuf>, Generics) {
    let mut dirs = Vec::new();
    #[allow(unused_mut)]
    let mut generics = Generics::default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "windows") {
        let root = std::env::var_os("SYSTEMROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        dirs.push(root.join("Fonts"));
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            dirs.push(profile.join("AppData\\Local\\Microsoft\\Windows\\Fonts"));
            dirs.push(profile.join("AppData\\Roaming\\Microsoft\\Windows\\Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.push("/Library/Fonts".into());
        dirs.push("/System/Library/Fonts".into());
        if let Ok(assets) = std::fs::read_dir("/System/Library/AssetsV2") {
            for entry in assets.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("com_apple_MobileAsset_Font")
                {
                    dirs.push(entry.path());
                }
            }
        }
        dirs.push("/Network/Library/Fonts".into());
        if let Some(home) = &home {
            dirs.push(home.join("Library/Fonts"));
        }
    } else if cfg!(target_os = "linux") {
        #[cfg(target_os = "linux")]
        if let Some((found, named)) = fontconfig(home.as_deref()) {
            return (found, named);
        }
        dirs.push("/usr/share/fonts/".into());
        dirs.push("/usr/local/share/fonts/".into());
        if let Some(home) = &home {
            dirs.push(home.join(".fonts"));
            dirs.push(home.join(".local/share/fonts"));
        }
    }
    (dirs, generics)
}

/// fontconfig's configuration, read as files (no library): its font
/// directories and the families its aliases prefer, the last alias for a
/// generic winning, as fontdb read them.
#[cfg(target_os = "linux")]
fn fontconfig(home: Option<&Path>) -> Option<(Vec<PathBuf>, Generics)> {
    let mut config = fontconfig_parser::FontConfig::default();
    if let Ok(file) = std::env::var("FONTCONFIG_FILE") {
        let _ = config.merge_config(Path::new(&file));
    } else {
        let xdg = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")));
        let read_global = match xdg {
            Some(p) => config
                .merge_config(&p.join("fontconfig/fonts.conf"))
                .is_err(),
            None => true,
        };
        if read_global {
            let _ = config.merge_config(Path::new("/etc/fonts/local.conf"));
        }
        let _ = config.merge_config(Path::new("/etc/fonts/fonts.conf"));
    }
    let mut generics = Generics::default();
    for alias in &config.aliases {
        let name = alias
            .prefer
            .first()
            .or_else(|| alias.accept.first())
            .or_else(|| alias.default.first());
        if let Some(name) = name {
            match alias.alias.to_lowercase().as_str() {
                "serif" => generics.serif = name.clone(),
                "sans-serif" | "sans serif" => generics.sans = name.clone(),
                "monospace" => generics.mono = name.clone(),
                _ => {}
            }
        }
    }
    if config.dirs.is_empty() {
        return None;
    }
    let dirs = config
        .dirs
        .iter()
        .filter_map(|dir| {
            if dir.path.starts_with("~") {
                Some(home?.join(dir.path.strip_prefix("~").ok()?))
            } else {
                Some(dir.path.clone())
            }
        })
        .collect();
    Some((dirs, generics))
}

/// Whether `family` is installed in `collection` (fontique's names are
/// case-insensitive, as fontdb's matching was here).
pub(super) fn installed_family(collection: &mut Collection, family: &str) -> bool {
    collection.family_id(family).is_some()
}

/// Whether `family`'s regular face is fixed-pitch (`post.isFixedPitch`, as
/// fontdb reported it). Maps that face's file.
pub(super) fn monospaced(collection: &mut Collection, family: &str) -> bool {
    let Some(info) = collection.family_by_name(family) else {
        return false;
    };
    info.fonts()
        .iter()
        .filter(|f| f.style() == FontStyle::Normal)
        .chain(info.fonts())
        .find_map(|f| {
            let blob = f.load(None)?;
            let font = swash::FontRef::from_index(blob.data(), f.index() as usize)?;
            Some(font.metrics(&[]).is_monospace)
        })
        .unwrap_or(false)
}

/// Point a generic family at `name` when it is installed.
pub(super) fn set_generic(collection: &mut Collection, generic: GenericFamily, name: &str) {
    if let Some(id) = collection.family_id(name) {
        collection.set_generic_families(generic, std::iter::once(id));
    }
}

/// Every installed family name, sorted (the last resort of fallback).
pub(super) fn family_names(collection: &mut Collection) -> Vec<String> {
    let mut names: Vec<String> = collection.family_names().map(str::to_owned).collect();
    names.sort();
    names
}
