//! Region-aware layout and completion on the owning UI/runtime thread.
use super::*;
use crate::content_region::{ContentRegionRegistration, NativeArtifact};
use exact_kernel::{RegionInputs, RegionTextRequest, TextMetrics};
use std::{any::Any, rc::Rc};
impl<D: DataSource> Host<D> {
    /// Boot one explicitly registered content region before any text layout.
    pub fn boot_region(
        plan: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        registration: ContentRegionRegistration,
    ) -> Result<(Self, String), HostError> {
        let (mut host, batch) = Self::boot_stored_after_decode(
            plan,
            data,
            measurer,
            width,
            height,
            None,
            Vec::new(),
            None,
            None,
            None,
            None,
            "/",
            Some(registration),
            |_| {},
        )?;
        host.commit_boot();
        Ok((host, batch))
    }
    /// The one current request. The source/ticket retains no Arena/Runtime.
    pub fn pending_region_request(&self) -> Option<(u64, &RegionTextRequest)> {
        let p = self.content_region.as_ref()?.pending.as_ref()?;
        Some((p.id, &p.request))
    }
    /// Source/paint snapshot for a current request, copied only by its consumer.
    pub fn region_request_json(&self, id: u64) -> Result<String, String> {
        self.region_request_json_known(id, 0)
    }
    pub(crate) fn region_request_json_known(&self, id: u64, known: u64) -> Result<String, String> {
        self.content_region
            .as_ref()
            .ok_or("no content region")?
            .request_json(self.runner.kernel(), id, known)
    }
    /// Selected immutable publication identity, including retained old content.
    pub fn region_publication_id(&self) -> Option<u64> {
        self.content_region
            .as_ref()?
            .publication
            .as_ref()
            .map(|(id, _)| *id)
    }
    /// Takes ownership even when stale. No stale metric or completion changes
    /// clocks, current requests or the retained accepted native artifact.
    pub fn complete_region_text(
        &mut self,
        id: u64,
        metrics: TextMetrics,
        owner: Rc<dyn Any>,
    ) -> String {
        let Some(request) = self
            .pending_region_request()
            .filter(|(n, _)| *n == id)
            .map(|(_, r)| r.clone())
        else {
            return self.finish(Batch::new(), None);
        };
        let artifact = Rc::new(NativeArtifact { id, _owner: owner });
        match self
            .runner
            .kernel_mut()
            .resolve_region_text(&request, metrics, artifact)
        {
            Ok(true) => {
                let mut batch = Batch::new();
                let error = self.layout(&mut batch).err();
                self.finish(batch, error)
            }
            Ok(false) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("region completion: {e:?}"))),
        }
    }
    pub(super) fn region_layout(
        &mut self,
        root: ViewId,
        offer: Offer,
        batch: &mut Batch,
    ) -> Result<(), String> {
        let Some(region) = self.content_region.as_mut() else {
            return Ok(());
        };
        if self.height_owner.is_some() {
            return Err("Height projection is unsupported with content region".into());
        }
        let receipt = self
            .runner
            .kernel_mut()
            .compute_region_layout(
                root,
                offer,
                RegionInputs {
                    catalog: region.incarnation,
                    consumer_revision: 0,
                },
            )
            .map_err(|e| format!("region layout: {e:?}"))?;
        region.observe(self.runner.kernel(), receipt)?;
        batch.region(&region.json(self.runner.kernel()));
        Ok(())
    }
}
