//! The commands a commit issues (LLP 1005 §3), run after it: what this
//! host has, and the rest named as unsupported. What asks the platform for
//! something (a haptic, the clipboard, a link out, the share sheet) is asked
//! of the host's shell where it has one (`requests.rs`, LLP 1115.001 D5).

use super::Presenter;
use crate::host::HostError;
use exact_runner::DataSource;

impl<D: DataSource> Presenter<D> {
    /// Run the last commits' commands (LLP 1005 §3): delivery belongs to the
    /// store (LLP 1030 D7); `setScheme` chooses this painter's `light-dark()`
    /// appearance (LLP 1034 D2); anything else is named.
    pub fn run_commands(&mut self, mut data: impl FnMut() -> D) {
        // Every loop runs this each turn, and a reader's shell is served
        // before its first: whether a shell keeps the screen on is known.
        self.wakelock_unavailable();
        self.run_queued_commands();
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

    /// [`Presenter::run_commands`] but for activating a staged update, which
    /// waits for the next full run: it needs a data source.
    pub(super) fn run_queued_commands(&mut self) {
        for c in std::mem::take(&mut self.commands) {
            match c.name.as_str() {
                "deliveryCheck" => {
                    if !self.check_update() {
                        eprintln!("exact update: no store, or a check is already running");
                    }
                }
                "deliveryActivate" => self.pending_update = true,
                "relaunch" => self.host.log(if self.agent {
                    "relaunch refused: the agent owns this process; start a new drive on the same storage"
                } else {
                    "relaunch refused: process relaunch is unsupported on Linux"
                }),
                // The app's chosen appearance is what a `light-dark()` colour
                // resolves to here (LLP 1034 D2); `system` is no override.
                "setScheme" => self.app_scheme(match c.args.first() {
                    Some(v) if v.as_str() == Some("dark") => Some(true),
                    Some(v) if v.as_str() == Some("light") => Some(false),
                    _ => None,
                }),
                // The shell's haptic engine (LLP 1077 D14); desktop Linux has
                // none, and there is nothing to feel there.
                "haptic" => {
                    let kind = c.args.first().and_then(exact_plan::Value::as_str);
                    self.haptic(kind.unwrap_or_default());
                }
                // The runner's own (LLP 1096 D9): its table is the record; no output here.
                "playSound" | "playSounds" | "stopSounds" | "sendMIDI" => {}
                // No MIDI here (LLP 1125 D6); under the agent the runner answered.
                "requestMIDIAccess" => self.midi_unavailable(),
                // The runner's own too: it laid out at the app's root font size (LLP 1069.000 D3).
                "setRootFontSize" => {}
                // Outside a bubbling event (a key's, focus's, the pointer's
                // or a wheel's, which take theirs), nothing to prevent or stop.
                "preventDefault" | "stopPropagation" => {}
                // The shell's share sheet; with no shell, refused into the
                // journal. Held for the agent like every host (LLP 1069.003 D6).
                "share" => self.share(&c.args, c.source),
                // No request carries a notification yet: refused, or listed
                // for the agent.
                "showNotification" | "closeNotification" => self.notify(&c.name, &c.args),
                "blur" => self.blur_command(&c.args),
                "focus" => {
                    self.focus_command(&c.args);
                }
                // The whole text, which the next key replaces (focus_command).
                "selectText" => self.select_text(&c.args),
                // A text field's selection, the focus unmoved (x2apps codeedit #2).
                "setSelectionRange" => self.set_selection_range(&c.args),
                "setRangeText" => self.set_range_text(&c.args),
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
                // The top layer, where a shell presents one (LLP 1115 D6;
                // LLP 1115.001 D10): a confirmation by its `id`, and its
                // close. With no windows, the journal names the refusal.
                name @ ("showPopover" | "hidePopover" | "togglePopover") => {
                    self.popover_command(name, &c.args)
                }
                "showModal" if self.has_windows() || self.neutral_alerts() => self.show_modal(&c.args, c.source),
                "close" if (self.has_windows() || self.neutral_alerts()) && !c.args.is_empty() => self.close_modal(&c.args),
                "showModal" => self
                    .host
                    .log("showModal refused: dialogs unsupported on the headless/DRM host"),
                "close" if !c.args.is_empty() => self
                    .host
                    .log("close refused: dialogs unsupported on the headless/DRM host"),
                // The shell's clipboard, and its browser, mail app and dialer.
                "copyText" if self.has_shell() => self.copy_text(&c.args),
                "openURL" if self.has_shell() => self.open_url(&c.args),
                // With no shell there is no clipboard or browser; and no
                // editor, dev menu or window to close on any Linux host, and
                // no media player (LLP 1042 §8): known, and named so.
                name @ ("copyText" | "openURL" | "format" | "reload" | "close" | "fastSeek"
                | "load") => {
                    eprintln!("exact: {name} unsupported on the headless/DRM host")
                }
                other => eprintln!("exact: unknown command {other}"),
            }
        }
        // Selection events are queued tasks: the input's commands run first.
        self.drain_field_select_tasks();
        self.dispatch_toggles();
    }
}
