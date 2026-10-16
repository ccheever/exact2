//! The build channel's presentation half (LLP 1122 D4, LLP 1121 stage 4
//! commit 5): the frames the URL poller's worker took are applied here, on
//! the presentation thread, and the reports they call for queued for its
//! next tick (`crate::build_channel`). A `pin` sets what the agent's
//! `prefer` sets — the system scheme and `prefers-color-scheme`,
//! `prefers-contrast`, the window's modes, the root font size — each fact
//! it does not name what the first pin replaced; a null pin hands all of
//! them back.
use super::*;
use crate::build_channel::{Frame, Pin, Pinned};
use serde_json::{json, Value};

impl<D: DataSource> Presenter<D> {
    /// Apply the frames the last tick took; report status after a
    /// registration or a pin.
    pub(super) fn serve_build_channel(&mut self) {
        let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) else {
            return;
        };
        let frames = std::mem::take(&mut link.frames);
        let mut status = std::mem::take(&mut link.registered);
        for frame in frames {
            match Frame::parse(&frame) {
                Some(Frame::Pin { revision, pin }) => {
                    self.apply_pin(revision, pin);
                    status = true;
                }
                Some(Frame::Reserved(kind)) => self.report_unsupported(&kind, None),
                None => {}
            }
        }
        if status {
            self.report_status();
        }
    }

    fn apply_pin(&mut self, revision: u64, pin: Option<Pin>) {
        let runner = self.host.runner();
        let now = Pinned {
            dark: self.scheme.1,
            contrast: runner.viewport().preferences.contrast,
            modes: runner.modes(),
            root_font_size: runner.host_root_font_size(),
        };
        let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) else {
            return;
        };
        let before = link.before.get_or_insert(now).clone();
        link.revision = Some(revision);
        link.held = pin.clone().map(|p| (revision, p));
        if pin.is_none() {
            link.before = None;
        }
        let pin = pin.unwrap_or_default();
        let dark = pin.dark.unwrap_or(before.dark);
        let mut preferences = self.host.runner().viewport().preferences;
        preferences.dark = dark;
        preferences.contrast = pin.contrast.unwrap_or(before.contrast);
        if let Some(e) = self.set_preferences(preferences) {
            self.report_unsupported("pin", Some(&e));
        }
        self.set_system_scheme(dark);
        let px = pin.root_font_size.unwrap_or(before.root_font_size);
        if let Some(e) = self.set_root_font_size(px) {
            self.report_unsupported("pin.rootFontSize", Some(&e));
        }
        let mut modes = before.modes;
        for (axis, value) in pin.mode {
            match modes.iter_mut().find(|(a, _)| *a == axis) {
                Some(slot) => slot.1 = value,
                None => modes.push((axis, value)),
            }
        }
        let pins: Vec<(&str, &str)> = modes
            .iter()
            .map(|(a, v)| (a.as_str(), v.as_str()))
            .collect();
        if pins.is_empty() {
            return;
        }
        let refused = match self.host.runner().check_modes(&pins) {
            Err(e) => Some(format!("{e:?}")),
            Ok(()) => self.set_modes(&pins),
        };
        if let Some(e) = refused {
            self.report_unsupported("pin.mode", Some(&e));
        }
    }

    /// A new generation's runner starts from its plan: the pin held is set
    /// on it again.
    pub(super) fn repin(&mut self) {
        let held = self
            .dev
            .as_ref()
            .and_then(|dev| dev.link.as_ref())
            .and_then(|link| link.held.clone());
        if let Some((revision, pin)) = held {
            self.apply_pin(revision, Some(pin));
        }
    }

    /// Queue the effective facts (`status`): the scheme a `light-dark()`
    /// colour resolves to, the contrast, the window's modes, the root font
    /// size, the plan identity and design digest, the pin revision applied.
    pub(super) fn report_status(&mut self) {
        let runner = self.host.runner();
        let design = runner.design();
        let preferences = runner.viewport().preferences;
        let mode: serde_json::Map<String, Value> = runner
            .modes()
            .into_iter()
            .map(|(a, v)| (a, v.into()))
            .collect();
        let root_font_size = runner.root_font_size();
        let dark = self.dark();
        let Some(dev) = self.dev.as_mut() else {
            return;
        };
        let plan = dev.identity.clone();
        let Some(link) = dev.link.as_mut() else {
            return;
        };
        link.plan = plan.clone();
        link.design = Some(design.clone());
        let revision = link.revision;
        link.report(json!({
            "kind": "status",
            "scheme": if dark { "dark" } else { "light" },
            "contrast": preferences.contrast.keyword(),
            "mode": mode,
            "rootFontSize": root_font_size,
            "plan": plan,
            "design": design,
            "revision": revision,
        }));
    }

    /// Report status when the design this build runs is not the one last
    /// reported (a live snapshot or batch applied).
    pub(super) fn report_design(&mut self) {
        let reported = self
            .dev
            .as_ref()
            .and_then(|dev| dev.link.as_ref())
            .map(|link| link.design.clone());
        if let Some(reported) = reported {
            if reported.as_deref() != Some(self.host.runner().design().as_str()) {
                self.report_status();
            }
        }
    }

    /// Queue `unsupported` for `what`; with no channel, a line to stderr.
    pub(super) fn report_unsupported(&mut self, what: &str, detail: Option<&str>) {
        match self.dev.as_mut().and_then(|dev| dev.link.as_mut()) {
            Some(link) => {
                link.report(json!({"kind": "unsupported", "what": what, "detail": detail}))
            }
            None => eprintln!("exact url: {what}: {}", detail.unwrap_or("unsupported")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pin sets what `prefer` sets and reports the effective facts; the
    /// null pin hands back what was there before the first.
    #[test]
    fn a_pin_frame_sets_and_hands_back_the_facts() {
        let dir = std::env::temp_dir().join(format!("exact-linux-pin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.tamber"),
            "tamber t\n\naxes\n  dark-style = black | dim\n\ntheme\n  page = #000000\n    when dark-style=dim: #161e27\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("app.contract"),
            "component App\n  view\n    column background-color=\"page\"\n      text \"a\"\n",
        )
        .unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = contract::bake(plan, ()).unwrap().encode();
        let (mut p, _) = Presenter::boot_with(
            &bytes,
            (),
            (200.0, 200.0),
            1.0,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        p.set_development(Some("http://127.0.0.1:9/".into()), Some("plan-a".into()));
        let frames = |p: &mut Presenter<()>, f: Vec<Value>| {
            let link = p.dev.as_mut().unwrap().link.as_mut().unwrap();
            link.frames = f;
            p.serve_build_channel();
            let link = p.dev.as_mut().unwrap().link.as_mut().unwrap();
            link.exchange().unwrap()
        };
        let sent = |e: crate::build_channel::Exchange| e.reports_for_test();
        let reports = sent(frames(
            &mut p,
            vec![
                json!({"kind": "pin", "revision": 2, "pin": {"scheme": "dark", "contrast": "more",
                    "mode": {"dark-style": "dim"}, "rootFontSize": 20}}),
                json!({"kind": "outline"}),
            ],
        ));
        assert_eq!(
            reports[0],
            json!({"kind": "unsupported", "what": "outline", "detail": null})
        );
        let status = &reports[1];
        assert_eq!(status["kind"], "status");
        assert_eq!(status["scheme"], "dark");
        assert_eq!(status["contrast"], "more");
        assert_eq!(status["mode"], json!({"dark-style": "dim"}));
        assert_eq!(status["rootFontSize"], 20.0);
        assert_eq!(status["plan"], "plan-a");
        assert_eq!(status["design"], json!(p.host.runner().design()));
        assert_eq!(status["revision"], 2);
        assert!(p.dark());
        // A pin naming only the font size hands the rest back.
        let status = &sent(frames(
            &mut p,
            vec![json!({"kind": "pin", "revision": 3, "pin": {"rootFontSize": 18}})],
        ))[0];
        assert_eq!(
            (
                &status["scheme"],
                &status["contrast"],
                &status["mode"]["dark-style"]
            ),
            (&json!("light"), &json!("no-preference"), &json!("black"))
        );
        assert_eq!(status["rootFontSize"], 18.0);
        let status = &sent(frames(
            &mut p,
            vec![json!({"kind": "pin", "revision": 4, "pin": null})],
        ))[0];
        assert_eq!(status["rootFontSize"], 16.0);
        assert_eq!(status["revision"], 4);
        assert!(p
            .dev
            .as_ref()
            .unwrap()
            .link
            .as_ref()
            .unwrap()
            .before
            .is_none());
        // A mode the design lacks is reported, not applied.
        let reports = sent(frames(
            &mut p,
            vec![json!({"kind": "pin", "revision": 5, "pin": {"mode": {"dark-style": "grey"}}})],
        ));
        assert_eq!(reports[0]["what"], "pin.mode");
        assert_eq!(reports[1]["mode"]["dark-style"], "black");
    }
}
