//! Presentation-only clock work: no settlement, handlers or app data calls.
//! @ref LLP 1126 D6
use super::*;
use exact_kernel::time::{duration_shape, instant, relative_delay, reservation, TimeRequest};
use exact_kernel::{Op, PropId as P, PropList, PropValue};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
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

#[derive(Default)]
pub(super) struct LiveUndo(BTreeMap<ViewId, Option<Live>>);

impl LiveUndo {
    fn remember(&mut self, times: &LiveTimes, id: ViewId) {
        self.0
            .entry(id)
            .or_insert_with(|| times.0.get(&id).cloned());
    }

    pub(super) fn restore(self, times: &mut LiveTimes) {
        for (id, live) in self.0 {
            if let Some(live) = live {
                times.0.insert(id, live);
            } else {
                times.0.remove(&id);
            }
        }
    }
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
        let undo = self.prepare_live_times(&mut ops);
        if ops.is_empty() {
            return Ok(None);
        }
        self.batch += 1;
        match self.kernel.apply(0, self.batch, &ops) {
            Ok(receipt) => Ok(Some(receipt)),
            Err(error) => {
                undo.restore(&mut self.live_times);
                Err(error.into())
            }
        }
    }

    pub(super) fn prepare_live_times(&mut self, ops: &mut Vec<Op>) -> LiveUndo {
        let mut undo = LiveUndo::default();
        let mut changed = BTreeSet::new();
        for op in ops.iter() {
            match op {
                Op::SetProp { id, prop, value }
                    if matches!(
                        prop,
                        P::SemanticTag | P::Type | P::Datetime | P::TimeFormat | P::Start | P::End
                    ) =>
                {
                    if self.live_times.0.get(id).and_then(|v| v.props.get(*prop)) != Some(value) {
                        undo.remember(&self.live_times, *id);
                        self.live_times
                            .0
                            .entry(*id)
                            .or_default()
                            .props
                            .set(*prop, value.clone());
                        changed.insert(*id);
                    }
                }
                Op::ClearProp { id, prop } => {
                    if self
                        .live_times
                        .0
                        .get(id)
                        .is_some_and(|v| v.props.get(*prop).is_some())
                    {
                        undo.remember(&self.live_times, *id);
                        self.live_times.0.get_mut(id).unwrap().props.remove(*prop);
                        changed.insert(*id);
                    }
                }
                Op::DestroyView { id } => {
                    if let Some(node) = self.kernel.node(*id) {
                        let arena = self.kernel.arena();
                        for slot in arena.subtree(node.key.index) {
                            let id = arena.local_id(slot);
                            if self.live_times.0.contains_key(&id) {
                                undo.remember(&self.live_times, id);
                                self.live_times.0.remove(&id);
                            }
                        }
                    }
                    if self.live_times.0.contains_key(id) {
                        undo.remember(&self.live_times, *id);
                        self.live_times.0.remove(id);
                    }
                }
                _ => {}
            }
        }
        for id in &changed {
            if self.live_times.0.get(id).is_some_and(|live| {
                live.props.str(P::SemanticTag) != Some("time")
                    && live.props.str(P::Type) != Some("progress")
            }) {
                self.live_times.0.remove(id);
            }
        }
        let now = self.time.epoch_at_zero + self.now_ms;
        let context = format!(
            "{}/{}/{}/{}",
            self.time.epoch_at_zero, self.time.utc_offset, self.place.locale, self.place.time_zone
        );
        let due: Vec<_> = self
            .live_times
            .0
            .iter()
            .filter_map(|(&id, live)| {
                (changed.contains(&id)
                    || live.context != context
                    || live.due.is_some_and(|due| due <= self.now_ms))
                .then_some(id)
            })
            .collect();
        for id in due {
            undo.remember(&self.live_times, id);
            let live = self.live_times.0.get_mut(&id).unwrap();
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
                    let reset = !live.reservation_key.starts_with(&format!("{key}/"));
                    if reset {
                        live.duration_shape = duration_shape(&request);
                    }
                    if format == "timer" {
                        live.duration_shape = live.duration_shape.max(duration_shape(&request));
                    }
                    let key = format!("{key}/{}", live.duration_shape);
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
                    for (prop, instant) in [(P::Start, start), (P::End, end)] {
                        if self
                            .kernel
                            .node(id)
                            .and_then(|n| n.props.get(prop))
                            .and_then(PropValue::as_float)
                            != Some(instant)
                        {
                            ops.push(Op::SetProp {
                                id,
                                prop,
                                value: PropValue::Float(instant),
                            });
                        }
                    }
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
                } else {
                    if live.props.get(P::Start).is_some() || live.props.get(P::End).is_some() {
                        ops.extend([P::Start, P::End].map(|prop| Op::ClearProp { id, prop }));
                    }
                    if live.value.is_some() {
                        put(id, P::Value, &mut live.value, None, ops);
                        ops.push(Op::ClearProp { id, prop: P::Max });
                    }
                    live.due = None;
                }
            }
        }
        undo
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

#[cfg(test)]
mod tests {
    use super::*;
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
    }

    #[test]
    fn unchanged_entries_are_not_copied_and_refusal_restores_touched_entries() {
        let mut source = "component App\n  view\n    column\n".to_string();
        for i in 0..300 {
            source.push_str(&format!(
                "      time datetime=-120000 format=\"relative\" testId=\"time-{i}\"\n"
            ));
        }
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let pointers: Vec<_> = r
            .live_times
            .0
            .values()
            .map(|v| v.reserve.as_ref().unwrap().as_ptr())
            .collect();
        let undo = r.prepare_live_times(&mut Vec::new());
        assert!(undo.0.is_empty());
        assert_eq!(
            pointers,
            r.live_times
                .0
                .values()
                .map(|v| v.reserve.as_ref().unwrap().as_ptr())
                .collect::<Vec<_>>()
        );
        let id = *r.live_times.0.keys().next().unwrap();
        let before = r.live_times.0[&id].props.clone();
        assert!(r
            .apply(vec![
                Op::SetProp {
                    id,
                    prop: P::Datetime,
                    value: PropValue::Float(9000000.)
                },
                Op::DestroyView { id: u32::MAX }
            ])
            .is_err());
        assert_eq!(r.live_times.0[&id].props, before);
        assert_eq!(r.live_times.0.len(), 300);
        r.advance(60000.).unwrap();
        assert_eq!(r.live_times.0[&id].text.as_deref(), Some("3 min. ago"));
    }
}
