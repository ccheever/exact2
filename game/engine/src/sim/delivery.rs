//! A hostless simulation fed its assets as a host feeds them: what setup
//! awaits before setup, then whatever a host would ask for (`take_assets`:
//! streamed, prefetched, first sight, a model's textures) at each observation
//! (the end of `run`, each `settle` round), landing at once or some ticks later.
use super::*;
use std::path::PathBuf;

type Read = Box<dyn FnMut(&str) -> Result<Vec<u8>, String> + Send>;

/// Where a hostless [`Sim`]'s asset bytes come from and when they land:
/// [`Delivery::baked`] (the game's bake, `Sim::baked`) or any loader by name.
pub struct Delivery {
    read: Read,
    after_ticks: u32,
    // Asked-for names and the tick each lands at, in request order.
    flying: Vec<(u64, String)>,
}
impl Delivery {
    /// Bytes by name from `read`, landing at once (see [`Delivery::after_ticks`]).
    pub fn new<E: std::fmt::Display>(
        mut read: impl FnMut(&str) -> Result<Vec<u8>, E> + Send + 'static,
    ) -> Self {
        Self {
            read: Box::new(move |name| read(name).map_err(|e| e.to_string())),
            after_ticks: 0,
            flying: Vec::new(),
        }
    }
    /// The game's baked `assets/`, the files its hosts serve: `bun
    /// game/app/shells.mjs <game> --test` bakes `art/` there before the tests.
    /// The game's directory is the nearest above the test's crate (Cargo's
    /// `CARGO_MANIFEST_DIR`, else the working directory) holding
    /// `logic/src/lib.rs`. A declared level is read where the author wrote
    /// it, not from the bake's copy, which an edit makes stale.
    pub fn baked() -> Self {
        let game = game_dir();
        let assets = game.join("assets");
        Self::new(move |name: &str| {
            // A level is read where the author wrote it: its assets/ copy
            // is the last bake's, older than the source after an edit.
            let at = assets.join(name);
            let level = game.join(name);
            let path = if name.ends_with(".level.json") && level.exists() {
                level
            } else {
                at
            };
            std::fs::read(&path).map_err(|e| {
                format!(
                    "{}: {e}; `bun game/app/shells.mjs {} --test` bakes art/ into assets/ first",
                    path.display(),
                    game.display()
                )
            })
        })
    }
    /// Land each name this many ticks after a host would ask for it, as a
    /// slow network would (what setup awaits still lands before setup). The
    /// default, 0, lands it at the observation that asked.
    pub fn after_ticks(mut self, ticks: u32) -> Self {
        self.after_ticks = ticks;
        self
    }
}
fn game_dir() -> PathBuf {
    // Cargo's test runners set it; a test binary run by hand falls back to
    // its working directory (the crate's, as `cargo test` would run it).
    let from = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    from.ancestors()
        .find(|dir| dir.join("logic/src/lib.rs").is_file())
        .unwrap_or_else(|| {
            panic!(
                "Delivery::baked: no game (logic/src/lib.rs) at or above {}; deliver with a loader instead",
                from.display()
            )
        })
        .to_path_buf()
}

impl<G: Game> Sim<G> {
    /// Construct, deliver what setup awaits, and keep delivering as a host
    /// does: `run` and `settle` hand over every name the world asks for, read
    /// by `read` and landing at once. A refused or missing asset is an error
    /// here and a panic in a later `run`, naming it.
    pub fn with_assets<E: std::fmt::Display>(
        args: G::Args,
        read: impl FnMut(&str) -> Result<Vec<u8>, E> + Send + 'static,
    ) -> Result<Self, String> {
        Self::delivered(args, Delivery::new(read))
    }
    /// [`Sim::with_assets`] from any [`Delivery`]: the game's bake, or a
    /// loader whose names land [`Delivery::after_ticks`] later.
    pub fn delivered(args: G::Args, delivery: Delivery) -> Result<Self, String> {
        let mut sim = Self::new(args)?;
        sim.delivery = Some(delivery);
        sim.deliver()?;
        if sim.is_loading() {
            return Err(format!(
                "assets not ready: {}",
                sim.world.assets.state_json()
            ));
        }
        Ok(sim)
    }
    /// [`Sim::with_assets`] from the game's bake ([`Delivery::baked`]): a
    /// test sees the asset states a host does, its real bytes included.
    /// Panics, naming the asset, when one is missing or refused.
    #[track_caller]
    pub fn baked(args: G::Args) -> Self {
        Self::delivered(args, Delivery::baked()).unwrap_or_else(|e| panic!("{e}"))
    }
    /// Advance the seekable clock to `to_ms`, stopping at each tick a name in
    /// flight lands at, and observe (deliver) at the end.
    pub(super) fn advance_delivering(&mut self, to_ms: f64) -> u32 {
        let mut ran = 0;
        while let Some(due) = self
            .delivery
            .as_ref()
            .and_then(|d| d.flying.iter().map(|f| f.0).min())
        {
            // Tick `due` completes when world time reaches due/HZ seconds.
            let world_us = (due as u128 * 1_000_000).div_ceil(G::HZ as u128) as i64;
            let at_us = self.last_us.unwrap_or(0) + (world_us - self.world_us).max(0);
            let at = at_us as f64 / 1000.0;
            if at >= to_ms {
                break;
            }
            let tick = self.world.tick();
            ran += self.advance(at, Clock::Seekable);
            self.deliver_or_panic();
            if self.world.tick() == tick {
                break; // paused or still loading: time does not reach it
            }
        }
        ran += self.advance(to_ms, Clock::Seekable);
        self.deliver_or_panic();
        ran
    }
    #[track_caller]
    pub(super) fn deliver_or_panic(&mut self) {
        if let Err(e) = self.deliver() {
            panic!("tick {}: {e}", self.world.tick());
        }
    }
    /// One observation: ask for what a host would, and land what is due.
    fn deliver(&mut self) -> Result<(), String> {
        let Some(mut delivery) = self.delivery.take() else {
            return Ok(());
        };
        let tick = self.world.tick();
        let mut result = Ok(());
        while result.is_ok() {
            for name in self.take_assets() {
                // Setup waits for what it asked for; a host fetches it first.
                let wait = if self.setup_pending {
                    0
                } else {
                    delivery.after_ticks
                };
                delivery.flying.push((tick + u64::from(wait), name));
            }
            let (due, later) = std::mem::take(&mut delivery.flying)
                .into_iter()
                .partition::<Vec<_>, _>(|(at, _)| *at <= tick);
            delivery.flying = later;
            if due.is_empty() {
                break;
            }
            for (_, name) in due {
                result = match (delivery.read)(&name) {
                    Ok(bytes) => self.asset(&name, Some(&bytes)),
                    Err(e) => {
                        self.asset_failed(&name, &e);
                        Err(format!("asset `{name}`: {e}"))
                    }
                };
                if result.is_err() {
                    break;
                }
            }
        }
        self.delivery = Some(delivery);
        result
    }
}
