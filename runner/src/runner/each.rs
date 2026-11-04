//! A resource over a list (LLP 1017.000 P4c, amended 2026-11-04): `resource
//! r = source(x) for x in list [key=k] as shape S`. The runner keeps one
//! request for each element, keyed as a `for` row is: each element is a
//! member, a resource row of its own appended after the plan's, that asks
//! the source with the element as its one parameter and settles as any
//! resource does. An element that joins adds a member; one that leaves lets
//! only its member's request go; the others go on. `r` itself asks nothing:
//! it reads its members' answers as `S[]` in the list's order, pending or
//! failed when any member is.

use super::{DataSource, Runner, RunnerError, Target};
use crate::held::Held;
use crate::vm::{self, Env, Trap};
use exact_plan::Value;

/// One element's request.
#[derive(Clone)]
pub(super) struct Member {
    /// The resource over the list.
    pub(super) family: usize,
    /// The element's key, as [`crate::instance::key_text`] writes it.
    pub(super) key: String,
    /// The element, the one parameter its arguments read.
    pub(super) item: Value,
    /// Its element left the list: its request was let go, and it settles
    /// to nothing. A member's row is never reused.
    pub(super) gone: bool,
}

/// The members, by resource row: `None` for each of the plan's own rows.
#[derive(Clone, Default)]
pub(super) struct Members {
    pub(super) rows: Vec<Option<Member>>,
}

impl<D: DataSource> Runner<D> {
    /// The resource rows the plan declares; the members follow them.
    pub fn declared_resources(&self) -> &[exact_plan::ResourcesRow] {
        let members = self.members.rows.iter().flatten().count();
        &self.plan.resources[..self.plan.resources.len() - members]
    }

    /// Whether row `i` is a resource over a list.
    pub(super) fn is_family(&self, i: usize) -> bool {
        self.plan.resources[i].each.len != 0
    }

    /// Row `i`'s element, when it is a member.
    pub(super) fn member(&self, i: usize) -> Option<&Member> {
        self.members.rows.get(i).and_then(Option::as_ref)
    }

    /// What row `i`'s arguments read as parameters: a member's element.
    pub(super) fn member_params(&self, i: usize) -> &[Value] {
        self.member(i)
            .map_or(&[], |m| std::slice::from_ref(&m.item))
    }

