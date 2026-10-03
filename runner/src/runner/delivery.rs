//! The `delivery` resource (LLP 1030 D7): the one thing a Contract app can
//! read about its own delivery, and the one thing a host tells the runner.
//!
//! The runner answers the source [`crate::delivery::SOURCE`] itself, before
//! the data seam — a data crate is never asked for it, and never could be:
//! these are the binary's and the update store's facts, not the app's. The
//! record is filled **by field name** from the shape the app declared, so an
//! app that wants only the stream declares only `stream`.
//!
//! The two commands are ordinary Contract commands (`deliveryCheck`,
//! `deliveryActivate`) and the runner adds nothing for them: they reach the
//! host through `take_commands` exactly as `setScheme` does.

use super::{Carried, DataError, DataSource, Runner, RunnerError, RunnerLinks, Seed};
use crate::delivery::{Delivery, SOURCE};
use exact_kernel::{CommitReceipt, Kernel};
use exact_plan::{Plan, TypeKind, Value};

impl<D: DataSource> Runner<D> {
    /// Boot with the host's snapshot of the app's kept secrets (LLP 1018
    /// D1): what the platform's store holds under the names the data
    /// crate's grants allow, read by the host before this call. A resource
    /// with no compiled value — one that read the store at bake — answers
    /// from it now, so the first frame is a returning user's.
    pub fn boot_stored(
        plan: impl Into<std::sync::Arc<Plan>>,
        data: D,
        kernel: Kernel,
        snapshot: Vec<(String, String)>,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(
            RunnerLinks::ALL,
            plan.into(),
            data,
            kernel,
            Seed::Fresh,
            snapshot,
            Delivery::default(),
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// Boot fresh state at a supplied clock, before evaluating initializers
    /// and resource arguments. Rendering never advances it or fires timers.
    pub fn boot_at(
        plan: impl Into<std::sync::Arc<Plan>>,
        data: D,
        kernel: Kernel,
        viewport: crate::Viewport,
        launch: &str,
        now_ms: f64,
    ) -> Result<Runner<D>, RunnerError> {
        Self::boot_inner(
            RunnerLinks::ALL,
            plan.into(),
            data,
            kernel,
            Seed::At(now_ms),
            Vec::new(),
            Delivery::default(),
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// Boot with complete host delivery facts before any resource settles.
    /// A carried runner's store takes precedence over the fresh snapshot.
    #[allow(clippy::too_many_arguments)] // the host boot facts
    pub fn boot_with_delivery(
        plan: impl Into<std::sync::Arc<Plan>>,
        data: D,
        kernel: Kernel,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        delivery: Delivery,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Self::boot_with_delivery_linked(
            RunnerLinks::ALL,
            plan,
            data,
            kernel,
            carried,
            snapshot,
            delivery,
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// [`Runner::boot_with_delivery`], with what the host links (LLP 1047 D3).
    #[allow(clippy::too_many_arguments)] // the host boot facts
    pub fn boot_with_delivery_linked(
        links: RunnerLinks,
        plan: impl Into<std::sync::Arc<Plan>>,
        data: D,
        kernel: Kernel,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        delivery: Delivery,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        let snapshot = carried.map_or(snapshot, |value| value.store.clone());
        let seed = carried.map_or(Seed::Fresh, Seed::Carried);
        Self::boot_inner(
            links,
            plan.into(),
            data,
            kernel,
            seed,
            snapshot,
            delivery,
            viewport,
            launch,
        )
    }

    /// What this runner believes about its delivery. Until a host says
    /// otherwise this is [`Delivery::default`] — the embedded answer.
    pub fn delivery(&self) -> &Delivery {
        &self.delivery
    }

    /// Tell the runner its delivery facts. Every resource reading
    /// [`crate::delivery::SOURCE`] is answered again in one commit; `None`
    /// when the facts did not change, or when no resource reads them.
    pub fn set_delivery(
        &mut self,
        delivery: Delivery,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if delivery == self.delivery {
            return Ok(None);
        }
        self.delivery = delivery;
        let which: Vec<usize> = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .collect();
        self.recommit(which, "delivery")
    }

    /// [`Runner::set_delivery`] with the binary's own three facts read out
    /// of the archive's `compat.json` (LLP 1030 D3a) — the compatibility id,
    /// whether an update store is linked, and the executors — and everything
    /// else left as it was.
    pub fn set_delivery_from_compat(
        &mut self,
        compat_json: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let delivery = self.delivery.with_compat(compat_json);
        self.set_delivery(delivery)
    }

    /// The delivery record for resource `i`, in the order its declared
    /// shape names its fields. A field the runner does not know is a
    /// refusal — bake catches it first, as `bake-delivery-field`.
    pub(super) fn delivery_answer(&self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        if row.args.len > 0 {
            return Err(DataError::BadArguments(format!(
                "{SOURCE} takes no arguments"
            )));
        }
        let ty = self.plan.type_(row.ty);
        if ty.kind != TypeKind::Record {
            return Err(DataError::Unavailable(format!(
                "{SOURCE} answers a record; this one is declared `{}`",
                self.plan.str(ty.name)
            )));
        }
        let mut fields = Vec::with_capacity(ty.fields.len as usize);
        for f in ty.fields.iter() {
            let name = self.plan.str(self.plan.field(f).name);
            match self.delivery.field(name) {
                Some(v) => fields.push(v),
                None => {
                    return Err(DataError::Unavailable(format!(
                        "{SOURCE} has no field `{name}`"
                    )))
                }
            }
        }
        Ok(Value::record(fields))
    }
}
