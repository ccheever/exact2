//! The commands a commit issues (LLP 1005 §3), run after it: what this
//! host has, and the rest named as unsupported.

use super::Presenter;
use crate::host::HostError;
use exact_runner::DataSource;

impl<D: DataSource> Presenter<D> {
    /// Run the last commits' commands (LLP 1005 §3): delivery belongs to the
    /// store (LLP 1030 D7); `setScheme` chooses this painter's `light-dark()`
    /// appearance (LLP 1034 D2); anything else is named.
    pub fn run_commands(&mut self, mut data: impl FnMut() -> D) {
        for c in std::mem::take(&mut self.commands) {
            match c.name.as_str() {
                "deliveryCheck" => {
                    if !self.check_update() {
                        eprintln!("exact update: no store, or a check is already running");
                    }
                }
                "deliveryActivate" => self.pending_update = true,
                // The app's chosen appearance is what a `light-dark()` colour
                // resolves to here (LLP 1034 D2); `system` is no override.
                "setScheme" => self.app_scheme(match c.args.first() {
                    Some(v) if v.as_str() == Some("dark") => Some(true),
                    Some(v) if v.as_str() == Some("light") => Some(false),
                    _ => None,
                }),
                // No haptic engine here (LLP 1077 D14): nothing to feel.
                "haptic" => {}
                // The runner's own (LLP 1096 D9): its table is the record; no output here.
                "playSound" | "playSounds" | "stopSounds" => {}
                // The runner's own too: it laid out at the app's root font size (LLP 1069.000 D3).
                "setRootFontSize" => {}
                // Outside a `key` event (`key_event` takes a key's), nothing to prevent or stop.
                "preventDefault" | "stopPropagation" => {}
                // No share sheet here: refused into the journal, or held for
                // the agent like every host (LLP 1069.003 D6).
                "share" => {
                    let share = exact_runner::share::Share::from_args(&c.args);
                    let runner = self.host.runner_mut();
                    exact_runner::share::arm(runner, share, c.source, self.agent, false);
                }
                // No notification centre here: refused, or listed for the agent.
                "showNotification" | "closeNotification" => self.notify(&c.name, &c.args),
                "blur" => self.blur_command(&c.args),
                "focus" => {
                    self.focus_command(&c.args);
                }
                // The whole text, which the next key replaces (focus_command).
                "selectText" => self.select_text(&c.args),
                // A text field's selection, the focus unmoved (x2apps codeedit #2).
                "setSelectionRange" => self.set_selection_range(&c.args),
                // An element's, by its id (minesweeper F3); a row's is the runner's.
                "scrollIntoView" => self.scroll_element_into_view(&c.args),
                // `Element.scrollBy(x, y)`, by its id.
                "scrollBy" => self.scroll_element_by(&c.args),
                // The inverse of `message=`: text into the named surface's
                // canvas, stamped now and delivered in order with its input.
                "postMessage" => {
                    let arg = |i: usize| c.args.get(i).and_then(exact_plan::Value::as_str);
                    let (text, name) = (arg(0).unwrap_or_default(), arg(1).unwrap_or_default());
                    let event = serde_json::json!({"t":"message","text":text,"at":self.host.now()});
                    if !self.surfaces.post(name, event) {
                        self.host.log(format!(
                            "postMessage: dropped: {} posts already wait for surface \"{name}\"",
                            crate::surfaces::POST_BOUND
                        ));
                    }
                }
                // @ref LLP 1069.002 D8 — refused with `cancel`; the agent's
                // substitute answers (D9).
                "showPicker" => match c.args.first().and_then(exact_plan::Value::as_str) {
                    Some(id) => self.show_picker(id),
                    _ => eprintln!("exact: showPicker requires an element id"),
                },
                // @ref LLP 1069.010 D3 — no save panel here: refused with
                // `cancel`, or held for the agent like every host.
                "saveFile" => self.save_file(&c.args),
                // @ref LLP 1069.010 D2 — no picker here either: refused with
                // `cancel`, or held for the agent.
                name @ ("showOpenFilePicker" | "showDirectoryPicker" | "showSaveFilePicker") => {
                    self.document_picker(name, &c.args)
                }
                // No clipboard, browser, editor, dev menu or window to close
                // here, and no media player (LLP 1042 §8): known, and named so.
                name @ ("copyText" | "openURL" | "format" | "reload" | "close" | "fastSeek"
                | "load") => {
                    eprintln!("exact: {name} unsupported on the headless/DRM host")
                }
                other => eprintln!("exact: unknown command {other}"),
            }
        }
        if self.pending_update {
            self.pending_update = false;
            match self.activate_update(data()) {
                Ok(true) => {}
                Ok(false) => eprintln!("exact update: nothing is staged"),
                Err(HostError::PreparingModule) => self.pending_update = true,
                Err(e) => eprintln!("exact update: activate: {e}"),
            }
        }
    }
}
