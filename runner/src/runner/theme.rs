//! A design token edited while the app runs (LLP 1121 D7, D13): the rows
//! that read it are set again from their text in their own tuple, with no
//! tree visited and no binding evaluated; a colour row so set moves no
//! geometry, so the commit paints.

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
        crate::bridge::edit_token(name, value, slices);
        // A role override is matched by its row spelling too.
        let role = self
            .plan
            .theme
            .iter()
            .find(|t| self.plan.str(t.name) == name)
            .map(|t| self.plan.str(t.role))
            .filter(|r| !r.is_empty());
        let bits = exact_kernel::style::theme::name_bit(name)
            | role.map_or(0, exact_kernel::style::theme::name_bit);
        let mut patches: Vec<(exact_kernel::ViewId, Box<StyleProps>)> = Vec::new();
        let mut set = 0;
        let mut rows: Vec<_> = self
            .ids
            .theme
            .rows()
            .filter(|(_, r)| r.tokens & bits != 0)
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
