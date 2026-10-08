//! Explicit fresh-generation import. No mutation of an accepted old catalog.
//!
//! A worker's catalog shares the UI catalog's faces outright (LLP 1085.000
//! §4, the storage spike's Q2): every face is a blob over its file, mapped
//! once and read by both threads, and a shaped run carries the face it was
//! shaped with. Nothing is read, copied or renumbered to cross the thread.
use super::transfer::TransferError;
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CATALOG: AtomicU64 = AtomicU64::new(1);

/// The UI catalog's faces and plan choices: clones of shared handles, no
/// font reads, parsing or catalog construction.
pub(crate) struct CatalogSnapshot {
    collection: fontique::Collection,
    files: Arc<HashMap<u64, Arc<std::path::Path>>>,
    locale: String,
    families: Vec<FamilyChoice>,
    declared: HashMap<(u16, u16, bool), catalog::FaceId>,
    sans: String,
}
/// What preparing a generation cost: faces and the files behind them. No
/// font byte is read or copied.
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
    collection: fontique::Collection,
    files: Arc<HashMap<u64, Arc<std::path::Path>>>,
    locale: String,
    families: Vec<FamilyChoice>,
    declared: HashMap<(u16, u16, bool), catalog::FaceId>,
    sans: String,
    pub cost: CaptureCost,
}
impl CatalogSnapshot {
    pub(super) fn capture(catalog: &catalog::Catalog) -> Result<Self, TransferError> {
        Ok(Self {
            collection: catalog.fonts.collection.clone(),
            files: catalog.files.clone(),
            locale: catalog.locale.clone(),
            families: catalog.families.clone(),
            declared: catalog.declared_faces.clone(),
            sans: catalog.sans.clone(),
        })
    }
    pub(super) fn prepare(mut self) -> Result<Arc<Recipe>, TransferError> {
        let label = NEXT_CATALOG
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| TransferError::CatalogExhausted)?;
        let names = super::fonts::family_names(&mut self.collection);
        let faces = names
            .iter()
            .filter_map(|n| self.collection.family_by_name(n))
            .map(|f| f.fonts().len())
            .sum();
        let cost = CaptureCost {
            faces,
            sources: self.files.len(),
            ..CaptureCost::default()
        };
        Ok(Arc::new(Recipe {
            label,
            collection: self.collection,
            files: self.files,
            locale: self.locale,
            families: self.families,
            declared: self.declared,
            sans: self.sans,
            cost,
        }))
    }
}
impl Recipe {
    /// The faces for one more catalog: a copy of the shared collection.
    pub fn fonts(&self) -> parley::FontContext {
        #[cfg(test)]
        super::transfer::work::add(|n| n.catalog_builds += 1);
        parley::FontContext {
            collection: self.collection.clone(),
            source_cache: fontique::SourceCache::default(),
        }
    }
    pub fn attach(&self, fonts: parley::FontContext) -> catalog::Catalog {
        let mut catalog = catalog::Catalog::attach(fonts, self.files.clone(), &self.locale);
        catalog.families = self.families.clone();
        catalog.declared_faces = self.declared.clone();
        catalog.sans = self.sans.clone();
        catalog
    }
    pub fn catalog(&self) -> catalog::Catalog {
        self.attach(self.fonts())
    }
}
