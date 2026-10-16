//! A live batch or snapshot applied to the running app (LLP 1121 D13,
//! stage 4): every token it carries set in one commit, no plan published.

use super::{DataSource, Runner, RunnerError};
use crate::design::{self, Edit};
use exact_kernel::CommitReceipt;

/// Why a build does not apply a batch.
#[derive(Debug, PartialEq)]
pub enum LiveRefusal {
    /// The batch's base is not the design this build runs: it asks for a
    /// snapshot.
    Stale,
    /// The batch is for another plan structure: a new plan is on its way.
    Structure,
    /// A token it names this plan cannot set (a role override, none such).
    Refused(String),
}

/// A batch applied: the commit, the rows set, and whether the wide colour
/// table refused a colour (the build reports it `unsupported`).
#[derive(Debug)]
pub struct LiveApplied {
    /// The commit.
    pub receipt: CommitReceipt,
    /// Rows set again.
    pub rows: usize,
    /// A colour was refused by a full wide table; its rows kept their value.
    pub full: bool,
}

impl<D: DataSource> Runner<D> {
    /// Admit live batches unless `compat` names a production build: the
    /// live table is development's only (`rules/DEFERRED.md` §Runtime).
    pub fn admit_live(&mut self, compat: Option<&str>) {
        self.live = !compat.is_some_and(crate::delivery::production);
    }

    /// The design digest this build runs.
    pub fn design(&self) -> String {
        self.design
            .clone()
            .unwrap_or_else(|| design::digest(&design::tokens(&self.plan)))
    }

    /// Apply a batch, or a snapshot, from the dev stream.
    pub fn apply_live(&mut self, edit: &Edit) -> Result<LiveApplied, LiveRefusal> {
        if !self.live {
            return Err(LiveRefusal::Refused(
                "a release build takes no live batch".into(),
            ));
        }
        if edit.structure != design::structure(&self.plan) {
            return Err(LiveRefusal::Structure);
        }
        let running = self.design();
        if edit.base.as_ref().is_some_and(|base| *base != running) {
            return Err(LiveRefusal::Stale);
        }
        // No tokens (a notice), or the design it runs already (a snapshot
        // polled again): nothing to set.
        if edit.tokens.is_empty() || edit.design == running {
            return Ok(LiveApplied {
                receipt: CommitReceipt::default(),
                rows: 0,
                full: false,
            });
        }
        let refusals = exact_kernel::style::wide::refusals();
        let slices: Vec<Vec<&str>> = edit
            .tokens
            .iter()
            .map(|t| t.slices.iter().map(String::as_str).collect())
            .collect();
        let edits: Vec<(&str, &str, &[&str])> = edit
            .tokens
            .iter()
            .zip(&slices)
            .map(|(t, s)| (t.name.as_str(), t.value.as_str(), s.as_slice()))
            .collect();
        let (receipt, rows) = self.set_tokens(&edits).map_err(|e| match e {
            RunnerError::InvalidValue { reason, .. } => LiveRefusal::Refused(reason),
            other => LiveRefusal::Refused(format!("{other:?}")),
        })?;
        self.design = Some(edit.design.clone());
        Ok(LiveApplied {
            receipt,
            rows,
            full: exact_kernel::style::wide::refusals() > refusals,
        })
    }
}
