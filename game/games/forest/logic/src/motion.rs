//! The art pass's motion as a look (`../../motion.look`; LLP 1046.009 §3.2, an
//! experiment): the declaration presents, and `look.rs` stays as the Rust it is
//! compared with. `reload` replaces the declaration in the running game; the
//! next present draws it, with no build and nothing in the world changed.
use exact_game::Present;
use exact_game_look::{Externs, Live, Stats, Stopwatch};
use std::cell::{Cell, RefCell};

/// The look this build ships.
pub const SOURCE: &str = include_str!("../../motion.look");

/// What the look may call of the game's own Rust: the terrain and the grid.
pub fn externs() -> Externs {
    Externs::new()
        .function("height", 2, |a| crate::forest::height(a[0], a[1]))
        .constant("CELL", crate::forest::CELL)
}

thread_local! {
    static LIVE: RefCell<Live> = RefCell::new(Live::new(SOURCE, externs()));
    static RUST: Cell<bool> = const { Cell::new(false) };
    // Present times while a diagnostic measures them (`spent`).
    static SPENT: RefCell<Option<Vec<f64>>> = const { RefCell::new(None) };
}

pub(crate) fn present(p: &mut Present<'_>) {
    // Timed only while a diagnostic asks (`spent`, examples/look_cost.rs); the
    // time reaches no row and nothing in the world.
    let watch = SPENT.with(|s| s.borrow().is_some()).then(Stopwatch::start);
    if RUST.with(Cell::get) {
        crate::look::present(p);
    } else {
        LIVE.with(|live| live.borrow_mut().present(p));
    }
    if let Some(watch) = watch {
        SPENT.with(|s| s.borrow_mut().as_mut().map(|spent| spent.push(watch.us())));
    }
}

/// Microseconds each present on this thread has taken since the last call
/// (a diagnostic: the first call starts measuring).
pub fn spent() -> Vec<f64> {
    SPENT.with(|s| s.borrow_mut().replace(Vec::new()).unwrap_or_default())
}

/// Replace the declaration (this thread's games): the next present compiles it
/// against the world's types; a refused one leaves the running look in place.
pub fn reload(source: &str) {
    LIVE.with(|live| live.borrow_mut().replace(source));
}
/// Present with the Rust reference (`look.rs`) instead, on this thread.
pub fn use_rust(on: bool) {
    RUST.with(|r| r.set(on));
}
/// Refusals and runtime failures since the look last compiled.
pub fn errors() -> Vec<String> {
    LIVE.with(|live| live.borrow().errors().to_vec())
}
/// The last present's counts, and how long the last compile took (µs).
pub fn stats() -> (Stats, f64) {
    LIVE.with(|live| {
        let live = live.borrow();
        (live.stats(), live.compile_us)
    })
}
/// Microseconds each rule took in the last present, with where it starts.
pub fn rule_us() -> Vec<(String, f64)> {
    LIVE.with(|live| {
        let live = live.borrow();
        let Some(look) = live.look() else {
            return Vec::new();
        };
        look.rules()
            .iter()
            .zip(live.rule_us())
            .map(|(r, us)| (format!("{} @{}", r.what, r.at.line), *us))
            .collect()
    })
}
/// The checker's report on the running look.
pub fn report() -> String {
    LIVE.with(|live| live.borrow().look().map(|l| l.report()).unwrap_or_default())
}
