use super::*;

impl<G: Game> Sim<G> {
    /// Queue a raw event in stamp order, preserving arrival order at equal stamps.
    /// Paused input updates held state directly, without edges or queued wheel deltas.
    pub fn input(&mut self, event: InputEvent) {
        let event = self.unmarked(event);
        if let Err(error) = self.validate_input(&event) {
            self.world.log(error);
            return;
        }
        self.invalidate();
        // A posted message waits for the next tick, paused or not.
        if G::paused(&self.args) && !matches!(event, InputEvent::Message { .. }) {
            self.deliver(event, self.world_us, true);
            return;
        }
        let host_us = micros(event.at_ms());
        let e = Queued {
            host_us,
            world_us: self
                .last_us
                .filter(|last| host_us <= *last)
                .map(|_| self.world_us),
            event,
        };
        let position = self
            .queue
            .partition_point(|old| self.projected(old) <= self.projected(&e));
        if let InputEvent::Key { code, down, .. } = &e.event {
            let previous = self
                .queue
                .range(..position)
                .rev()
                .find_map(|old| match &old.event {
                    InputEvent::Key {
                        code: key, down, ..
                    } if key == code => Some(*down),
                    InputEvent::Blur { .. } => Some(false),
                    _ => None,
                })
                .unwrap_or_else(|| self.input.keys.contains(code));
            if previous == *down {
                return;
            }
        }
        // A host may repeat a pad's state; only a change is an event.
        if let InputEvent::Pad { index, .. } = &e.event {
            if self.pad_before(position, *index) == pad_reading(&e.event) {
                return;
            }
        }
        // Only coalesce across other moves/wheels: contact and key edges retain
        // the positions and device state that were current when they arrived.
        let boundary = self.boundary(&e);
        for i in (0..position).rev() {
            if self.boundary(&self.queue[i]) != boundary {
                break;
            }
            // A pad state that pressed, released or connected keeps its stamp:
            // only a sticks-only state may be replaced by a later one.
            let edge = match &self.queue[i].event {
                InputEvent::Pad { index, .. } => {
                    let before = self.pad_before(i, *index);
                    let at = pad_reading(&self.queue[i].event);
                    before.map(|(s, b, _)| (s, b)) != at.map(|(s, b, _)| (s, b))
                }
                _ => false,
            };
            match (&mut self.queue[i].event, &e.event) {
                (old, new) if old.same_motion(new) && !edge => {
                    let mut e = e;
                    e.event.add_motion(old);
                    self.queue.remove(i);
                    self.queue.insert(position - 1, e);
                    return;
                }
                (InputEvent::Wheel { dx, dy, .. }, InputEvent::Wheel { dx: x, dy: y, .. }) => {
                    *dx += x;
                    *dy += y;
                    return;
                }
                (
                    InputEvent::Pointer {
                        phase: PointerPhase::Move,
                        ..
                    }
                    | InputEvent::Control {
                        phase: PointerPhase::Move,
                        ..
                    }
                    | InputEvent::Wheel { .. },
                    _,
                ) => {}
                _ => break,
            }
        }
        let (mut position, mut e) = (position, e);
        if self.queue.len() < QUEUE_LIMIT {
            // Below the limit an overflow episode is over: the next one journals again.
            self.overflow_logged = false;
            self.posts_logged = false;
        }
        if self.queue.len() == QUEUE_LIMIT {
            // A posted message is never dropped silently: a full queue refuses
            // the new post, by name, and a device event never displaces one.
            let message = |e: &Queued| matches!(e.event, InputEvent::Message { .. });
            let drop = (!message(&e))
                .then(|| {
                    let moves = self.queue.iter().position(|e| e.event.is_move());
                    moves.or_else(|| self.queue.iter().position(|e| !message(e)))
                })
                .flatten();
            let Some(drop) = drop else {
                self.refused_posts += u64::from(message(&e));
                // Once per overflow episode for each kind; state counts each post.
                let logged = if message(&e) {
                    &mut self.posts_logged
                } else {
                    &mut self.overflow_logged
                };
                if !*logged {
                    *logged = true;
                    self.world.log(if message(&e) {
                        "postMessage refused: the input queue holds 1024 events; post less often (state input.refusedPosts counts them)"
                    } else {
                        "input queue overflow: every queued event is a posted message; dropped the new event"
                    });
                }
                return;
            };
            let was_move = self.queue[drop].event.is_move();
            let dropped = self.queue.remove(drop).expect("a queued event");
            position -= usize::from(drop < position);
            // A dropped move's motion joins that pointer's next event, so a
            // flood of moves loses positions, never the turn they add up to.
            if let InputEvent::Pointer { id, .. } = dropped.event {
                let same =
                    |ev: &InputEvent| matches!(ev, InputEvent::Pointer { id: n, .. } if *n == id);
                let (mut before, mut after) = (drop..position, position..self.queue.len());
                if let Some(i) = before.find(|&i| same(&self.queue[i].event)) {
                    self.queue[i].event.add_motion(&dropped.event);
                } else if same(&e.event) {
                    e.event.add_motion(&dropped.event);
                } else if let Some(i) = after.find(|&i| same(&self.queue[i].event)) {
                    self.queue[i].event.add_motion(&dropped.event);
                }
            }
            if !self.overflow_logged {
                self.world.log(if was_move {
                    "input queue overflow: dropped oldest move"
                } else {
                    "input queue overflow: dropped oldest event"
                });
                self.overflow_logged = true;
            }
        }
        self.queue.insert(position, e);
    }
    pub(super) fn flush_paused(&mut self, now: i64) {
        let mut held = VecDeque::new();
        while self
            .queue
            .front()
            .is_some_and(|e| e.world_us.is_some() || e.host_us <= now)
        {
            let e = self.queue.pop_front().unwrap();
            if let InputEvent::Message { .. } = e.event {
                // Restamped at the pause's end, so it reaches the first tick after.
                held.push_back(Queued {
                    world_us: None,
                    host_us: now,
                    ..e
                });
            } else {
                self.deliver(e.event, self.world_us, true);
            }
        }
        held.append(&mut self.queue);
        self.queue = held;
        self.input.clear_edges();
    }
    /// Apply one event at world microsecond `at_us`, journaling a pad's
    /// connection; `paused` keeps no edges.
    pub(super) fn deliver(&mut self, event: InputEvent, at_us: i64, paused: bool) {
        if let InputEvent::Pad {
            index,
            connected,
            standard,
            ..
        } = &event
        {
            if self.input.pad(*index).is_some() != *connected {
                self.world.log(match (connected, standard) {
                    (false, _) => format!("pad {index} disconnected"),
                    (true, true) => format!("pad {index} connected standard"),
                    (true, false) => {
                        format!(
                            "pad {index} connected unmapped: no standard mapping, delivers nothing"
                        )
                    }
                });
            }
        }
        // Every event applies over the world's sensor state, which the last
        // tick may have changed (`World::set_tilt_neutral`).
        self.load_sensors();
        if paused {
            self.input.apply_paused(event, at_us);
        } else {
            self.input.apply(event, at_us);
        }
        self.store_applied();
    }
    /// Bring the world's sensor state into input.
    pub(super) fn load_sensors(&mut self) {
        let state = self
            .world
            .try_resource::<crate::SensorState>()
            .map(|s| s.clone())
            .unwrap_or_default();
        self.input.load_sensors(state);
    }
    /// Write input's applied state back to the world, where it is hashed, when
    /// it changed. An unused world never gains the resource.
    pub(super) fn store_applied(&mut self) {
        let applied = self.input.applied();
        let stored = self
            .world
            .try_resource::<crate::InputState>()
            .map(|g| g.clone());
        if stored.as_ref() != Some(&applied) && (stored.is_some() || !applied.is_empty()) {
            self.world.insert_resource(applied);
        }
        let sensors = self.input.sensor_state();
        let stored = self.world.try_resource::<crate::SensorState>();
        let changed = match &stored {
            Some(s) => **s != *sensors,
            None => !sensors.is_empty(),
        };
        drop(stored);
        if changed {
            let sensors = sensors.clone();
            self.world.insert_resource(sensors);
        }
    }
    /// Mature holds and let windows lapse up to a tick's end (in µs × HZ).
    pub(super) fn mature_input(&mut self, end: u128) {
        self.input
            .mature(|due| (due.max(0) as u128) * (G::HZ as u128) < end);
        self.store_applied();
    }
    /// Bring the world's input contexts and bindings into input at a tick's start.
    pub(super) fn sync_input(&mut self) {
        let contexts = self
            .world
            .try_resource::<crate::InputContexts>()
            .map(|c| c.active.clone())
            .unwrap_or_default();
        let bindings = self
            .world
            .try_resource::<crate::Bindings>()
            .map(|b| b.actions.clone())
            .unwrap_or_default();
        for line in self.input.sync(&contexts, &bindings) {
            self.world.log(line);
        }
        self.load_sensors();
        self.store_applied();
    }
}
impl<G: Game> Sim<G> {
    /// A pad's reading before queue position `end`: the last queued state for
    /// it (none after a blur), else the state input holds.
    fn pad_before(&self, end: usize, index: u32) -> Option<(bool, [f32; 17], [f32; 4])> {
        self.queue
            .range(..end)
            .rev()
            .find_map(|old| match &old.event {
                InputEvent::Pad { index: i, .. } if *i == index => Some(pad_reading(&old.event)),
                InputEvent::Blur { .. } => Some(None),
                _ => None,
            })
            .unwrap_or_else(|| {
                self.input
                    .pad(index)
                    .map(|p| (p.standard, p.buttons, p.axes))
            })
    }
}
/// What a pad event reports, or None for a disconnect.
fn pad_reading(event: &InputEvent) -> Option<(bool, [f32; 17], [f32; 4])> {
    match event {
        InputEvent::Pad {
            connected: true,
            standard,
            buttons,
            axes,
            ..
        } => Some(if *standard {
            (true, *buttons, *axes)
        } else {
            (false, [0.; 17], [0.; 4])
        }),
        _ => None,
    }
}
