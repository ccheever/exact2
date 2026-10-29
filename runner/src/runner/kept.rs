//! Kept answers (LLP 1027 D4, as ruled 2026-09-03): a store-reading
//! resource's last fresh answer, persisted beside the app's secrets under
//! [`Store::KEPT`] so the next launch's first frame shows it before the
//! data source is ready — a TypeScript module its host loads after first
//! pixel — and the runner asks again at `data_ready`.
//!
//! Kept as the identifying arguments' canonical list followed by the answer's
//! canonical bytes, each decoded as it was encoded; a kept answer whose arguments no longer match, whose
//! bytes no longer decode, or whose value no longer fits the declared shape
//! is simply not used, and the compiled empty-store placeholder stands.

use super::{DataSource, ResourceState, Runner, RunnerError};
use crate::store::Store;
use exact_kernel::CommitReceipt;
use exact_plan::Value;

/// The largest kept answer: its identifying arguments' and its value's
/// canonical bytes together. Reading, decoding and shape-checking one this
/// size at boot takes under a millisecond on an iPhone simulator (LLP 1027
/// D4, 2026-10-16): a library a first screen shows, never a feed.
const KEPT_DATA_BYTES: usize = 256 * 1024;
/// The longest kept entry: the data, with the arguments' list header
pub(super) const MAX_KEPT_BYTES: usize = KEPT_DATA_BYTES + 5;
/// The longest the kept entries are together: what a boot reads and
/// decodes in all, on Apple's main thread (about 2 ms for two entries on a
/// loaded iPhone simulator), what a native host keeps on disk beside the
/// app's own files, and, as base64, about 700 K of the characters a web
/// page's `localStorage` shares with the app's secrets and persisted
/// states (LLP 1027 D4, 2026-10-16).
pub(crate) const KEPT_TOTAL_BYTES: usize = 2 * MAX_KEPT_BYTES;

/// Whether answers are kept, and the resources an answer too big to keep was said of, once each
#[derive(Debug, Default)]
pub(super) struct Keeping {
    pub(super) on: bool,
    pub(super) too_big: Vec<usize>,
    /// The resources said to be kept under an argument of this launch's own, once each
    pub(super) of_launch: Vec<usize>,
    /// Kept answers this boot refused, by resource, with why: journaled
    /// with the ones it shows, so a refusal is never silent.
    pub(super) refused: Vec<(usize, String)>,
    /// Whether a module source declared `kept` keeps its answers (LLP
    /// 1133.015 D4): always, but in a private session.
    pub(super) modules: bool,
}

/// Whether resource `i` reads a module's live source declared `"kept":
/// true` (LLP 1133.015 D4): its implementation loads after first pixel
/// whatever the data source, so its last answer is kept as a TypeScript
/// source's is.
pub(super) fn kept_module(plan: &exact_plan::Plan, i: usize) -> bool {
    let source = plan.resources[i].source;
    (plan.modules.iter()).any(|m| {
        m.live
            && !m.send
            && plan.sources[m.source.0 as usize].name == source
            && serde_json::from_str::<serde_json::Value>(plan.str(m.substitute))
                .is_ok_and(|sub| sub["kept"] == true)
    })
}

/// Note that resource `i`'s kept answer, kept for `kept`, was refused
/// for this launch's identifying `now` (or, with `now` unknown, because
/// they read a store reader still on its fallback). Once per resource.
pub(super) fn refuse(
    keeping: &mut Keeping,
    plan: &exact_plan::Plan,
    i: usize,
    kept: &[Value],
    now: Option<&[Value]>,
) {
    if keeping.refused.iter().any(|(r, _)| *r == i) {
        return;
    }
    let json = |args: &[Value]| {
        let mut out = String::new();
        crate::agent::untyped_json(&Value::list(args.to_vec()), &mut out);
        out
    };
    let why = match now {
        Some(now) => format!(
            "this launch asks for {}",
            super::lines::shortened(&json(now))
        ),
        None => "this launch's arguments read a store reader still on its fallback".into(),
    };
    let name = plan.str(plan.resources[i].name);
    keeping
        .refused
        .push((i, super::lines::kept_refused(name, Some(&json(kept)), &why)));
}

