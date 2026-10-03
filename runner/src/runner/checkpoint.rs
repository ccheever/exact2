//! A rendered document's checkpoint (LLP 1048.000 D6): the answers the
//! document was projected from, handed to the runtime as an input of their
//! own — not the store, not kept answers, not a reload's carried state.
use super::{DataSource, ResourceState, Runner, RunnerError};
use crate::delivery::Delivery;
use crate::Store;
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};

/// The state a document was projected from. Never a store.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Checkpoint {
    /// The router location the document was rendered at.
    pub location: String,
    /// The render time: the clock the document was projected at (LLP
    /// 1048.000 D5), and every answer's freshness.
    pub now_ms: f64,
    /// The identity of the logic that answered (`DataSource::revision`).
    pub logic: Option<String>,
    /// Resource name → (source name, arguments, answer), for every resource
    /// that had answered when the document was projected.
    pub answers: Vec<(String, String, Vec<Value>, Value)>,
    /// What was still pending then, and rendered its placeholder.
    pub pending: Vec<String>,
}

impl<D: DataSource> Runner<D> {
    /// Boot from a rendered document's checkpoint (LLP 1048.000 D6). Each
    /// answer seeds its resource — the same name and source, answered by
    /// the same logic, a value that still fits — as an answer this runner
    /// already has: it isn't asked again at boot or at `data_ready`, only
    /// when its arguments or inputs change, as any answer is. The clock
    /// starts at the render time; the host's clock and store then apply as
    /// ordinary updates. A store-reading resource the render answered is
    /// asked again when the device's store holds the app's own entries,
    /// showing the rendered answer meanwhile: the render had no store.
    #[allow(clippy::too_many_arguments)] // the host boot facts
    pub fn boot_checkpoint(
        plan: Plan,
        data: D,
        kernel: Kernel,
        checkpoint: &Checkpoint,
        snapshot: Vec<(String, String)>,
        delivery: Delivery,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_checkpoint_linked(
            super::RunnerLinks::ALL,
            plan,
            data,
            kernel,
            checkpoint,
            snapshot,
            delivery,
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// [`Runner::boot_checkpoint`], with what the host links (LLP 1047 D3).
    #[allow(clippy::too_many_arguments)] // the host boot facts
    pub fn boot_checkpoint_linked(
        links: super::RunnerLinks,
        plan: Plan,
        data: D,
        kernel: Kernel,
        checkpoint: &Checkpoint,
        snapshot: Vec<(String, String)>,
        delivery: Delivery,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(
            links,
            plan,
            data,
            kernel,
            super::Seed::Checkpoint(checkpoint),
            snapshot,
            delivery,
            viewport,
            launch,
        )
    }

    /// What a document projected from this runner now was projected from
    /// (LLP 1048.000 D6): every answer of the app's sources, the location,
    /// the clock, and what is still pending. The runner's own facts
    /// (delivery, the viewport, surface records) are the runtime's to
    /// answer, never a checkpoint's.
    pub fn document_checkpoint(&self, location: &str) -> Checkpoint {
        let mut pending: Vec<String> = self.pending().into_iter().map(|(name, _)| name).collect();
        let mut answers = Vec::new();
        for (i, (row, state)) in self.plan.resources.iter().zip(&self.resources).enumerate() {
            let name = self.plan.str(row.name);
            let source = self.plan.str(row.source);
            if exact_plan::runner_owned_source(source) {
                continue;
            }
            if self.stale[i] {
                if !pending.iter().any(|p| p == name) {
                    pending.push(name.to_string());
                }
                continue;
            }
            if let Some(state) = state
                .as_ref()
                .filter(|state| !self.pending_res[i] && !state.placeholder)
            {
                answers.push((
                    name.to_string(),
                    source.to_string(),
                    state.args.clone(),
                    state.value.get(&self.plan).clone(),
                ));
            }
        }
        Checkpoint {
            location: location.to_string(),
            now_ms: self.now_ms,
            logic: self.data.revision().map(str::to_owned),
            answers,
            pending,
        }
    }

    /// Seed each resource `checkpoint` answered (see [`Runner::boot_checkpoint`]).
    /// Returns which rows it seeded, and the journal's line for it, which
    /// boot writes after its own.
    pub(super) fn seed_checkpoint(&mut self, checkpoint: &Checkpoint) -> (Vec<bool>, String) {
        let mut seeded = vec![false; self.plan.resources.len()];
        if checkpoint.logic.as_deref() != self.data.revision() {
            let note = "checkpoint: other logic answered it; its answers are asked again";
            return (seeded, note.into());
        }
        for (i, seed) in seeded.iter_mut().enumerate() {
            let row = &self.plan.resources[i];
            let (name, source) = (self.plan.str(row.name), self.plan.str(row.source));
            if self.resources[i].is_some() || exact_plan::runner_owned_source(source) {
                continue;
            }
            let Some((_, _, args, value)) = checkpoint
                .answers
                .iter()
                .find(|(n, s, _, _)| n == name && s == source)
            else {
                continue;
            };
            if args.len() != row.args.len as usize || self.check_shape(i, value).is_err() {
                continue;
            }
            self.resources[i] = Some(ResourceState {
                args: args.clone(),
                value: crate::held::Held::new(value.clone()),
                store_revision: self.store.revision(),
                placeholder: false,
                kept_seed: false,
            });
            *seed = true;
        }
        let taken = seeded.iter().filter(|s| **s).count();
        let note = super::lines::seeded(taken, checkpoint.answers.len());
        (seeded, note)
    }

    /// Whether the store holds the app's own entries, not only the
    /// runner's kept answers: device state a render never has.
    pub(super) fn has_app_store(&self) -> bool {
        self.store
            .names()
            .iter()
            .any(|name| !name.starts_with(Store::KEPT))
    }
}
