//! `showPicker` and its answer on Linux (LLP 1069.002 D8, D9): refused with
//! `cancel` outside the agent (ruled: no desktop host yet); under the agent,
//! held for `tap @t cancel` or `type @t <path>…`, whose files are copied
//! into `app:/tmp/picked/` before `change` fires.
use super::*;
use exact_runner::picker_support as support;
use exact_runner::ControlValue;

impl<D: DataSource> Presenter<D> {
    /// `showPicker(id)`, from an action or a press on a visible file input.
    pub(crate) fn show_picker(&mut self, id: &str) {
        if crate::picker::agent() {
            let request = format!(
                "{{\"op\":\"showPicker\",\"id\":{}}}",
                serde_json::Value::from(id)
            );
            let _ = self.host.answer_hold(&request);
            self.dirty = true;
            return;
        }
        self.host.log("picker: refused: unavailable");
        if let Some(view) = self.host.runner().picker(id).map(|p| p.view) {
            self.deliver_picker(view, Event::Cancel);
        }
    }

    /// The agent's answer to a held picker, after the runner consumed it:
    /// `request` is the driver's line (its value never journalled), `reply`
    /// the runner's.
    pub(crate) fn answer_picker(&mut self, request: &str, reply: &str) {
        let r: serde_json::Value = serde_json::from_str(reply).unwrap_or_default();
        if r["capability"] != "pick" {
            return;
        }
        let Some(view) = r["node"].as_u64().map(|n| n as ViewId) else {
            return;
        };
        if r["answered"] == "cancel" {
            self.deliver_picker(view, Event::Cancel);
            return;
        }
        let q: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
        let value = q["text"].as_str().unwrap_or("");
        let accept: Vec<String> = self
            .host
            .kernel()
            .node(view)
            .and_then(|n| n.props.str(PropId::Accept).map(str::to_owned))
            .unwrap_or_default()
            .split(',')
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        let mut files = Vec::new();
        for path in support::answer_paths(value) {
            let source = std::path::PathBuf::from(&path);
            let name = source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mime = support::mime_for(&name);
            // D6: a HEIC photo under an `accept` without HEIC is a JPEG,
            // which this host cannot make.
            if support::delivered_type(mime, &accept) != mime {
                self.host.log(format!(
                    "picker: refused: {mime} to JPEG is unavailable on Linux"
                ));
                self.deliver_picker(view, Event::Cancel);
                return;
            }
            let app_path = self.host.runner_mut().picked_path(&name);
            match crate::picker::copy_in(&source, &app_path, mime) {
                Ok(f) => files.push(f),
                Err(e) => {
                    self.host.log(format!("picker: refused: {e}"));
                    self.deliver_picker(view, Event::Cancel);
                    return;
                }
            }
        }
        self.deliver_picker(view, Event::Change(ControlValue::Files(files)));
    }

    fn deliver_picker(&mut self, view: ViewId, event: Event) {
        let now = self.host.now();
        if let Some(e) = self.host.dispatch_at(view, event, now) {
            eprintln!("exact: {e}");
        }
        self.dirty = true;
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
    }
}
