use super::*;
impl<G: Game> Sim<G> {
    /// Set once by the bake's host from its embedded trust; never saved or read by gameplay.
    pub fn development_edits(&mut self, allowed: bool) {
        self.development_edits = allowed;
    }

    /// Assert a test hash against the game's single pins.json (included by its test).
    /// Unknown metadata fields are skipped by Data; no generated Rust is needed.
    pub fn assert_pin(&self, pins: &str) {
        assert!(
            !self.world.set_seen.get(),
            "pin refused: journal contains a set development edit"
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

    /// Arm host-only tick measurements. The game receives no measurement reads.
    pub fn measure_spans(&mut self, clock: fn() -> f64, reset: bool) {
        self.world.spans.borrow_mut().arm(clock, reset);
    }
    /// Exclusive span distributions in milliseconds over the retained tick window.
    pub fn spans_json(&self, since: Option<u64>) -> String {
        self.world.spans.borrow().json(since)
    }
}
