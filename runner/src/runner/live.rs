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
    /// The batch is for another design shape: a new plan is on its way.
    Structure,
    /// A token it names this plan cannot set (a role override, none such).
    Refused(String),
}

/// A batch applied: the commit, the rows set, whether the session's role
/// overrides moved (its host re-presents them), and whether the wide colour
/// table refused a colour (the build reports it `unsupported`).
#[derive(Debug)]
pub struct LiveApplied {
    /// The commit.
    pub receipt: CommitReceipt,
    /// Rows set again.
    pub rows: usize,
    /// The session's role overrides are another table now (LLP 1121 stage
    /// 5): its host re-presents what it drew from them.
    pub roles: bool,
    /// A colour was refused by a full wide table; its rows kept their value.
    pub full: bool,
}

impl<D: DataSource> Runner<D> {
    /// Admit live batches unless `compat` names a production build: the
    /// live table is development's only (`rules/DEFERRED.md` §Runtime).
    pub fn admit_live(&mut self, compat: Option<&str>) {
        self.live = !compat.is_some_and(crate::delivery::production);
    }

    /// Its role overrides, its own (LLP 1121 stage 5): what a host that
    /// drives several sessions on one thread binds around the work it does
    /// for this one ([`Runner::bind_roles`]).
    pub fn roles(&self) -> &std::rc::Rc<exact_kernel::style::roles::Roles> {
        self.roles.roles()
    }

    /// Its role overrides resolve until the scope ends.
    pub fn bind_roles(&self) -> exact_kernel::style::roles::RolesScope {
        exact_kernel::style::roles::bind(self.roles.roles())
    }

    /// @ref LLP 1095 D4 — its role overrides as a host's own sheet declares
    /// them ([`exact_kernel::style::roles::theme_css`]).
    pub fn theme_css(&self) -> String {
        let _roles = self.bind_roles();
        exact_kernel::style::roles::theme_css()
    }

    /// Its role overrides as this thread's (it boots installed; a host that
    /// hands it to another thread uninstalls it on this one first), the
    /// ones before put back when it leaves ([`Runner::uninstall_roles`]).
    pub fn install_roles(&mut self) {
        self.roles.install();
    }

    /// No longer this thread's: the ones before resolve here again.
    pub fn uninstall_roles(&mut self) {
        self.roles.uninstall();
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
        if edit.structure != design::shape(&self.plan) {
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
                roles: false,
                full: false,
            });
        }
        let refusals = exact_kernel::style::wide::refusals();
        let roles = self.roles().generation();
        let tokens: Vec<_> = edit.tokens.iter().collect();
        let slices: Vec<Vec<&str>> = tokens
            .iter()
            .map(|t| t.slices.iter().map(String::as_str).collect())
            .collect();
        let edits: Vec<(&str, &str, &[&str])> = tokens
            .iter()
            .zip(&slices)
            .map(|(t, s)| (t.name.as_str(), t.value.as_str(), s.as_slice()))
            .collect();
        let (mut receipt, rows) = self.set_tokens(&edits).map_err(|e| match e {
            RunnerError::InvalidValue { reason, .. } => LiveRefusal::Refused(reason),
            other => LiveRefusal::Refused(format!("{other:?}")),
        })?;
        self.design = Some(edit.design.clone());
        // A role crosses to a host as what replaces it, in any row, and the
        // kernel's own defaults read it: every node is the host's to send
        // again (LLP 1121 stage 5).
        let moved = self.roles().generation() != roles;
        if moved {
            let mut touched: std::collections::HashSet<_> =
                receipt.touched.iter().copied().collect();
            let kernel = self.kernel();
            let mut nodes: Vec<_> = kernel
                .roots()
                .into_iter()
                .filter_map(|id| kernel.node(id))
                .collect();
            while let Some(node) = nodes.pop() {
                if touched.insert(node.key) {
                    receipt.touched.push(node.key);
                }
                nodes.extend(node.child_nodes());
            }
        }
        Ok(LiveApplied {
            receipt,
            rows,
            roles: moved,
            full: exact_kernel::style::wide::refusals() > refusals,
        })
    }
}
