//! Typed simulation events. Both pending and delivered batches belong to a hashed resource.
use crate::{Data, DataError, Input, Reader, Resource, World, Writer};
use std::{any::Any, collections::BTreeMap, sync::Arc};

/// A registered record produced by a tick for the next tick.
pub trait TypedEvent: Data + Send + Sync {
    /// Agent-facing type spelling.
    const NAME: &'static str;
    /// Resource name, generated as `events:Type` by the derive.
    const QUEUE_NAME: &'static str;
    /// Maximum emissions per tick; overflow refuses without changing the queue.
    const CAPACITY: usize = 256;
}
#[derive(Default, Clone)]
pub(crate) struct InputEvents(pub BTreeMap<&'static str, Arc<dyn Any + Send + Sync>>);
struct Queue<E: TypedEvent> {
    pending: Vec<E>,
    delivered: Arc<Vec<E>>,
    tick: u64,
}
impl<E: TypedEvent> Default for Queue<E> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            delivered: Arc::new(Vec::new()),
            tick: 0,
        }
    }
}
impl<E: TypedEvent> Resource for Queue<E> {
    const NAME: &'static str = E::QUEUE_NAME;
}
impl<E: TypedEvent> Data for Queue<E> {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("pending");
        self.pending.write(w);
        w.field("delivered");
        self.delivered.as_ref().write(w);
        w.field("tick");
        self.tick.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field.as_str() {
                "pending" => self.pending.read(r)?,
                "delivered" => {
                    let mut events = Vec::<E>::new();
                    events.read(r)?;
                    self.delivered = Arc::new(events);
                }
                "tick" => self.tick.read(r)?,
                _ => r.skip()?,
            }
        }
        if self.pending.len() > E::CAPACITY || self.delivered.len() > E::CAPACITY {
            return Err(DataError::new(format!(
                "event {} exceeds capacity {}",
                E::NAME,
                E::CAPACITY
            )));
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Registration {
    deliver: fn(&World, &mut Input),
    json: fn(&World) -> Result<String, DataError>,
}
impl World {
    /// Register an event and its bounded, saved and hashed queue in `Game::register`.
    pub fn register_event<E: TypedEvent>(&mut self) -> &mut Self {
        assert!(
            E::CAPACITY > 0 && E::CAPACITY <= 65536,
            "event {} capacity must be 1..65536",
            E::NAME
        );
        self.register_resource::<Queue<E>>();
        if self.try_resource::<Queue<E>>().is_none() {
            self.insert_resource(Queue::<E>::default());
        }
        self.event_types.insert(
            E::NAME,
            Registration {
                deliver: |w, input| {
                    {
                        let q = w.resource::<Queue<E>>();
                        if q.pending.is_empty() && q.delivered.is_empty() {
                            return;
                        }
                    }
                    let mut q = w.resource_mut::<Queue<E>>();
                    q.delivered = Arc::new(std::mem::take(&mut q.pending));
                    q.tick = w.tick() + 1;
                    input.events.0.insert(E::NAME, q.delivered.clone());
                },
                json: |w| {
                    let q = w.resource::<Queue<E>>();
                    let data = crate::json::to_string(q.delivered.as_ref())?;
                    Ok(format!(
                        "{{\"type\":{},\"tick\":{},\"records\":{data}}}",
                        crate::values::quote(E::NAME),
                        q.tick
                    ))
                },
            },
        );
        self
    }
    /// Append a simulation event for the next tick; a refusal leaves its queue unchanged.
    pub fn emit_event<E: TypedEvent>(&self, event: E) -> bool {
        self.sim_writes(format_args!("emitted event {}", E::NAME));
        let Some(queue) = self.try_resource::<Queue<E>>() else {
            self.log(format_args!(
                "event {} refused: unregistered; call register_event in Game::register",
                E::NAME
            ));
            return false;
        };
        if queue.pending.len() >= E::CAPACITY {
            drop(queue);
            if self.event_overflow.borrow_mut().insert(E::NAME) {
                self.log(format_args!(
                    "event {} refused: capacity {} per tick",
                    E::NAME,
                    E::CAPACITY
                ));
            }
            return false;
        }
        drop(queue);
        self.resource_mut::<Queue<E>>().pending.push(event);
        true
    }
    pub(crate) fn deliver_events(&self, input: &mut Input) {
        self.event_overflow.borrow_mut().clear();
        input.events.0.clear();
        for reg in self.event_types.values() {
            (reg.deliver)(self, input);
        }
    }
    pub(crate) fn events_json(&self, kind: Option<&str>) -> Result<String, String> {
        if kind.is_some_and(|k| !self.event_types.contains_key(k)) {
            return Err(format!("unknown event type {}", kind.unwrap()));
        }
        let rows = self
            .event_types
            .iter()
            .filter(|(name, _)| kind.is_none_or(|k| k == **name))
            .map(|(_, r)| (r.json)(self).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("[{}]", rows.join(",")))
    }
}

#[cfg(test)]
mod tests;
