//! One process service, one admitted region, one exact ready result per UI turn.
//! Only Send messages cross threads. FontWorker and its Rc catalog are created
//! and destroyed INSIDE the persistent service. New catalog capture is queued
//! only after the first actual placeholder frame, never hidden in registration.
use super::{source::PaintSource, worker};
use crate::text::{transfer::*, Shared};
use exact_kernel::{Kernel, ParagraphStamp, RegionTextRequest};
use std::{os::unix::io::RawFd, rc::Rc, sync::Arc};

const SOURCE_SLOTS: usize = exact_kernel::region::REGION_OFFERS;
static SERVICE: worker::ThreadSlot<FontService> = worker::ThreadSlot::new();

enum Job {
    Initialize(Box<CatalogSnapshot>),
    Text(PreparedText),
}
enum Reply {
    Initialized(Result<Box<PreparedCatalog>, TransferError>),
    Text(Result<CompletedText, TransferError>),
}
struct FontService(Option<FontWorker>);
impl worker::TextWork for FontService {
    type Input = Job;
    type Output = Reply;
    fn execute(&mut self, job: Job) -> Reply {
        #[cfg(test)]
        if matches!(job, Job::Text(_)) {
            super::test_hooks::before_text();
        }
        match job {
            Job::Initialize(snapshot) => Reply::Initialized((|| {
                let catalog = prepare_catalog_generation(*snapshot)?;
                let worker = FontWorker::new(catalog.recipe().clone())?;
                self.0 = Some(worker);
                Ok(Box::new(catalog))
            })()),
            Job::Text(input) => Reply::Text(match &mut self.0 {
                Some(worker) => worker.execute(input),
                None => Err(TransferError::CatalogMismatch),
            }),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Phase {
    AwaitingFirstPaint,
    Initializing,
    Ready,
    Refused(String),
}
struct SourceSlot {
    stamp: ParagraphStamp,
    source: PreparedSource,
    paint: Arc<PaintSource>,
}
struct Expected {
    serial: u64,
    input: PreparedText,
    paint: Arc<PaintSource>,
}
/// Opaque kernel artifact, including intrinsic source-only answers. The final
/// definite paint offer alone owns a paragraph. Old publications keep their old
/// catalog, source ranges, links and palette without consulting the live tree.
pub(crate) struct NativeText {
    pub(super) adopted: AdoptedText,
    pub(super) paint: Arc<PaintSource>,
}
impl NativeText {
    pub(crate) fn paint_context(&self) -> PaintContext {
        self.adopted.paint_context()
    }
    pub(crate) fn paragraph(&self) -> Option<&Rc<crate::text::Paragraph>> {
        self.adopted.paragraph()
    }
    pub(crate) fn palette(&self, dark: bool) -> std::borrow::Cow<'_, [crate::text::RunPaint]> {
        self.paint.palette(dark)
    }
    pub(crate) fn runs(&self) -> &[super::TextSourceRun] {
        &self.paint.runs
    }
    pub(crate) fn matches(&self, request: &RegionTextRequest) -> bool {
        Arc::ptr_eq(&self.paint.source, request.source())
    }
}

pub(super) struct Controller {
    port: worker::Port<Job, Reply>,
    phase: Phase,
    initializing: Option<u64>,
    recipe: Option<FontRecipe>,
    raster: Option<RasterCatalog>,
    expected: Option<Expected>,
    sources: Vec<SourceSlot>,
    paint: Option<PaintContext>,
}
impl Controller {
    pub(super) fn admit() -> Result<Self, String> {
        let port = SERVICE
            .start(|| FontService(None))
            .map_err(|e| format!("content font admission refused: {e}"))?;
        Ok(Self {
            port,
            phase: Phase::AwaitingFirstPaint,
            initializing: None,
            recipe: None,
            raster: None,
            expected: None,
            sources: Vec::new(),
            paint: None,
        })
    }
    pub(super) fn phase(&self) -> &Phase {
        &self.phase
    }
    pub(super) fn fd(&self) -> RawFd {
        self.port.fd()
    }
    pub(super) fn counts(&self) -> worker::Counts {
        self.port.counts()
    }
    pub(super) fn catalog(&self) -> Option<u64> {
        self.recipe.as_ref().map(FontRecipe::catalog_label)
    }
    pub(super) fn validate_scale(&self, scale: f32) -> Result<(), String> {
        let paint =
            PaintContext::new(scale).map_err(|e| format!("content raster context: {e:?}"))?;
        if self.paint.is_some_and(|old| old != paint) {
            return Err("content-region DPR changed; new registration required".into());
        }
        Ok(())
    }
    pub(super) fn configure_scale(&mut self, scale: f32) -> Result<(), String> {
        let result = self.validate_scale(scale);
        self.record_refusal(result)?;
        self.paint = Some(PaintContext::new(scale).map_err(|e| format!("{e:?}"))?);
        Ok(())
    }
    pub(super) fn first_painted(&mut self, text: &Shared, scale: f32) -> Result<(), String> {
        self.configure_scale(scale)?;
        if self.phase != Phase::AwaitingFirstPaint {
            return Ok(());
        }
        let result = (|| {
            let snapshot = snapshot_catalog(&text.borrow()).map_err(|e| format!("{e:?}"))?;
            let serial = self.port.submit(Job::Initialize(Box::new(snapshot)))?;
            self.initializing = Some(serial);
            self.phase = Phase::Initializing;
            Ok(())
        })();
        self.record_refusal(result)
    }
    // Failure affects admission/status, never lies about kernel publication.
    // Pending/accepted pixels remain selected until a coherent new result exists.
    pub(super) fn record_refusal(&mut self, result: Result<(), String>) -> Result<(), String> {
        if let Err(e) = &result {
            // Refusal is terminal for this registration. Retained UI pixels
            // need not keep an idle font session admitted. Running work still
            // owns the process slot until its actual worker-side drops finish.
            self.port.close();
            self.expected = None;
            self.initializing = None;
            self.phase = Phase::Refused(e.clone());
        }
        result
    }
    pub(super) fn reconcile(&mut self, kernel: &Kernel, dark: bool) -> Result<(), String> {
        if self.phase != Phase::Ready {
            return Ok(());
        }
        let result = self.queue_current(kernel, dark);
        self.record_refusal(result)
    }
    fn queue_current(&mut self, kernel: &Kernel, dark: bool) -> Result<(), String> {
        // No visited-history source list: one current metric source per live
        // paragraph, hard bounded by the kernel's exact-offer ceiling.
        self.sources.retain(|s| {
            kernel.node_by_key(s.stamp.owner()).is_some_and(|n| {
                n.paragraph_stamp()
                    .is_some_and(|stamp| stamp.same_metrics(&s.stamp))
            })
        });
        let Some(request) = kernel.region_text_request() else {
            if self.expected.take().is_some() {
                self.port.cancel();
            }
            return Ok(());
        };
        if self.expected.as_ref().is_some_and(|e| {
            same_request(e.input.request(), request) && e.paint.matches(request, dark)
        }) {
            return Ok(());
        }
        let slot = self
            .sources
            .iter()
            .position(|s| s.stamp.same_metrics(request.stamp()));
        if slot.is_none() {
            let bytes = self
                .sources
                .iter()
                .map(|s| s.paint.source.bytes())
                .sum::<usize>();
            if self.sources.len() >= SOURCE_SLOTS
                || bytes.saturating_add(request.source().bytes())
                    > exact_kernel::region::REGION_SOURCE_BYTES
            {
                return Err("content native source retention limit".into());
            }
        }
        // Capture native palette/link/ranges BEFORE queue, at the same full stamp.
        let paint = match slot.and_then(|i| self.sources.get(i)) {
            Some(s) if s.paint.matches(request, dark) => s.paint.clone(),
            _ => Arc::new(PaintSource::capture(kernel, request, dark)?),
        };
        let input = prepare(
            self.recipe.as_ref().ok_or("missing content font recipe")?,
            request.clone(),
            self.paint.ok_or("content raster context missing")?,
            slot.map(|i| &self.sources[i].source),
        )
        .map_err(|e| format!("content prepare: {e:?}"))?;
        let source = SourceSlot {
            stamp: request.stamp().clone(),
            source: input.source().clone(),
            paint: paint.clone(),
        };
        let serial = self.port.submit(Job::Text(input.clone()))?;
        if let Some(i) = slot {
            self.sources[i] = source;
        } else {
            self.sources.push(source);
        }
        self.expected = Some(Expected {
            serial,
            input,
            paint,
        });
        Ok(())
    }
    /// Takes at most one completion. No layout recursion, join or wait. The host
    /// performs one UI offer-discovery pass after true and queues its next miss.
    pub(super) fn poll(&mut self, kernel: &mut Kernel) -> Result<bool, String> {
        let result = self.poll_one(kernel);
        if let Err(e) = &result {
            let _ = self.record_refusal(Err(e.clone()));
        }
        result
    }
    fn poll_one(&mut self, kernel: &mut Kernel) -> Result<bool, String> {
        let Some((serial, reply)) = self.port.take() else {
            return if self.port.is_closed() && !matches!(self.phase, Phase::Refused(_)) {
                Err("content font service ended".into())
            } else {
                Ok(false)
            };
        };
        match reply {
            Reply::Initialized(result) => {
                if self.initializing != Some(serial) {
                    return Ok(false);
                }
                let prepared = result.map_err(|e| format!("content font preparation: {e:?}"))?;
                let (recipe, raster) = adopt_catalog_generation(*prepared);
                self.recipe = Some(recipe);
                self.raster = Some(raster);
                self.initializing = None;
                self.sources.clear();
                self.phase = Phase::Ready;
                Ok(true)
            }
            Reply::Text(result) => {
                if self.expected.as_ref().is_none_or(|e| e.serial != serial) {
                    return Ok(false);
                }
                let expected = self.expected.take().unwrap();
                if self.paint != Some(expected.input.paint_context()) {
                    return Ok(false);
                }
                if !kernel
                    .region_text_request()
                    .is_some_and(|r| same_request(r, expected.input.request()))
                {
                    return Ok(false);
                }
                let completed = result.map_err(|e| format!("content text work: {e:?}"))?;
                let adopted = adopt(
                    completed,
                    &expected.input,
                    self.raster
                        .as_ref()
                        .ok_or("missing content raster catalog")?,
                )
                .map_err(|e| format!("content text adoption: {e:?}"))?;
                let request = adopted.request().clone();
                let metrics = adopted.metrics();
                let artifact = Rc::new(NativeText {
                    adopted,
                    paint: expected.paint,
                });
                kernel
                    .resolve_region_text(&request, metrics, artifact)
                    .map_err(|e| format!("content exact answer: {e:?}"))
            }
        }
    }
}
fn same_request(a: &RegionTextRequest, b: &RegionTextRequest) -> bool {
    a.ticket() == b.ticket()
        && a.catalog() == b.catalog()
        && a.stamp() == b.stamp()
        && a.offer() == b.offer()
        && Arc::ptr_eq(a.source(), b.source())
}
#[cfg(test)]
pub(super) fn test_wait_idle() {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while SERVICE.occupied() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}
#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;
