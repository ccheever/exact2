//! `perf <target>`: the work a subtree's plan sites did, totalled across
//! their instances' lifetimes (LLP 1079 D1–D2).
//!
//! @ref LLP 1079 D1 (what is counted, in which unit), D2 (the read)
//!
//! The counters live beside the instance ids ([`crate::instance::Ids`]),
//! because every instance is realized there, and grow only while a host
//! measures ([`Runner::measure`]). A read changes nothing: two reads tagged
//! with the same `incarnation` and `plan` are subtracted by the driver.

use crate::agent::{error, field_bool, num, quote};
use crate::runner::{DataSource, Runner};
use exact_plan::NodesId;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The most instances one read walks, and the most sites it returns; past
/// either the reply says `truncated` (D2).
pub const WALK_LIMIT: usize = 20_000;
/// See [`WALK_LIMIT`].
pub const SITE_LIMIT: usize = 500;

/// One plan site's work since measuring began, across every instance of it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SiteWork {
    /// Instances realized, retired ones included (D1 `created`).
    pub created: u64,
    /// Retired instances the runner has forgotten ([`crate::instance::Ids::retain`]);
    /// a read adds the remembered ones no longer live.
    pub forgotten: u64,
    /// Views already gone when measuring began: in `forgotten` or among the
    /// remembered dead, and never this site's retirements.
    pub before: u64,
    /// Binding expressions evaluated for its instances.
    pub evaluated: u64,
    /// Of those, the ones whose result equalled the last, so no op was built.
    pub unchanged: u64,
    /// Transactions whose receipt touched an instance through an op naming it.
    pub authored: u64,
    /// Transactions whose receipt touched an instance with no op naming it:
    /// an inherited value, the document's language or direction, a resolved
    /// relative unit.
    pub inherited: u64,
    /// Layout passes that moved an instance's frame (a host that lays out).
    pub moved: u64,
}

/// What is measured: a row per plan node, empty while nothing is.
#[derive(Debug, Default)]
pub struct Work {
    /// By node index.
    pub sites: Vec<SiteWork>,
    /// The host lays out and reports frames that moved ([`Runner::moved`]).
    pub moves: bool,
    /// The last transaction a batch trailer reported ([`Runner::seq_range`]).
    pub reported: std::cell::Cell<u64>,
}

/// The node `site` sits under in the plan, through its region's arm when it
/// has no node parent (the walk `Plan::validate` bounds).
fn static_parent(plan: &exact_plan::Plan, site: NodesId) -> Option<NodesId> {
    let node = plan.node(site);
    let (mut parent, mut arm) = (node.parent, node.arm);
    loop {
        match (parent, arm) {
            (Some(p), _) => return Some(p),
            (None, Some(a)) => {
                let region = &plan.regions[plan.arm(a).region.0 as usize];
                (parent, arm) = (region.parent, region.arm);
            }
            (None, None) => return None,
        }
    }
}

/// `{"op":"perf"[,"target":V]}`: the sites under the target (every root's
/// without one), each with its live instances under the target and its
/// site-wide totals. `{"op":"perf","frames":true}` is the host's (D3); one
/// that reaches here observes no presentation.
pub fn reply<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    if field_bool(request, "frames") {
        return "{\"unavailable\":true}".into();
    }
    let ids = runner.ids();
    if ids.work.sites.is_empty() {
        return error("perf: this build measures no work (production trust, LLP 1079 D1)");
    }
    let kernel = runner.kernel();
    let plan = runner.plan();
    let (roots, target) = match crate::agent::target(runner, request) {
        Ok(Some((view, _))) => (vec![view], Some(view)),
        Ok(None) => (runner.roots(), None),
        Err(e) => return error(&e),
    };
    // Live instances under the target, by site: one pass over the kernel's
    // children, a site lookup per view and nothing else materialized.
    let arena = kernel.arena();
    let mut sites: BTreeMap<u32, u64> = BTreeMap::new();
    let mut stack: Vec<u32> = roots.iter().filter_map(|v| arena.slot_of(*v)).collect();
    let (mut walked, mut truncated) = (0usize, false);
    while let Some(slot) = stack.pop() {
        if walked == WALK_LIMIT {
            truncated = true;
            break;
        }
        walked += 1;
        if let Some(site) = ids.site(arena.local_id(slot)) {
            *sites.entry(site.0).or_default() += 1;
        }
        stack.extend(arena.children(slot).iter().rev());
    }
    // A site whose instances have all gone still has totals to show: one
    // sitting statically under the target's site, or, reading every root,
    // any site that did work.
    if target.is_none() {
        for (i, w) in ids.work.sites.iter().enumerate() {
            if *w != SiteWork::default() {
                sites.entry(i as u32).or_default();
            }
        }
    } else if let Some(top) = target.and_then(|v| ids.site(v)) {
        for i in 0..plan.nodes.len() {
            let mut at = Some(NodesId(i as u32));
            while let Some(site) = at {
                if site == top {
                    sites.entry(i as u32).or_default();
                    break;
                }
                at = static_parent(plan, site);
            }
        }
    }
    if sites.len() > SITE_LIMIT {
        truncated = true;
        while sites.len() > SITE_LIMIT {
            sites.pop_last();
        }
    }
    // Site-wide liveness: what the runner remembers, live or not.
    let mut gone: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    for (view, site) in ids.sites() {
        if sites.contains_key(&site.0) {
            let e = gone.entry(site.0).or_default();
            if kernel.node(view).is_some() {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
    }
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"seq\":{},\"plan\":",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms()),
        runner.seq()
    );
    quote(runner.inspection_digest(), &mut s);
    s.push_str(",\"sites\":[");
    for (i, (site, instances)) in sites.iter().enumerate() {
        let w = ids.work.sites[*site as usize];
        let (live, dead) = gone.get(site).copied().unwrap_or_default();
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"site\":{site},\"instances\":{instances},\"live\":{live},\"created\":{},\"retired\":{},\"evaluated\":{},\"unchanged\":{},\"authored\":{},\"inherited\":{}",
            w.created,
            (w.forgotten + dead).saturating_sub(w.before),
            w.evaluated,
            w.unchanged,
            w.authored,
            w.inherited
        );
        if ids.work.moves {
            let _ = write!(s, ",\"moved\":{}", w.moved);
        }
        s.push('}');
    }
    let _ = write!(s, "],\"walked\":{walked},\"truncated\":{truncated}}}");
    s
}
