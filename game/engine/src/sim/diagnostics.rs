use super::*;
impl<G: Game> Sim<G> {
    /// Set once by the bake's host from its embedded trust; never saved or read by gameplay.
    pub fn development_edits(&mut self, allowed: bool) {
        self.development_edits = allowed;
    }

    pub(crate) fn queued_sets(&self) -> usize {
        self.queue.iter().filter(|q| matches!(&q.event, InputEvent::Message { text, .. } if crate::world::edit::command(text).is_some())).count()
    }

    /// Assert a test hash against the game's single pins.json (included by its test).
    /// Unknown metadata fields are skipped by Data; no generated Rust is needed.
    pub fn assert_pin(&self, pins: &str) {
        assert!(
            !self.edits_admitted && !self.world.set_seen.get(),
            "pin refused: run contains a set development edit"
        );
        assert_eq!(
            self.queued_sets(),
            0,
            "pin refused: queued set development edit awaits admission"
        );
        let (game, tick, got) = (G::ID, self.world.tick(), self.world.hash());
        #[derive(Default, crate::Data)]
        struct Pins {
            game: String,
            ticks: std::collections::BTreeMap<String, String>,
        }
        let pins: Pins = crate::json::from_str(pins).unwrap_or_else(|error| {
            panic!("pins.json game <invalid>, expected `{game}`: {error}; run bun game/prove.mjs {game}")
        });
        assert!(
            pins.game == game,
            "pins.json game `{}`, expected `{game}`; run bun game/prove.mjs {game}",
            if pins.game.is_empty() {
                "<missing>"
            } else {
                &pins.game
            }
        );
        let expected = pins
            .ticks
            .get(&tick.to_string())
            .unwrap_or_else(|| panic!("pin {tick} missing; run bun game/prove.mjs {game}"));
        let got = format!("0x{got:016x}");
        assert!(expected == &got,
            "pin {tick} differs (expected {expected}, got {got}); if the change is intended: bun game/prove.mjs {game} --repin");
    }

    /// Join completed tick spans to the frame the renderer just submitted.
    pub fn trace_frame(&mut self) -> u64 {
        self.world.spans.borrow_mut().frame()
    }
    /// Chrome trace-event JSON in the host CPU clock domain.
    pub fn trace_events_json(&self) -> String {
        self.world.spans.borrow().trace_json()
    }
    /// Arm host-only tick measurements. The game receives no measurement reads.
    pub fn measure_spans(&mut self, clock: fn() -> f64, reset: bool) {
        self.world.spans.borrow_mut().arm(clock, reset);
    }
    /// Exclusive span distributions in milliseconds over the retained tick window.
    pub fn spans_json(&self, since: Option<u64>) -> String {
        self.world.spans.borrow().json(since)
    }
}
