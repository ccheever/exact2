//! Host geometry for runner-owned list windows. LLP 1010 §6.
use super::*;

/// The host's current scrollport and measured row boxes, in CSS pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListViewport<'a> {
    /// Native/browser scroll offset.
    pub top: f64,
    /// Recent user scroll velocity in CSS pixels/second; zero keeps symmetric overscan.
    pub velocity: f64,
    /// Actual scrollport height.
    pub height: f64,
    /// Actual row containing-block width; a change invalidates cached heights.
    pub width: f64,
    /// Content origin inside the scroller.
    pub origin: f64,
    /// Focused and interacting descendants, or zero.
    pub pins: [ViewId; 2],
    /// Current wrapper IDs and their measured border-box heights.
    pub rows: &'a [(ViewId, f64)],
}

/// Where a windowed list stands after its last geometry report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListStatus {
    /// The scroll offset held; measuring rows can move it to keep the reading
    /// key still, and a host settling several rounds reports from here.
    pub top: f64,
    /// Rows the last report created.
    pub created: usize,
    /// The window still has rows to create or retire under its budget.
    pub pending: bool,
}

/// A text position that survives retirement of a list row's native views.
#[derive(Debug, Clone, Copy)]
pub struct ListTextPosition<'a> {
    /// The opaque key published on the row wrapper.
    pub key: &'a str,
    /// Zero-based paragraph within the row.
    pub paragraph: usize,
    /// UTF-16 offset within that paragraph, as on the DOM and Apple text APIs.
    pub offset: usize,
}

impl<D: DataSource> Runner<D> {
    /// Resolve an opaque row key without materializing that row.
    pub fn list_index(&self, view: ViewId, key: &str) -> Option<usize> {
        self.tree.as_ref()?.list_index(view, key)
    }

    /// A windowed list's offset, and what its last report did and left undone.
    pub fn list_status(&self, view: ViewId) -> Option<ListStatus> {
        self.tree.as_ref()?.list_status(view)
    }

    /// Copy all logical text, or the range between two stable endpoints.
    /// At most one unmounted row is realized at a time; its operations are
    /// never committed. No data query, clock advance, or native view is created.
    pub fn list_text(
        &self,
        view: ViewId,
        range: Option<(ListTextPosition<'_>, ListTextPosition<'_>)>,
    ) -> Result<String, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let mut ids = Ids::default();
        let mut u = Update {
            env: self.env(&[], &[]),
            sites: &self.sites,
            ids: &mut ids,
            ops: Vec::new(),
            surfaces: Vec::new(),
            work: Default::default(),
        };
        Ok(self
            .tree
            .as_ref()
            .expect("booted")
            .list_text(&mut u, view, range)?)
    }
    /// Report an actual list scrollport in CSS pixels. `top` is scrollTop,
    /// `origin` is the content origin inside that scrollport, and `pins`
    /// names at most one focused and one interacting descendant. This path
    /// does not settle data or execute a Contract action.
    pub fn list_viewport(
        &mut self,
        view: ViewId,
        geometry: ListViewport<'_>,
    ) -> Result<CommitReceipt, RunnerError> {
        self.list_viewport_within(view, geometry, None)
    }

    /// [`Runner::list_viewport`] under a budget: `create_limit` is how many
    /// rows this report may create beyond those the scrollport itself shows,
    /// or `None` for the whole window at once. A host that scrolls on the
    /// thread that lays out fills its overscan a few rows at a time, between
    /// frames; rows the reader can see are never rationed.
    /// [`ListStatus::pending`] says whether the window wants another report.
    pub fn list_viewport_within(
        &mut self,
        view: ViewId,
        geometry: ListViewport<'_>,
        create_limit: Option<usize>,
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        if ![
            geometry.top,
            geometry.velocity,
            geometry.height,
            geometry.width,
            geometry.origin,
        ]
        .iter()
        .all(|n| n.is_finite())
            || geometry.height < 0.0
            || geometry.width < 0.0
            || geometry
                .rows
                .iter()
                .any(|(_, h)| !h.is_finite() || *h < 0.0 || *h > f32::MAX as f64)
        {
            return Err(crate::instance::InstanceError::List("invalid list geometry").into());
        }
        if self
            .kernel
            .node(view)
            .is_none_or(|n| n.node_type != exact_kernel::NodeType::List)
        {
            return Err(RunnerError::UnknownView(view));
        }
        self.update_tree(false, |tree, u| {
            tree.update_list(u, view, geometry, create_limit)
        })
    }

    /// Re-evaluate every site and apply one batch. A failure here means the
    /// instance tree and the kernel may disagree; the runner is poisoned and
    /// the host restarts it — never a half-applied frame.
    pub(super) fn update(&mut self) -> Result<CommitReceipt, RunnerError> {
        self.update_tree(true, |tree, u| tree.update(u))
    }

    fn update_tree(
        &mut self,
        settled: bool,
        update: impl FnOnce(&mut Tree, &mut Update<'_>) -> Result<(), crate::instance::InstanceError>,
    ) -> Result<CommitReceipt, RunnerError> {
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let result = {
            let mut u = Update {
                env: self.env(&[], &[]),
                sites: &self.sites,
                ids: &mut ids,
                ops: Vec::new(),
                surfaces: Vec::new(),
                work: Default::default(),
            };
            update(&mut tree, &mut u).map(|_| (u.ops, u.surfaces))
        };
        self.ids = ids;
        self.tree = Some(tree);
        let (ops, surfaces) = match result {
            Ok(x) => x,
            Err(e) => {
                self.poison();
                return Err(e.into());
            }
        };
        // Only ordinary settlement wakes deferred collection edges; geometry-only
        // feedback must not replenish this work.
        if settled {
            if let Err(error) = self.wake_deferred_edges() {
                self.poison();
                return Err(error);
            }
        }
        match self.apply(ops) {
            Ok(receipt) => {
                self.surfaces.extend(surfaces);
                Ok(receipt)
            }
            Err(e) => {
                self.poison();
                Err(e)
            }
        }
    }
}