/// The store name a resource's kept answer lives under.
/// The kept entries in `snapshot` no declared reader of `plan` seeds. A
/// cold boot seeds a first frame only from declared readers' kept answers,
/// so any other entry (a reader made transient, a resource removed) is
/// forgotten on disk, not carried forever. A reload's carried readers are
/// checked for compatibility instead.
pub(super) fn obsolete(plan: &exact_plan::Plan, snapshot: &[(String, Vec<u8>)]) -> Vec<String> {
    snapshot
        .iter()
        .filter_map(|(name, _)| {
            let resource = name.strip_prefix(Store::KEPT)?;
            // The uploads' marks are no resource's answer (LLP 1016.000 D10).
            (name != super::uploads::UPLOADS
                && !(0..plan.resources.len()).any(|i| {
                    (plan.resources[i].reader || kept_module(plan, i))
                        && plan.str(plan.resources[i].name) == resource
                }))
            .then(|| name.clone())
        })
        .collect()
}

/// Every kept entry in `snapshot`: what an app that keeps no answers
/// forgets (LLP 1069.008.000 D7).
pub(super) fn all(snapshot: &[(String, Vec<u8>)]) -> Vec<String> {
    snapshot
        .iter()
        .filter(|(name, _)| name.starts_with(Store::KEPT))
        .map(|(name, _)| name.clone())
        .collect()
}

pub(super) fn kept_name(resource: &str) -> String {
    let mut name = String::from(Store::KEPT);
    name.push_str(resource);
    name
}

pub(super) fn encode(args: &[Value], value: &Value) -> Vec<u8> {
    let mut w = exact_plan::bytes::Writer::default();
    Value::list(args.to_vec()).encode(&mut w);
    value.encode(&mut w);
    w.into_vec()
}

// Count canonical bytes only as far as the budget permits. Iterators
// borrow children: even an enormous list never allocates a traversal-sized stack.
fn fits(args: &[Value], value: &Value) -> bool {
    let mut remaining = KEPT_DATA_BYTES;
    let mut stack = vec![std::slice::from_ref(value).iter(), args.iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        let (bytes, children) = match item {
            Value::Number(_) => (9, None),
            Value::Bool(_) => (2, None),
            s @ exact_plan::str_value!() => {
                let Some(bytes) = s.text().len().checked_add(5) else {
                    return false;
                };
                (bytes, None)
            }
            Value::Unit | Value::Option(None) => (1, None),
            Value::Option(Some(v)) => (1, Some(std::slice::from_ref(v.as_ref()))),
            Value::List(v) | Value::Record(v) => (5, Some(&v[..])),
        };
        let Some(left) = remaining.checked_sub(bytes) else {
            return false;
        };
        remaining = left;
        if let Some(children) = children {
            // Every child uses at least one canonical byte.
            if children.len() > remaining {
                return false;
            }
            stack.push(children.iter());
        }
    }
    true
}

pub(super) fn decode(bytes: &[u8]) -> Option<(Vec<Value>, Value)> {
    if bytes.len() > MAX_KEPT_BYTES {
        return None;
    }
    // Each from its own depth, as it was kept: no envelope costs the answer a level
    let mut r = exact_plan::bytes::Reader::new(bytes);
    let Value::List(args) = Value::decode(&mut r).ok()? else {
        return None;
    };
    let value = Value::decode(&mut r).ok()?;
    r.is_empty().then(|| (args.to_vec(), value))
}