    /// Family `i`'s elements in the list's order: each element's key and
    /// the element, read against this pass's values.
    pub(super) fn elements(
        &self,
        i: usize,
        env: &Env<'_>,
    ) -> Result<Result<Vec<(String, Value)>, RunnerError>, Trap> {
        let (list, key) = self
            .plan
            .resource_each(&self.plan.resources[i])
            .expect("a family");
        let Value::List(items) = vm::eval(self.plan.code(list), env, &[])?.value else {
            return Err(Trap::TypeMismatch {
                pc: 0,
                op: exact_plan::Opcode::LoadResource,
            });
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items.iter() {
            let params = std::slice::from_ref(item);
            let key = vm::eval(self.plan.code(key), &Env { params, ..*env }, &[])?.value;
            let Some(key) = crate::instance::key_text(&key) else {
                let resource = self.plan.str(self.plan.resources[i].name).to_string();
                return Ok(Err(RunnerError::ResourceKey { resource }));
            };
            out.push((key, item.clone()));
        }
        Ok(Ok(out))
    }

    /// The members of family `i` for `elements`, in their order (an element
    /// whose key repeats an earlier one's reads that one's answer), adding a
    /// member for each new key; and the family's live members no element
    /// keys, which leave. Whether a member was added comes back too.
    pub(super) fn join(
        &mut self,
        i: usize,
        elements: Vec<(String, Value)>,
    ) -> (Vec<usize>, Vec<usize>, bool) {
        let mut order = Vec::with_capacity(elements.len());
        let mut added = false;
        for (key, item) in elements {
            let found = (self.members.rows.iter().enumerate()).find_map(|(m, row)| {
                row.as_ref()
                    .filter(|row| row.family == i && !row.gone && row.key == key)
                    .map(|_| m)
            });
            let m = match found {
                Some(m) => m,
                None => {
                    added = true;
                    self.add_member(i, key, item.clone())
                }
            };
            // A repeated key reads the first element's request.
            if !order.contains(&m) {
                if let Some(Some(row)) = self.members.rows.get_mut(m) {
                    row.item = item;
                }
            }
            order.push(m);
        }
        let leaving = (self.members.rows.iter().enumerate())
            .filter(|(m, row)| {
                row.as_ref()
                    .is_some_and(|row| row.family == i && !row.gone && !order.contains(m))
            })
            .map(|(m, _)| m)
            .collect();
        (order, leaving, added)
    }

    /// A member of family `i` for the element `item` keyed `key`: a copy of
    /// the family's row that asks for one element and answers its type.
    fn add_member(&mut self, i: usize, key: String, item: Value) -> usize {
        // The inspection digest names the plan as it was built.
        let _ = self.inspection_digest();
        let mut row = self.plan.resources[i].clone();
        row.ty = self.plan.types[row.ty.0 as usize]
            .elem
            .expect("a list type");
        row.each = Default::default();
        self.plan.resources.push(row);
        let m = self.plan.resources.len() - 1;
        self.members.rows.resize(m, None);
        self.members.rows.push(Some(super::each::Member {
            family: i,
            key,
            item,
            gone: false,
        }));
        self.fit_resources();
        m
    }

    /// Every per-resource table as long as the rows: a member's entries
    /// start empty, as a resource's do before boot settles it.
    pub(super) fn fit_resources(&mut self) {
        let n = self.plan.resources.len();
        self.resources.resize(n, None);
        self.resource_values.resize(n, None);
        self.failed_args.resize(n, None);
        self.failed_why.resize(n, None);
        self.pending_res.resize(n, false);
        self.overlays.resize(n, None);
        self.store_readers.resize(n, false);
        self.entropy_readers.resize(n, false);
        self.watching.resize(n, Vec::new());
        self.stale.resize(n, false);
        self.awaiting.resize(n, false);
        self.members.rows.resize(n, None);
        if !self.ids.work.resources.is_empty() {
            self.ids.work.resources.resize(n, Default::default());
        }
    }

    /// Family `i`'s value from its members' this pass: their answers as a
    /// list, pending when any is, failed (with the first failure) when any
    /// is.
    pub(super) fn gather(
        &mut self,
        i: usize,
        order: &[usize],
        resources: &[Option<Held>],
        pending: &[bool],
    ) -> (Value, bool) {
        let value = Value::list(
            (order.iter())
                .map(|&m| {
                    resources[m]
                        .as_ref()
                        .expect("settled")
                        .get(&self.plan)
                        .clone()
                })
                .collect(),
        );
        let failed = order.iter().find(|&&m| self.failed_args[m].is_some());
        self.failed_args[i] = failed.map(|_| Vec::new());
        self.failed_why[i] = failed.and_then(|&m| self.failed_why[m].clone());
        (value, order.iter().any(|&m| pending[m]))
    }

    /// Let go of each member in `leaving`: its request goes, and it holds
    /// nothing from now on.
    pub(super) fn let_go_members(&mut self, leaving: &[usize]) {
        for &m in leaving {
            self.forget(Target::Resource(m));
            if let Some(Some(row)) = self.members.rows.get_mut(m) {
                row.gone = true;
            }
            self.resources[m] = None;
            self.resource_values[m] = None;
            self.failed_args[m] = None;
            self.failed_why[m] = None;
            self.overlays[m] = None;
            self.store_readers[m] = false;
            self.watching[m].clear();
            self.stale[m] = false;
            self.awaiting[m] = false;
            self.refresh_next.retain(|r| *r != m);
        }
        self.sync_pending_flags();
    }

    /// A family is pending while any of its live members is.
    pub(super) fn pending_families(&mut self) {
        for m in 0..self.members.rows.len() {
            if let Some(row) = self.members.rows[m].as_ref().filter(|row| !row.gone) {
                let family = row.family;
                self.pending_res[family] |= self.pending_res[m];
            }
        }
    }

    /// Which members are gone, for a refused commit to put back.
    pub(super) fn gone_members(&self) -> Vec<bool> {
        (self.members.rows.iter())
            .map(|row| row.as_ref().is_some_and(|row| row.gone))
            .collect()
    }

    /// After a refused commit: the members it let go are live again, and the
    /// tables it put back are as long as the rows.
    pub(super) fn restore_members(&mut self, gone: &[bool]) {
        for (row, &gone) in self.members.rows.iter_mut().zip(gone) {
            if let Some(row) = row {
                row.gone = gone;
            }
        }
        self.fit_resources();
    }
}
