//! Private owned text transfer seam. No scheduling, wake or painter publication.
use super::catalog_recipe::Recipe;
pub(crate) use super::catalog_recipe::{CaptureCost, CatalogSnapshot};
use super::*;
use exact_kernel::{Offer, RegionTextRequest};
use std::sync::{Mutex, OnceLock, Weak};

#[cfg(test)]
pub(super) mod work {
    use std::cell::Cell;
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Counts {
        pub catalog_builds: usize,
        pub source_bytes: usize,
        pub shapes: usize,
        pub layouts: usize,
    }
    thread_local! { static COUNTS: Cell<Counts> = Cell::new(Counts::default()); }
    pub fn read() -> Counts {
        COUNTS.with(Cell::get)
    }
    pub fn add(f: impl FnOnce(&mut Counts)) {
        COUNTS.with(|c| {
            let mut n = c.get();
            f(&mut n);
            c.set(n)
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransferError {
    InvalidOffer,
    InvalidPaintContext,
    InkIndexRefused,
    CatalogMismatch,
    SourceMismatch,
    StaleResult,
    FontCapture,
    UnrepresentableCatalog,
    CatalogExhausted,
}
/// The scale used by CPU glyph rasterization, exactly bound to each job.
/// Origin/clip/transform are query inputs; palette/publication liveness remains
/// controller-owned. No global paint epoch or historical context registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaintContext {
    scale_bits: u32,
}
impl PaintContext {
    pub fn new(scale: f32) -> Result<Self, TransferError> {
        if !scale.is_finite() || scale <= 0. {
            return Err(TransferError::InvalidPaintContext);
        }
        Ok(Self {
            scale_bits: scale.to_bits(),
        })
    }
    pub fn scale(self) -> f32 {
        f32::from_bits(self.scale_bits)
    }
}
#[derive(Clone)]
pub(crate) struct FontRecipe(pub(super) Arc<Recipe>);
impl FontRecipe {
    pub fn catalog_label(&self) -> u64 {
        self.0.label
    }
    pub fn capture_cost(&self) -> CaptureCost {
        self.0.cost
    }
    pub fn mapped_face(&self, old: fontdb::ID) -> Option<fontdb::ID> {
        self.0.remap.get(&old).copied()
    }
}
pub(crate) struct RasterCatalog {
    recipe: Arc<Recipe>,
    catalog: catalog::Lease,
}
pub(super) struct Source {
    recipe: Arc<Recipe>,
    stamp: ParagraphStamp,
    spec: Arc<Spec>,
    pub(super) shape: OnceLock<(Arc<shaping::ShapeData>, usize)>,
    // One payload-free, replaceable weak slot; no request/job/catalog ownership.
    // Heights remain part of request admission, but this backend does not use
    // them to wrap definite-width text. Never use this key for publication.
    definite: Mutex<Option<Definite>>,
}
struct Definite {
    key: (u32, u32),
    payload: Weak<Layout>,
}
#[derive(Clone)]
pub(crate) struct PreparedSource(pub(super) Arc<Source>);
impl PreparedSource {
    pub fn shape_capacity_bytes(&self) -> usize {
        self.0.shape.get().map_or(0, |s| s.1)
    }
}
#[derive(Clone)]
pub(crate) struct PreparedText {
    job: Arc<()>,
    paint: PaintContext,
    request: RegionTextRequest,
    source: PreparedSource,
}
impl PreparedText {
    pub fn paint_context(&self) -> PaintContext {
        self.paint
    }
    pub fn request(&self) -> &RegionTextRequest {
        &self.request
    }
    pub fn source(&self) -> &PreparedSource {
        &self.source
    }
}
pub(super) struct Layout {
    pub(super) index: ink::Index,
    lines: Arc<Vec<Vec<cosmic_text::LayoutLine>>>,
    baselines: Arc<Vec<f32>>,
    metrics: TextMetrics,
    capacity: usize,
    #[cfg(test)]
    lifetime: Arc<()>,
}
pub(crate) struct CompletedText {
    input: PreparedText,
    metrics: TextMetrics,
    layout: Option<Arc<Layout>>,
    #[cfg(test)]
    pub(super) probe: std::sync::Weak<()>,
    #[cfg(test)]
    pub(super) ink_probe: std::sync::Weak<()>,
}
impl CompletedText {
    pub fn paint_context(&self) -> PaintContext {
        self.input.paint
    }
    pub fn ink_capacity_bytes(&self) -> usize {
        self.layout.as_ref().map_or(0, |l| l.index.bytes())
    }
    pub fn request(&self) -> &RegionTextRequest {
        self.input.request()
    }
    pub fn source(&self) -> &PreparedSource {
        self.input.source()
    }
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }
    pub fn layout_capacity_bytes(&self) -> usize {
        self.layout.as_ref().map_or(0, |l| {
            l.capacity
                .saturating_sub(self.input.source.0.shape.get().map_or(0, |s| s.1))
        })
    }
}
pub(crate) struct AdoptedText {
    paint: PaintContext,
    request: RegionTextRequest,
    metrics: TextMetrics,
    // Intrinsic results retain source/shape, not temporary width layouts.
    pub(super) source: PreparedSource,
    raster: catalog::Lease,
    paragraph: Option<Rc<Paragraph>>,
}
impl AdoptedText {
    pub fn paint_context(&self) -> PaintContext {
        self.paint
    }
    pub fn request(&self) -> &RegionTextRequest {
        &self.request
    }
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }
    pub fn paragraph(&self) -> Option<&Rc<Paragraph>> {
        self.paragraph.as_ref()
    }
}
/// UI metadata-only snapshot. Contains no UI-owned mutable font state.
pub(crate) fn snapshot_catalog(engine: &TextEngine) -> Result<CatalogSnapshot, TransferError> {
    CatalogSnapshot::capture(&engine.catalog.borrow())
}
/// Send-owned output of background font capture/construction. Not a UI Catalog.
pub(crate) struct PreparedCatalog {
    recipe: FontRecipe,
    raster_fonts: FontSystem,
}
impl PreparedCatalog {
    pub fn recipe(&self) -> &FontRecipe {
        &self.recipe
    }
}
/// Run after the pending shell, on the worker. No UI TextEngine is reachable.
pub(crate) fn prepare_catalog_generation(
    snapshot: CatalogSnapshot,
) -> Result<PreparedCatalog, TransferError> {
    let recipe = snapshot.prepare()?;
    let raster_fonts = recipe.fonts();
    Ok(PreparedCatalog {
        recipe: FontRecipe(recipe),
        raster_fonts,
    })
}
/// UI attachment only. No FontSystem constructor, file read, shape or layout.
pub(crate) fn adopt_catalog_generation(prepared: PreparedCatalog) -> (FontRecipe, RasterCatalog) {
    let PreparedCatalog {
        recipe,
        raster_fonts,
    } = prepared;
    let raster = RasterCatalog {
        catalog: Rc::new(RefCell::new(recipe.0.attach(raster_fonts))),
        recipe: recipe.0.clone(),
    };
    (recipe, raster)
}
fn valid(offer: Offer) -> bool {
    [offer.width, offer.height].iter().all(|axis| match axis {
        AxisOffer::Definite(v) => v.is_finite() && *v >= 0.,
        _ => true,
    })
}
pub(crate) fn prepare(
    recipe: &FontRecipe,
    request: RegionTextRequest,
    paint: PaintContext,
    reuse: Option<&PreparedSource>,
) -> Result<PreparedText, TransferError> {
    if request.catalog() != recipe.catalog_label() {
        return Err(TransferError::CatalogMismatch);
    }
    if !valid(request.offer()) {
        return Err(TransferError::InvalidOffer);
    }
    let source = match reuse {
        Some(source) => {
            if !Arc::ptr_eq(&recipe.0, &source.0.recipe)
                || !source.0.stamp.same_metrics(request.stamp())
            {
                return Err(TransferError::SourceMismatch);
            }
            source.clone()
        }
        None => PreparedSource(Arc::new(Source {
            recipe: recipe.0.clone(),
            stamp: request.stamp().clone(),
            spec: Arc::new(request.with_request(|r| {
                #[cfg(test)]
                work::add(|n| n.source_bytes += r.runs.iter().map(|r| r.text.len()).sum::<usize>());
                Spec::from_request(r)
            })),
            shape: OnceLock::new(),
            definite: Mutex::new(None),
        })),
    };
    Ok(PreparedText {
        job: Arc::new(()),
        paint,
        request,
        source,
    })
}
/// Created and used on the worker. Rc here is thread-local, never transported.
pub(crate) struct FontWorker {
    #[cfg(test)]
    pub(super) index_limit: usize,
    #[cfg(test)]
    pub(super) last_layout: std::sync::Weak<()>,
    recipe: FontRecipe,
    catalog: catalog::Lease,
}
impl FontWorker {
    pub fn new(recipe: FontRecipe) -> Result<Self, TransferError> {
        let catalog = Rc::new(RefCell::new(recipe.0.catalog()));
        Ok(Self {
            recipe,
            catalog,
            #[cfg(test)]
            index_limit: ink::MAX_BYTES,
            #[cfg(test)]
            last_layout: std::sync::Weak::new(),
        })
    }
    pub fn execute(&mut self, input: PreparedText) -> Result<CompletedText, TransferError> {
        if !Arc::ptr_eq(&self.recipe.0, &input.source.0.recipe) {
            return Err(TransferError::CatalogMismatch);
        }
        let source = &input.source.0;
        let key = match input.request.offer().width {
            AxisOffer::Definite(width) => Some((width.to_bits(), input.paint.scale_bits)),
            _ => None,
        };
        // Only lookup/upgrade under the lock. The strong result leaves the
        // scope before construction, return, or destruction can run.
        let hit = key.and_then(|key| {
            let slot = source.definite.lock().unwrap();
            slot.as_ref()
                .filter(|old| old.key == key)
                .and_then(|old| old.payload.upgrade())
        });
        if let Some(layout) = hit {
            #[cfg(test)]
            let probe = Arc::downgrade(&layout.lifetime);
            #[cfg(test)]
            let ink_probe = Arc::downgrade(&layout.index.lifetime);
            #[cfg(test)]
            {
                self.last_layout = probe.clone();
            }
            return Ok(CompletedText {
                metrics: layout.metrics,
                input,
                layout: Some(layout),
                #[cfg(test)]
                probe,
                #[cfg(test)]
                ink_probe,
            });
        }
        let (data, bytes) = source.shape.get_or_init(|| {
            #[cfg(test)]
            work::add(|n| n.shapes += 1);
            let built = ShapedSource::new(self.catalog.clone(), source.spec.clone());
            (built.data, built.accessible_capacity_bytes)
        });
        let shaped = Rc::new(ShapedSource::attach(
            self.catalog.clone(),
            source.spec.clone(),
            data.clone(),
            *bytes,
        ));
        let width = match input.request.offer().width {
            AxisOffer::Definite(w) => Some(w),
            AxisOffer::MaxContent => None,
            AxisOffer::MinContent => {
                let wrap = (source.spec.overflow_wrap == exact_kernel::OverflowWrap::BreakWord)
                    .then_some(Wrap::Word);
                #[cfg(test)]
                work::add(|n| n.layouts += 1);
                Some(shaped.layout(Some(0.), wrap).width)
            }
        };
        #[cfg(test)]
        work::add(|n| n.layouts += 1);
        let p = shaped.layout(width, None);
        let metrics = if source.spec.is_empty() {
            TextMetrics::default()
        } else {
            paragraph_metrics(&p)
        };
        #[cfg(test)]
        let probe = Arc::downgrade(&p.layout_lifetime);
        #[cfg(test)]
        {
            self.last_layout = probe.clone();
        }
        // Intrinsic probes publish scalar metrics + shared shape, never width
        // arrays or an index. Every successful definite result is paint-ready.
        let layout = if matches!(input.request.offer().width, AxisOffer::Definite(_)) {
            #[cfg(not(test))]
            let index = ink::Index::build(&mut self.catalog.borrow_mut(), &p, input.paint.scale());
            #[cfg(test)]
            let index = ink::Index::with_limit(
                &mut self.catalog.borrow_mut(),
                &p,
                input.paint.scale(),
                self.index_limit,
            );
            let index = index.ok_or(TransferError::InkIndexRefused)?;
            Some(Arc::new(Layout {
                index,
                metrics,
                lines: p.layouts,
                baselines: p.baselines,
                capacity: p.resident_capacity_bytes,
                #[cfg(test)]
                lifetime: p.layout_lifetime,
            }))
        } else {
            None
        };
        if let (Some(key), Some(layout)) = (key, &layout) {
            // Failed index preparation and intrinsic answers never seed a hit.
            // Retire even the old weak control block outside the lock.
            let old = source.definite.lock().unwrap().replace(Definite {
                key,
                payload: Arc::downgrade(layout),
            });
            drop(old);
        }
        #[cfg(test)]
        let ink_probe = layout
            .as_ref()
            .map_or_else(std::sync::Weak::new, |l| Arc::downgrade(&l.index.lifetime));
        Ok(CompletedText {
            input,
            metrics,
            layout,
            #[cfg(test)]
            probe,
            #[cfg(test)]
            ink_probe,
        })
    }
}
pub(crate) fn adopt(
    result: CompletedText,
    expected: &PreparedText,
    raster: &RasterCatalog,
) -> Result<AdoptedText, TransferError> {
    // Private identity binds the exact immutable request including BOTH axes,
    // full stamp/source, region ticket, recipe and exact paint context; no equivalence reconstruction.
    if !Arc::ptr_eq(&result.input.job, &expected.job) {
        return Err(TransferError::StaleResult);
    }
    if !Arc::ptr_eq(&result.input.source.0.recipe, &raster.recipe) {
        return Err(TransferError::CatalogMismatch);
    }
    let CompletedText {
        input,
        metrics,
        layout,
        ..
    } = result;
    let paragraph = layout.map(|l| {
        let (data, bytes) = input
            .source
            .0
            .shape
            .get()
            .expect("only execute constructs a completion");
        Rc::new(Paragraph {
            source: Rc::new(ShapedSource::attach(
                raster.catalog.clone(),
                input.source.0.spec.clone(),
                data.clone(),
                *bytes,
            )),
            layouts: l.lines.clone(),
            baselines: l.baselines.clone(),
            #[cfg(test)]
            layout_lifetime: l.lifetime.clone(),
            width: metrics.width,
            height: metrics.height,
            first_baseline: metrics.first_baseline.unwrap_or(0.),
            ink: RefCell::new(ink::Cache::from_index(
                &raster.catalog.borrow().ink_catalog,
                input.paint.scale(),
                l.clone(),
            )),
            resident_capacity_bytes: l.capacity,
            private_text_bytes_estimate: 0,
        })
    });
    Ok(AdoptedText {
        paint: input.paint,
        request: input.request,
        metrics,
        source: input.source,
        raster: raster.catalog.clone(),
        paragraph,
    })
}