impl<D: DataSource> Runner<D> {
    /// Whether resource `i`'s identifying call arguments (not its `with`
    /// context) read, directly or through derives, a store-reading resource
    /// still on its fallback, its compiled answer or placeholder, before its
    /// source is ready: its identity is not yet known, so its kept answer
    /// must wait. An account resource with no kept answer of its own shows
    /// its placeholder (`did: ""`), and that empty id would match whatever
    /// another launch kept (LLP 1027.005 D3, 2026-10-10). Kept seeds,
    /// carried and checkpoint answers and non-store values are known.
    pub(super) fn identity_unknown(&self, i: usize, states: &[Option<ResourceState>]) -> bool {
        use crate::instance::Input;
        let deps = self.sites.deps();
        let row = &self.plan.resources[i];
        let identity = row.args.len as usize - usize::from(row.context);
        let mut work: Vec<_> = row
            .args
            .iter()
            .take(identity)
            .map(|a| deps.reads_of(&self.plan, self.plan.arg(a).expr))
            .collect();
        let mut derives = vec![false; self.plan.derives.len()];
        while let Some(reads) = work.pop() {
            for input in deps.inputs(&reads) {
                match input {
                    Input::Resource(r) | Input::PendingResource(r) | Input::FailedResource(r)
                        if self.store_readers[r]
                            && states[r]
                                .as_ref()
                                .is_some_and(|s| s.placeholder || s.value.is_compiled()) =>
                    {
                        return true;
                    }
                    Input::Derive(d) if !derives[d] => {
                        derives[d] = true;
                        work.push(deps.derives[d].clone());
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// What the first frame shows from the bake until its source is ready
    /// (LLP 1048.003 D6; feed F24: say so in the journal), and each store
    /// reader whose build-time answer a store holding secrets passed over
    /// for its placeholder (LLP 1027 D4, 2026-10-10).
    pub(super) fn journal_build_time(&mut self) {
        let secrets = self.store.opened_with_secrets();
        for i in 0..self.stale.len() {
            let Some(state) = self.resources[i].as_ref().filter(|_| self.stale[i]) else {
                continue;
            };
            let name = self.plan.str(self.plan.resources[i].name);
            let line = if state.value.is_compiled() {
                super::lines::build_time(name)
            } else if state.placeholder
                && secrets
                && self.store_readers[i]
                && self.plan.resources[i].initial.len > 0
            {
                super::lines::secrets_unseen(name)
            } else {
                continue;
            };
            self.log(line);
        }
    }

    /// Journal each kept answer the first frame shows, with the arguments it
    /// was kept for (development tools read the journal; its request lines
    /// already carry URLs), so a stale answer is traceable to its key.
    pub(super) fn journal_kept_seeds(&mut self) {
        self.journal_refused();
        for i in 0..self.plan.resources.len() {
            if let Some(state) = self.resources[i].as_ref().filter(|s| s.kept_seed) {
                let mut args = String::new();
                crate::agent::untyped_json(&Value::list(state.args.clone()), &mut args);
                let line =
                    super::lines::kept_shown(self.plan.str(self.plan.resources[i].name), &args);
                self.log(line);
            }
        }
    }

    /// Journal the kept answers refused by a settlement that stood: at
    /// boot, and at a later commit before readiness (an identity changed).
    pub(super) fn journal_refused(&mut self) {
        for (_, line) in std::mem::take(&mut self.keeping.refused) {
            self.log(line);
        }
    }

    /// The kept answer that may seed resource `i`'s first frame: its
    /// identifying arguments and value, when they still decode, count as
    /// many arguments as the call identifies and fit its shape; otherwise
    /// the refusal is noted for the journal.
    pub(super) fn kept_seed(&mut self, i: usize) -> Option<(Vec<Value>, Value)> {
        let row = &self.plan.resources[i];
        let name = self.plan.str(row.name);
        let entry = self.store.kept(&kept_name(name))?;
        let identity = row.args.len as usize - usize::from(row.context);
        let seed = decode(entry)
            .filter(|(args, value)| args.len() == identity && self.check_shape(i, value).is_ok());
        if seed.is_none() {
            let line = super::lines::kept_refused(name, None, "it no longer decodes or fits");
            self.keeping.refused.push((i, line));
        }
        seed
    }

    /// Before the source is ready, each store reader is asked again at
    /// `data_ready`, and shows the answer kept from the last launch where it
    /// still fits (`seeded`: a document's answers already seeded it). A
    /// launch that begins with a presence secret locked or pending shows
    /// none (LLP 1133.010 D5): a kept answer is the account's content, and
    /// the person has not passed the check yet.
    pub(super) fn seed_kept(&mut self, seeded: &[bool], device_state: bool) {
        let locked = self.store.opened_locked();
        if locked {
            self.log("kept answers not shown: the launch begins with a presence secret locked (LLP 1133.010 D5)");
        }
        for (i, &taken) in seeded.iter().enumerate() {
            if !self.plan.resources[i].reader
                || exact_plan::runner_owned_source(self.plan.str(self.plan.resources[i].source))
            {
                continue;
            }
            // An answer the runtime already has isn't asked again when
            // the module loads, unless the device's store may change it.
            if taken && !device_state {
                continue;
            }
            self.stale[i] = true;
            if self.resources[i].is_some() || locked {
                continue;
            }
            if let Some((args, value)) = self.kept_seed(i) {
                self.resources[i] = Some(ResourceState {
                    inspection_generation: 0,
                    args,
                    value: crate::held::Held::new(value),
                    store_revision: self.store.revision(),
                    placeholder: false,
                    kept_seed: true,
                    answered_for: None,
                    origin: 0,
                });
            }
        }
    }

    /// The first frame's kept answers: a store reader's while the data
    /// source is not ready, and a kept module source's always.
    pub(super) fn seed_at_boot(
        &mut self,
        seeded: &[bool],
        device_state: bool,
        ready: bool,
        private: bool,
    ) {
        if !ready {
            self.seed_kept(seeded, device_state);
        }
        self.seed_kept_modules(ready, private);
    }

    /// Each resource reading a kept module source (LLP 1133.015 D4) shows
    /// the answer kept from the last launch where it still fits, and is
    /// asked again: at `data_ready` when the data source is not ready yet,
    /// else at once. The seed is provisional, as any kept answer is. A
    /// private session keeps none.
    pub(super) fn seed_kept_modules(&mut self, ready: bool, private: bool) {
        self.keeping.modules = !private;
        if private || self.store.opened_locked() {
            return;
        }
        for i in 0..self.plan.resources.len() {
            if !kept_module(&self.plan, i) || self.resources[i].is_some() {
                continue;
            }
            let Some((args, value)) = self.kept_seed(i) else {
                continue;
            };
            self.resources[i] = Some(ResourceState {
                inspection_generation: 0,
                args,
                value: crate::held::Held::new(value),
                store_revision: self.store.revision(),
                placeholder: false,
                kept_seed: true,
                answered_for: None,
                origin: 0,
            });
            match ready {
                true => self.refresh_next.push(i),
                false => self.stale[i] = true,
            }
        }
    }

    /// Keep a store-reading resource's fresh answer for the next boot's
    /// first frame (LLP 1027 D4), when this source may not be ready then.
    pub(super) fn keep_answer(&mut self, i: usize, args: &[Value], value: &Value) {
        let module = self.keeping.modules && kept_module(&self.plan, i);
        if !module && (!self.keeping.on || !self.store_readers[i]) {
            return;
        }
        // @ref LLP 1039 D3 / LLP 1030 D7 — runner facts are never kept answers.
        let source = self.plan.str(self.plan.resources[i].source);
        if exact_plan::runner_owned_source(source) {
            return;
        }
        // @ref LLP 1027.005 D6 — context reaches the source but does not
        // enter the kept entry or its budget.
        let row = &self.plan.resources[i];
        let args = &args[..row.args.len as usize - usize::from(row.context)];
        let name = kept_name(self.plan.str(row.name));
        if !fits(args, value) {
            // An older answer kept for it would show stale at the next launch: forget it (with the commit), and
            // say so once
            self.store.forget_kept(&name);
            if !self.keeping.too_big.contains(&i) {
                self.keeping.too_big.push(i);
                let line = format!(
                    "too big to keep: resource `{}` answered with more than {KEPT_DATA_BYTES} bytes ({kib} KiB) counting its identifying arguments, so the next launch cannot show this answer before the resource answers again. To show it at launch, keep the answer and its identifying arguments under {kib} KiB: move what the first screen does not need into another resource, and pass large inputs that do not identify the answer (a token, a time) after `with`, which are not counted; an input that identifies it stays in the call, or becomes a smaller key (LLP 1027 D4)",
                    self.plan.str(row.name),
                    kib = KEPT_DATA_BYTES / 1024,
                );
                self.log(line);
            }
            return;
        }
        self.say_of_launch(i, args);
        let entry = encode(args, value);
        self.make_room(&name, entry.len());
        self.store.keep(&name, entry);
    }

    /// Forget other kept answers, the longest first, until `len` more bytes
    /// under `name` keep the entries within [`KEPT_TOTAL_BYTES`]: a kept
    /// answer is a cache, which its resource fills again when it answers.
    pub(super) fn make_room(&mut self, name: &str, len: usize) {
        let mut others: Vec<(String, usize)> = self
            .store
            .kept_lengths()
            .into_iter()
            .filter(|(other, _)| other != name)
            .collect();
        let mut total = len + others.iter().map(|(_, n)| n).sum::<usize>();
        // The uploads' marks count, and are never forgotten for an answer (LLP 1016.000 D10).
        others.retain(|(other, _)| other != super::uploads::UPLOADS);
        others.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        for (other, n) in others {
            if total <= KEPT_TOTAL_BYTES {
                break;
            }
            total -= n;
            self.store.forget_kept(&other);
            let line = format!(
                "kept answers are at most {} KiB together: forgot `{}`'s ({n} bytes) to keep `{}`'s, so the next launch shows `{0}`'s placeholder until it answers (LLP 1027 D4)",
                KEPT_TOTAL_BYTES / 1024,
                &other[Store::KEPT.len()..],
                &name[Store::KEPT.len()..],
            );
            self.log(line);
        }
    }

    /// Say once that resource `i` is kept under an identifying argument of
    /// this launch's own, so the next launch never matches it (LLP
    /// 1027.005): one that reads an `exactTime()` resource and is its
    /// `epochAtZero` or `seed` as they are. The compiler names one written
    /// so; this finds one it could not follow. A time made from the clock
    /// (a record's creation, saved) may identify the answer: not said.
    fn say_of_launch(&mut self, i: usize, args: &[Value]) {
        if self.keeping.of_launch.contains(&i) {
            return;
        }
        let epoch = self.time.epoch_at_zero;
        let seed = self.place().seed;
        let Some((k, what)) = args.iter().enumerate().find_map(|(k, arg)| {
            let &Value::Number(v) = arg else { return None };
            let what = if epoch > 0.0 && v == epoch {
                "`epochAtZero`"
            } else if seed > 0.0 && v == seed {
                "`seed`"
            } else {
                return None;
            };
            self.reads_clock(i, k).then_some((k, what))
        }) else {
            return;
        };
        self.keeping.of_launch.push(i);
        let row = &self.plan.resources[i];
        let line = format!(
            "`{name}`'s kept answer is never shown at launch: its identifying argument {n} is this launch's {what}, which changes every launch. Pass it after `with` (`resource {name} = {source}(…) with …`), which still reaches the source but is not counted (LLP 1027.005)",
            name = self.plan.str(row.name),
            source = self.plan.str(row.source),
            n = k + 1,
        );
        self.log(line);
    }

    /// Whether resource `i`'s argument `k` reads, directly or through
    /// derives, an `exactTime()` resource.
    fn reads_clock(&self, i: usize, k: usize) -> bool {
        use crate::instance::Input;
        let deps = self.sites.deps();
        let Some(arg) = self.plan.resources[i].args.iter().nth(k) else {
            return false;
        };
        let mut work = vec![deps.reads_of(&self.plan, self.plan.arg(arg).expr)];
        let mut derives = vec![false; self.plan.derives.len()];
        while let Some(reads) = work.pop() {
            for input in deps.inputs(&reads) {
                match input {
                    Input::Resource(r) | Input::PendingResource(r) | Input::FailedResource(r)
                        if self.plan.str(self.plan.resources[r].source) == crate::time::SOURCE =>
                    {
                        return true;
                    }
                    Input::Derive(d) if !derives[d] => {
                        derives[d] = true;
                        work.push(deps.derives[d].clone());
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// A peer committed to the app's store (LLP 1126.001 WP12). The host
    /// supplies a fresh snapshot; only store readers and persisted slots
    /// are refreshed. Ordinary window state and pure resources stay local.
    pub fn store_changed(
        &mut self,
        snapshot: Vec<(String, Vec<u8>)>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        let result = self.restore_store(snapshot).and_then(|change| {
            if !change.any {
                return Ok(None);
            }
            if change.read {
                let readers = self.store_readers();
                self.owe(&readers);
                self.refresh_next.extend(readers);
            }
            if self.poisoned {
                return Err(RunnerError::Poisoned);
            }
            self.settle(false)
                .and_then(|_| self.gate_step())
                .and_then(|_| self.update())
                .map(Some)
        });
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_next(result.is_ok());
        match result {
            Ok(receipt) => {
                if let Some(receipt) = &receipt {
                    self.log_outcome("store changed", &Ok(receipt.clone()), was_poisoned);
                }
                Ok(receipt)
            }
            Err(error) => {
                let refused = Err(error);
                self.log_outcome("store changed", &refused, was_poisoned);
                refused.map(Some)
            }
        }
    }

    /// Adopt a pre-pixel change without asking; `data_ready_after_store`
    /// performs the activation's one settlement, including queued sends.
    pub fn adopt_store(
        &mut self,
        snapshot: Vec<(String, Vec<u8>)>,
    ) -> Result<crate::StoreChange, RunnerError> {
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        let result = self.restore_store(snapshot);
        self.conclude(checkpoint, &result, was_poisoned);
        result
    }

    fn restore_store(
        &mut self,
        snapshot: Vec<(String, Vec<u8>)>,
    ) -> Result<crate::StoreChange, RunnerError> {
        let change = self.store.refresh_snapshot(snapshot);
        if !change.any {
            return Ok(change);
        }
        for i in 0..self.slots.len() {
            if self.plan.slots[i].persist {
                let initial = self.eval(self.plan.slots[i].init, &[], &[])?;
                self.slots[i] = self.restore_persisted(i, initial);
            }
        }
        Ok(change)
    }

    pub(super) fn store_readers(&self) -> Vec<usize> {
        self.store_readers
            .iter()
            .enumerate()
            .filter_map(|(i, reads)| {
                (*reads
                    && !exact_plan::runner_owned_source(
                        self.plan.str(self.plan.resources[i].source),
                    ))
                .then_some(i)
            })
            .collect()
    }

    /// The data source is ready — a host loaded its TypeScript module after
    /// the first pixel (LLP 1027 D4): every deferred resource shown
    /// from a placeholder is asked again, and every send made before now
    /// is sent, in one commit. `None` when nothing was waiting, or when the
    /// source is still not ready. Hosts call it once, so a stand-in's
    /// resource is asked here or not at all: that ask never refuses the
    /// commit, and a refusal fails the resource, which `refresh` asks again,
    /// as after a failed reply.
    pub fn data_ready(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        self.data_ready_after_store(crate::StoreChange::default())
    }

    /// Activation after adopting a peer's pre-pixel store revision.
    pub fn data_ready_after_store(
        &mut self,
        change: crate::StoreChange,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if !self.data.ready() {
            self.log("data_ready: the data source is not ready");
            return Ok(None);
        }
        // A pending presence move runs once the source has loaded (LLP
        // 1133.010 D4), so the store's readers see it from this commit on.
        self.presence_ready();
        let readers = if change.read {
            self.store_readers()
        } else {
            Vec::new()
        };
        let stale: Vec<usize> = (0..self.stale.len())
            .filter(|i| self.stale[*i] || readers.contains(i))
            .collect();
        // Writes made before the source was ready are overlaid from here
        self.overlays.iter_mut().for_each(|o| *o = None);
        if self.unsent.is_empty() {
            if !change.any && stale.is_empty() && self.writes.records.is_empty() {
                return Ok(None);
            }
            self.owe(&stale);
            return self.commit_again(stale, "data_ready").map(Some);
        }
        let what = format!(
            "data_ready ({} asked again, {} sent)",
            stale.len(),
            self.unsent.len()
        );
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        self.refresh_next.extend(&stale);
        self.owe(&stale);
        let result = if self.poisoned {
            Err(RunnerError::Poisoned)
        } else {
            let later = self.send_unsent();
            // The gate step joins this settlement as every commit's (LLP
            // 1092 D8; b6 review A2): a gate the sent answers open arms its
            // task, and a key that is no key refuses this commit.
            match self
                .router_change()
                .and_then(|_| self.settle(false))
                .and_then(|_| self.gate_step())
            {
                Ok(()) => {
                    for (m, source, args, request) in later {
                        self.enqueue(super::Target::Mutation(m), source, args, request, false);
                    }
                    self.update()
                }
                Err(e) => {
                    self.discard_later(&later);
                    Err(e)
                }
            }
        };
        self.conclude(checkpoint, &result, was_poisoned);
        let dropped = result.is_err() && !self.poisoned;
        // Refused, the sends it asked are not asked again (a host makes one
        // `data_ready`): they are dropped, said in the journal, and their
        // writes end.
        if dropped {
            for (m, ..) in std::mem::take(&mut self.unsent) {
                let name = self.plan.str(self.plan.mutations[m].name).to_string();
                self.log(super::lines::unsent_refused(
                    &name,
                    "data_ready was refused",
                ));
                self.end_write(m, None);
            }
            self.sync_pending_flags();
        }
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        // The refusal is in the journal; the commit that ends them, and asks
        // what `data_ready` owed, is the host's to apply, as after a failed
        // reply.
        if dropped {
            self.owe(&stale);
            return self
                .commit_again(stale, "the sends a refused data_ready dropped")
                .map(Some);
        }
        result.map(Some)
    }

    /// Each waiting send asks its source now, as the action would have: an
    /// answer lands in the mutation's slot (its `then` armed, what it
    /// refreshes asked again); a request is returned, to go out once the
    /// commit stands. The action that made it committed long ago, so a
    /// source's refusal ends that one send, said in the journal, as a
    /// refused reply does — never this commit.
    #[allow(clippy::type_complexity)]
    fn send_unsent(&mut self) -> Vec<(usize, String, Vec<Value>, super::Request)> {
        let mut later = Vec::new();
        for (m, source, args) in std::mem::take(&mut self.unsent) {
            let target = super::Target::Mutation(m);
            let name = self.plan.str(self.plan.mutations[m].name).to_string();
            self.mutation_requests += 1;
            let answer = if Self::activity_send(&source) {
                self.activity_request(&source, &args)
            } else {
                self.store.set_clock_ms(self.now_ms);
                let answer = self
                    .data
                    .answer_for(target, &mut self.store, &source, &args);
                self.refuse_locked(answer)
            };
            // What its turn sent as an answer (LLP 1016.000 D7, the amendment
            // of 2026-10-18).
            self.stage_answer(target);
            let refused = match answer {
                Ok(super::Answer::Now(value))
                    if self.conforms(&value, self.plan.mutations[m].ty) =>
                {
                    match self.mutation_slot(m) {
                        Ok(slot) => {
                            self.forget_ticket(super::Target::Mutation(m));
                            self.land_write(m, None, &value);
                            self.slots[slot] = Value::some(value);
                            self.landed.push(m);
                            for r in self.declared_refreshes(m) {
                                self.force_refresh(r);
                            }
                            None
                        }
                        Err(e) => Some(format!("{e:?}")),
                    }
                }
                Ok(super::Answer::Now(_)) => {
                    Some("its answer does not fit the mutation's shape".to_string())
                }
                Ok(super::Answer::Later(request)) => {
                    later.push((m, source, args, request));
                    None
                }
                Err(error) => Some(format!("{error:?}")),
            };
            if let Some(why) = refused {
                self.log(super::lines::unsent_refused(&name, &why));
                // Its write ends; the resources it showed in drop it in
                // this commit, and a refusal of the commit does not bring
                // it back.
                self.refused_unsent.push((m, self.asked_write(m)));
                self.end_write(m, None);
            }
        }
        self.sync_pending_flags();
        // Pending until the request goes out with the commit, or is let go.
        for (m, ..) in &later {
            self.pending_mut[*m] = true;
        }
        later
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_answer_round_trips_and_a_damaged_one_is_not_used() {
        let args = vec![Value::str("mv"), Value::Number(3.0)];
        let value = Value::record(vec![Value::Bool(true), Value::str("ada")]);
        let bytes = encode(&args, &value);
        let (a, v) = decode(&bytes).unwrap();
        assert_eq!(a, args);
        assert_eq!(v.to_bytes(), value.to_bytes());
        assert!(decode(b"").is_none());
        assert!(decode(&bytes[..bytes.len() - 1]).is_none());
        // An entry kept as hex text, or bytes that are not an arguments list then one answer, is not used
        assert!(decode(b"0500000000|00").is_none());
        let mut trailing = bytes.clone();
        trailing.push(0);
        for not_an_entry in [
            Value::list(args.clone()).to_bytes(),
            [Value::str("mv").to_bytes(), value.to_bytes()].concat(),
            trailing,
        ] {
            assert!(decode(&not_an_entry).is_none());
        }
        // The deepest answer a value may be still decodes: keeping it costs no level
        let mut deep = Value::Number(1.0);
        for _ in 0..64 {
            deep = Value::record(vec![deep]);
        }
        assert!(decode(&encode(&args, &deep)).is_some());
        assert_eq!(kept_name("remembered"), "exact.kept.remembered");
    }

    #[test]
    fn size_refusal_matches_the_wire_budget_including_arguments_and_unicode() {
        let values = [
            Value::Unit,
            Value::NONE,
            Value::some(Value::Number(-0.0)),
            Value::Bool(true),
            Value::str("é💬"),
            Value::list(vec![Value::Bool(false), Value::Unit]),
            Value::record(vec![Value::str("field"), Value::Number(2.0)]),
        ];
        for value in &values {
            for args in [&[][..], &values[..]] {
                assert_eq!(
                    fits(args, value),
                    encode(args, value).len() <= MAX_KEPT_BYTES
                );
            }
        }
        // The data budget's edge: with no arguments, a string of the budget less its 5-byte header fits, one more does not.
        let edge = KEPT_DATA_BYTES - 5;
        for size in [edge - 1, edge, edge + 1, 2 * KEPT_DATA_BYTES] {
            let value = Value::str(&"x".repeat(size));
            for args in [&[][..], &values[..]] {
                assert_eq!(
                    fits(args, &value),
                    encode(args, &value).len() <= MAX_KEPT_BYTES
                );
            }
        }
        assert!(!fits(
            &[],
            &Value::list(vec![Value::Unit; KEPT_DATA_BYTES + 1])
        ));
    }
}
