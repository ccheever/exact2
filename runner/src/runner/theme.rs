//! A design token edited while the app runs (LLP 1121 D7, D13): the rows
//! that read it are set again from their text in their own tuple, with no
//! tree visited and no binding evaluated; a colour row so set moves no
//! geometry, so the commit paints. A token that overrides a role sets the
//! session's overrides too (stage 5), which its host re-presents.

use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, Op, StyleProps};

impl<D: DataSource> Runner<D> {
    /// Edit token `name`: its default tuple's value and, when it varies,
    /// every tuple's (the editor's edit, D13). Each row the index keeps that
    /// read it is set again in one commit; how many is the second value.
    pub fn set_token(
        &mut self,
        name: &str,
        value: &str,
        slices: &[&str],
    ) -> Result<(CommitReceipt, usize), RunnerError> {
        self.set_tokens(&[(name, value, slices)])
    }

    /// Edit several tokens as [`set_token`](Self::set_token) does one, in
    /// one commit (a live batch, LLP 1121 stage 4). Nothing is set when any
    /// is refused.
    pub fn set_tokens(
        &mut self,
        edits: &[(&str, &str, &[&str])],
    ) -> Result<(CommitReceipt, usize), RunnerError> {
        let mut bits = 0;
        let mut roles = Vec::new();
        for &(name, value, _) in edits {
            let refused = |reason: &str| RunnerError::InvalidValue {
                event: "setToken",
                reason: format!("`{name}`: {reason}"),
            };
            let token = self
                .plan
                .theme
                .iter()
                .find(|t| self.plan.str(t.name) == name)
                .ok_or_else(|| refused("is no token this plan keeps"))?;
            // A role's value also crosses to the hosts and the kernel's own
            // defaults (LLP 1095 D4's reach): the session's overrides take it.
            if let Some(id) = exact_kernel::style::roles::role(self.plan.str(token.role)) {
                let colour = crate::bridge::role_colour(value)
                    .ok_or_else(|| refused("overrides a role with no colour"))?;
                let now = self.roles().overrides().iter().find(|(r, _)| *r == id);
                if now.is_none_or(|(_, c)| *c != colour) {
                    roles.push((id, colour));
                }
            }
            bits |= exact_kernel::style::theme::name_bit(name);
        }
        for &(name, value, slices) in edits {
            crate::bridge::edit_token(&self.plan, name, value, slices);
        }
        // Rows below parse under the new overrides; what the host drew from
        // the old ones it re-presents (`roles_moved`).
        if !roles.is_empty() {
            let next = self.roles().with(&roles);
            self.roles.replace(next);
        }
        let mut patches: Vec<(exact_kernel::ViewId, Box<StyleProps>)> = Vec::new();
        let mut set = 0;
        // A retired view is not set: the index forgets it with the rest.
        let kernel = &self.kernel;
        let mut rows: Vec<_> = self
            .ids
            .theme
            .rows()
            .filter(|(view, r)| r.tokens & bits != 0 && kernel.node(*view).is_some())
            .collect();
        rows.sort_by_key(|(view, r)| (*view, r.style as u16));
        for (view, row) in rows {
            let at = match patches.last_mut() {
                Some((v, p)) if *v == view => p,
                _ => {
                    patches.push((view, Box::default()));
                    &mut patches.last_mut().expect("pushed").1
                }
            };
            let _tuple = Tuple::new(row.tuple);
            if crate::bridge::set_plan_style(at, row.style as u16, &row.text, &self.plan).is_ok() {
                match row.written {
                    true => at.written.set(row.style),
                    false => at.written.clear(row.style),
                }
                set += 1;
            }
        }
        exact_kernel::style::theme::take_named();
        let ops = patches
            .into_iter()
            .map(|(id, patch)| Op::SetStyle { id, patch })
            .collect();
        Ok((self.apply(ops)?, set))
    }
}

/// The tuple rows are parsed in, restored when dropped.
struct Tuple(u32);

impl Tuple {
    fn new(tuple: u32) -> Tuple {
        Tuple(crate::bridge::set_tuple(tuple))
    }
}

impl Drop for Tuple {
    fn drop(&mut self) {
        crate::bridge::set_tuple(self.0);
    }
}
