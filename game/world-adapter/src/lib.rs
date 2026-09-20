//! Contract conversions for the external world surface adapter.
#![deny(unsafe_code)]
pub mod args;
pub mod publication;

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_public_report_names_busy_work_and_observation() {
        use exact_world::{json, Work, World};
        let w = World::new(60, 0);
        w.busy("controller moving").unwrap();
        w.work("assets", Work::Pending).unwrap();
        w.work("timer", Work::Deadline(90)).unwrap();
        let mut out = json::Encoder::default();
        w.report(&mut out).unwrap();
        let text = out.finish().unwrap();
        for expected in [
            "controller moving",
            "assets",
            "timer",
            "90",
            "observation",
            "truncated",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        for i in 0..64 {
            if i > 1 {
                w.work(&format!("extra-{i:02}"), Work::Ready).unwrap();
            }
        }
        let mut out = json::Encoder::default();
        w.report(&mut out).unwrap();
        let text = out.finish().unwrap();
        assert!(text.contains("\"truncated\":true"), "{text}");
        assert!(!text.contains("extra-63"));
        assert!(w.work("overflow", Work::Pending).is_err());
    }
}
