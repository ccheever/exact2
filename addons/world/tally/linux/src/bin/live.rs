//! Mac-runnable Linux presenter with an app-owned, real-time headless loop.
//! The stock headless entry only serves the controlled agent clock.
use exact_linux::app::{boot_presenter, Config};
use std::time::{Duration, Instant};
use tally_data::TallySource;
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
fn main() {
    tally_data::metrics::entry();
    let mut config = Config::from_env(PLAN, COMPAT);
    let (mut p, error) = boot_presenter::<TallySource>(&mut config, (820., 900.)).unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p.first_pixel();
    let start = Instant::now();
    let mut frames = 0;
    loop {
        let now = start.elapsed().as_secs_f64() * 1000.;
        if let Some(due) = p.host().timer_due_ms() {
            if due > now {
                std::thread::sleep(Duration::from_secs_f64((due - now) / 1000.));
            }
        }
        let (_, error) = p.clock(start.elapsed().as_secs_f64() * 1000.);
        assert!(error.is_none(), "{error:?}");
        p.frame();
        frames += 1;
        if frames % 100 == 0 {
            eprintln!("X1FRAMES {frames}");
        }
    }
}
