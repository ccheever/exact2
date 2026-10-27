//! A received share at the bridge (LLP 1133.024 D2–D4): claimed by the
//! running app, or before the boot, whose location and tokens it becomes.
use super::*;
use exact_runner::share_in::{Claims, Pending, ShareTarget};

impl<D: DataSource> Bridge<D> {
    /// `exact_share`: the JSON in the input buffer, `{"root", "target",
    /// "share", "inbox"?}` (`inbox`: the directory a `share.json` share's
    /// files wait in, moved from there). Running, a batch with the location
    /// delivered at `root`; before the boot, the launch location and the
    /// tokens are set for it and an empty batch answers.
    pub fn share(&mut self, len: usize, now_ms: f64) -> u32 {
        let request: serde_json::Value =
            serde_json::from_slice(&self.input[..len.min(self.input.len())]).unwrap_or_default();
        let inbox = request["inbox"].as_str().map(std::path::PathBuf::from);
        let out = match self.host.as_mut() {
            Some(h) => h.receive_share(&request, inbox.as_deref(), now_ms),
            None => {
                let claimed = ShareTarget::from_json(&request["target"])
                    .zip(Pending::from_json(&request["share"], inbox.as_deref()))
                    .map(|(target, share)| {
                        let seed = self.launch_facts.place.as_ref().map_or(0.0, |p| p.seed);
                        let mut claims = Claims::new(seed);
                        let claimed = claims.claim(&target, share);
                        self.launch = Some(claimed.location.clone());
                        self.launch_facts.shared = claimed.tokens.clone();
                        self.launch_facts.claims = Some(claims);
                        (claimed, inbox.is_some())
                    });
                self.cold_share = claimed;
                "{\"ops\":[],\"timers\":false,\"motion\":false}".to_string()
            }
        };
        self.emit(out)
    }

    /// `exact_share_land`: a share claimed before the boot lands once
    /// storage is configured, a batch (empty when there is none).
    pub fn share_land(&mut self) -> u32 {
        let out = match (self.host.as_mut(), self.cold_share.take()) {
            (Some(h), Some((claimed, moving))) => h.land_share(claimed, moving),
            _ => "{\"ops\":[],\"timers\":false,\"motion\":false}".to_string(),
        };
        self.emit(out)
    }
}
