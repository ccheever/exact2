//! Hook diagnostics stay outside saved state and allocate only for hooked canvases.
use super::{HookWork, Needs};
use crate::perf::Ring;
pub(crate) const NAMES: [&str; 6] = [
    "prepare",
    "compute",
    "opaque",
    "background",
    "surface",
    "post",
];
#[derive(Default)]
pub(crate) struct Metrics {
    pub engine: HookWork,
    before: Option<(HookWork, Option<HookWork>)>,
    cpu: [Ring; 6],
    pub attachment_bytes: u64,
}
impl Metrics {
    pub fn arm(&mut self, reset: bool) {
        for ring in &mut self.cpu {
            ring.arm(reset);
        }
    }
    pub fn record(&mut self, times: [Option<f64>; 6], work: Option<HookWork>, ready: bool) {
        for (ring, time) in self.cpu.iter_mut().zip(times) {
            if let Some(ms) = time {
                ring.push(ms);
            }
        }
        if ready {
            self.before.get_or_insert((self.engine, work));
        }
    }
    pub fn append(&self, out: &mut String, work: Option<HookWork>, needs: Needs) {
        let stages = stages(needs);
        out.push_str(&format!(",\"renderHooks\":{{\"stages\":{stages},\"attachmentBytes\":{},\"gpuTiming\":\"optional query set; opaque/background inside forward, surface inside continuation\",\"engineWork\":{{\"beforeReady\":{},\"afterReady\":{}}},\"reportedWork\":",
            self.attachment_bytes,self.before.map_or(self.engine,|b|b.0).json(),self.engine.since(self.before.map_or(self.engine,|b|b.0)).json()));
        if let Some(work) = work {
            let before = self.before.and_then(|b| b.1).unwrap_or(work);
            out.push_str(&format!(
                "{{\"beforeReady\":{},\"afterReady\":{}}}",
                before.json(),
                work.since(before).json()
            ));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"cpuMs\":{");
        for (i, (name, ring)) in NAMES.iter().zip(&self.cpu).enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("\"{name}\":"));
            ring.json(out);
        }
        out.push_str("}}");
    }
}
pub(crate) fn stages(needs: Needs) -> String {
    let mut names = vec!["compute", "opaque", "background"];
    if needs.contains(Needs::SCENE_COPY) {
        names.push("surface");
    }
    if needs.contains(Needs::HDR_POST) {
        names.push("post");
    }
    exact_game::json::to_string(&names.into_iter().map(str::to_owned).collect::<Vec<_>>()).unwrap()
}
