//! Presentation-only clock work: no settlement, handlers or app data calls.
//! @ref LLP 1126 D6
use super::*;
use exact_kernel::time::{duration_shape, instant, relative_delay, reservation, TimeRequest};
use exact_kernel::{Op, PropId as P, PropList, PropValue};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct LiveTimes(BTreeMap<ViewId, Live>);

#[derive(Clone, Default)]
struct Live {
    props: PropList,
    due: Option<f64>,
    text: Option<String>,
    reserve: Option<String>,
    value: Option<String>,
    reservation_key: String,
    context: String,
    duration_shape: u32,
}

impl LiveTimes {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn due(&self) -> Option<f64> {
        self.0.values().filter_map(|v| v.due).reduce(f64::min)
    }
}

impl<D: DataSource> Runner<D> {
    pub(super) fn tick_live_times(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let mut ops = Vec::new();
        let next = self.prepare_live_times(&mut ops);
        if ops.is_empty() {
            self.live_times = next;
            return Ok(None);
        }
        self.batch += 1;
        let receipt = self.kernel.apply(0, self.batch, &ops)?;
        self.live_times = next;
        Ok(Some(receipt))
    }

    pub(super) fn prepare_live_times(&mut self, ops: &mut Vec<Op>) -> LiveTimes {
        let mut next = self.live_times.clone();
        let mut changed = BTreeSet::new();
        for op in ops.iter() {
            match op {
                Op::SetProp { id, prop, value }
                    if matches!(
                        prop,
                        P::SemanticTag | P::Type | P::Datetime | P::TimeFormat | P::Start | P::End
                    ) =>
                {
                    let live = next.0.entry(*id).or_default();
                    if live.props.get(*prop) != Some(value) {
                        live.props.set(*prop, value.clone());
                        changed.insert(*id);
                    }
                }
                Op::ClearProp { id, prop } => {
                    if let Some(live) = next.0.get_mut(id) {
                        if live.props.remove(*prop).is_some() {
                            changed.insert(*id);
                        }
                    }
                }
                Op::DestroyView { id } => {
                    if let Some(node) = self.kernel.node(*id) {
                        let arena = self.kernel.arena();
                        for slot in arena.subtree(node.key.index) {
                            next.0.remove(&arena.local_id(slot));
                        }
                    }
                    next.0.remove(id);
                }
                _ => {}
            }
        }
        next.0.retain(|_, live| {
            live.props.str(P::SemanticTag) == Some("time")
                || live.props.str(P::Type) == Some("progress")
        });
        let now = self.time.epoch_at_zero + self.now_ms;
        let context = format!(
            "{}/{}/{}/{}",
            self.time.epoch_at_zero, self.time.utc_offset, self.place.locale, self.place.time_zone
        );
        for (&id, live) in &mut next.0 {
            if !changed.contains(&id)
                && live.context == context
                && live.due.is_none_or(|due| due > self.now_ms)
            {
                continue;
            }
            live.context.clone_from(&context);
            let number = |p| {
                live.props
                    .get(p)
                    .and_then(PropValue::as_float)
                    .filter(|n| instant(*n))
            };
            if live.props.str(P::SemanticTag) == Some("time") {
                let text = number(P::Datetime).and_then(|datetime| {
                    let format = live.props.str(P::TimeFormat).unwrap_or("date-medium");
                    let shown_now = if format == "relative" {
                        datetime - ((datetime - now) / 60_000.0).ceil() * 60_000.0
                    } else {
                        now
                    };
                    let request = TimeRequest {
                        datetime,
                        now: shown_now,
                        format,
                        locale: &self.place.locale,
                        time_zone: &self.place.time_zone,
                        utc_offset: self.time.utc_offset,
                    };
                    let text = self.kernel.format_time(&request)?;
                    let key = format!(
                        "{datetime}/{}/{}/{}/{}/{}",
                        request.format,
                        request.locale,
                        request.time_zone,
                        request.utc_offset,
                        self.time.epoch_at_zero
                    );
                    let identity = key;
                    let reset = !live.reservation_key.starts_with(&format!("{identity}/"));
                    if reset {
                        live.duration_shape = duration_shape(&request);
                    }
                    if format == "timer" {
                        live.duration_shape = live.duration_shape.max(duration_shape(&request));
                    }
                    let key = format!("{identity}/{}", live.duration_shape);
                    if key != live.reservation_key {
                        let reserve = reservation(&request, |r| self.kernel.format_time(r));
                        put(
                            id,
                            P::TextReservation,
                            &mut live.reserve,
                            Some(reserve),
                            ops,
                        );
                        live.reservation_key = key;
                    }
                    live.due = match request.format {
                        "relative" => Some(self.now_ms + relative_delay(datetime, now)),
                        "timer" | "countdown"
                            if request.format != "countdown" || datetime > now =>
                        {
                            Some(self.now_ms + 1000.0 - (now - datetime).rem_euclid(1000.0))
                        }
                        _ => None,
                    };
                    Some(text)
                });
                if text.is_none() {
                    live.due = None;
                    live.reservation_key.clear();
                    put(id, P::TextReservation, &mut live.reserve, None, ops);
                }
                put(id, P::Text, &mut live.text, text, ops);
            } else {
                let interval = number(P::Start).zip(number(P::End)).filter(|(a, b)| b > a);
                if let Some((start, end)) = interval {
                    let value = ((now - start) / (end - start)).clamp(0.0, 1.0);
                    put(
                        id,
                        P::Value,
                        &mut live.value,
                        Some(exact_num::js(value)),
                        ops,
                    );
                    if self
                        .kernel
                        .node(id)
                        .is_none_or(|n| n.props.str(P::Max) != Some("1"))
                    {
                        ops.push(Op::SetProp {
                            id,
                            prop: P::Max,
                            value: "1".into(),
                        });
                    }
                    live.due = (now < end).then_some(if now < start {
                        start - self.time.epoch_at_zero
                    } else {
                        (self.now_ms + 1000.0 - (now - start).rem_euclid(1000.0))
                            .min(end - self.time.epoch_at_zero)
                    });
                } else if live.value.is_some() {
                    put(id, P::Value, &mut live.value, None, ops);
                    ops.push(Op::ClearProp { id, prop: P::Max });
                    live.due = None;
                }
            }
        }
        next
    }
}

fn put(id: ViewId, prop: P, old: &mut Option<String>, value: Option<String>, ops: &mut Vec<Op>) {
    if *old == value {
        return;
    }
    ops.push(match &value {
        Some(value) => Op::SetProp {
            id,
            prop,
            value: value.clone().into(),
        },
        None => Op::ClearProp { id, prop },
    });
    *old = value;
}
