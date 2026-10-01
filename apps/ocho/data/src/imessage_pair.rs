//! Self-service iMessage pairing (imessage_pair.rs; workspace.rs
//! `pair_imessage_open`/`pair_imessage_generate`; ui.rs
//! `render_pair_imessage`; #223). View ▸ "Pair iMessage…" (and the palette's
//! "Pair iMessage…", command `PairIMessage`) opens a one-field form; its
//! number, normalised, asks Fleet for a pairing code, shown with a QR the
//! iPhone camera opens Messages with.
//!
//! The form is the forms module's (`FormKind::PairIMessage`): title
//! [`FORM_TITLE`], one field [`FIELD_LABEL`] (empty), the description
//! [`FORM_HINT`], the submit button [`SUBMIT_LABEL`]. On submit,
//! [`normalize_phone`] either gives the number for [`ImessagePair::generate`]
//! or the form stays with [`PHONE_ERROR`].
//!
//! The `fleet` commands, in order (serve.rs `info`, then `load`):
//! 1. `serve --describe --no-qr` → JSON with `machine` (skipped when the
//!    model already knows this Mac's machine id from a running server);
//! 2. `machine pair-imessage MACHINE --phone PHONE` → JSON `{code,
//!    expires_in_minutes, recipient, url, qr: ["0101…", …]}`.
//!
//! `view` is the `IMessagePair` shape:
//!
//! ```text
//! shape IMessagePair
//!   visible: bool
//!   title: string       // "Pair iMessage"
//!   state: string       // loading | error | ready
//!   loading: string     // "Creating your pairing code…"
//!   error: string       // the failure (`warn`, 12 px)
//!   instructions: string // "Scan with your iPhone camera, then send the code in Messages."
//!   qr: list<string>    // rows of "0"/"1", quiet zone included; "1" is a dark module
//!   code: string        // Menlo
//!   sendTo: string      // "Send to {recipient} · Expires in {n} minutes"
//!   sendFrom: string    // "Send from {phone}. Wait for “Paired”, then send !! to choose a Codex session. Desktop stays connected."
//!   url: string         // "Open Messages" opens it
//!   retry: bool         // "New code" shows (a result, good or bad, has arrived)
//! ```
//!
//! Drawing (ui.rs `render_pair_imessage`, 1:1; TEXT_SM 12 px): a dialog
//! card (ui.rs `card`, 460 px wide; bg `elevated`, `rounded_lg`, `border_1`
//! `border`, `shadow_lg`) centred on the 45 % black backdrop. Title `px_4
//! pt_3 pb_2`, semibold. Loading: `px_4 py_3` `muted`. Error: `px_4 py_3`
//! 12 px `warn`. Ready: the instructions `px_4` 12 px; the QR centred in a
//! `py_3` row: a white `p_3 rounded_md` box of 4 × 4 px modules, dark ones
//! black, whatever the theme; the code `px_4` Menlo; sendTo `px_4 py_2` 12 px
//! `muted`; sendFrom `px_4 pb_3` 12 px `muted`; a row `gap_2 px_4 pb_2` of
//! "Copy code" (id "imessage-copy", copies `code`) and "Open Messages"
//! (primary, id "imessage-open"). Footer row `justify_end gap_2 px_4 py_2`:
//! "New code" (id "imessage-retry", when `retry`) and "Close" (hint "Esc",
//! id "imessage-close"). Buttons are ui.rs `action_button`s.

use crate::picker::{canon, Mods};
use serde::Deserialize;
use serde_json::{json, Value as Json};

/// The form's title.
pub const FORM_TITLE: &str = "Pair iMessage";
/// The form's one field.
pub const FIELD_LABEL: &str = "Your iMessage number";
/// The form's description.
pub const FORM_HINT: &str = "Use the number you send iMessages from, including country code (for example +14155552671). We'll show you a QR code to connect it to your Fleet.";
/// The form's submit button.
pub const SUBMIT_LABEL: &str = "Get pairing code";
/// The form's error for a number without a country code.
pub const PHONE_ERROR: &str =
    "Enter your phone number with country code, for example +14155552671.";
/// The dialog's width, px.
pub const WIDTH: f64 = 460.0;
/// A pairing reply this CLI cannot read.
pub const UNREADABLE: &str =
    "Could not read iMessage pairing details. Update the Ocho CLI and try again.";

/// A pairing code, as `fleet machine pair-imessage` prints it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Pairing {
    /// The code to send.
    pub code: String,
    /// Its lifetime, minutes.
    pub expires_in_minutes: u32,
    /// Where to send it.
    pub recipient: String,
    /// The Messages link (`sms:` / `imessage:`) with the code filled in.
    pub url: String,
    /// QR rows of `0`/`1`.
    pub qr: Vec<String>,
}

#[derive(Deserialize)]
struct Describe {
    machine: String,
}

/// A number with its country code, pasted formatting removed: `+` and
/// 7-15 ASCII digits, not starting with 0.
pub fn normalize_phone(input: &str) -> Option<String> {
    let phone: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '-'))
        .collect();
    let digits = phone.strip_prefix('+')?;
    (digits.len() >= 7
        && digits.len() <= 15
        && !digits.starts_with('0')
        && digits.bytes().all(|c| c.is_ascii_digit()))
    .then_some(phone)
}

fn pair_argv(machine: &str, phone: &str) -> Vec<String> {
    vec![
        "machine".into(),
        "pair-imessage".into(),
        machine.into(),
        "--phone".into(),
        phone.into(),
    ]
}

/// The pairing dialog (`Overlay::PairIMessage`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImessagePair {
    /// The dialog is up.
    pub open: bool,
    /// The normalised number.
    pub phone: String,
    /// The latest generation; replies to older ones are dropped.
    pub request: u64,
    /// The result, once it arrives.
    pub info: Option<Result<Pairing, String>>,
}

impl ImessagePair {
    /// Show the dialog for `phone` and ask for a code: the first command to
    /// run, tagged with the request. `machine` is this Mac's machine id when
    /// the model already has it (a running `fleet serve`); otherwise
    /// `serve --describe --no-qr` comes first. Also "New code".
    pub fn generate(&mut self, phone: &str, machine: Option<&str>) -> (u64, Vec<String>) {
        self.open = true;
        self.phone = phone.to_string();
        self.request += 1;
        self.info = None;
        let argv = match machine {
            Some(machine) => pair_argv(machine, phone),
            None => vec!["serve".into(), "--describe".into(), "--no-qr".into()],
        };
        (self.request, argv)
    }

    /// The reply to `serve --describe`: the pairing command to run next
    /// (same request), or `None` (stale, or the failure is shown).
    pub fn set_describe(
        &mut self,
        request: u64,
        reply: Result<String, String>,
    ) -> Option<Vec<String>> {
        if !self.open || request != self.request {
            return None;
        }
        let machine = reply.and_then(|output| {
            serde_json::from_str::<Describe>(&output)
                .map(|d| d.machine)
                .map_err(|e| format!("invalid pairing details: {e}"))
        });
        match machine {
            Ok(machine) => Some(pair_argv(&machine, &self.phone)),
            Err(error) => {
                self.info = Some(Err(error));
                None
            }
        }
    }

    /// The reply to `machine pair-imessage` (dropped when stale or closed).
    pub fn set_pairing(&mut self, request: u64, reply: Result<String, String>) {
        if !self.open || request != self.request {
            return;
        }
        self.info = Some(reply.and_then(|output| {
            serde_json::from_str::<Pairing>(&output).map_err(|_| UNREADABLE.to_string())
        }));
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// A key while the dialog is up: Esc, Enter or q close it; every key is
    /// the dialog's. Returns whether it closed.
    pub fn key(&mut self, name: &str, mods: &Mods) -> bool {
        let name = canon(name);
        let typed = crate::picker::typed(&name, mods);
        if matches!(name.as_str(), "escape" | "enter") || typed.as_deref() == Some("q") {
            self.close();
            return true;
        }
        false
    }

    /// The `IMessagePair` shape.
    pub fn view(&self) -> Json {
        let (state, error, ready) = match &self.info {
            None => ("loading", String::new(), None),
            Some(Err(error)) => ("error", error.clone(), None),
            Some(Ok(info)) => ("ready", String::new(), Some(info)),
        };
        json!({
            "visible": self.open,
            "title": "Pair iMessage",
            "state": state,
            "loading": "Creating your pairing code…",
            "error": error,
            "instructions": "Scan with your iPhone camera, then send the code in Messages.",
            "qr": ready.map(|i| i.qr.clone()).unwrap_or_default(),
            "code": ready.map(|i| i.code.clone()).unwrap_or_default(),
            "sendTo": ready
                .map(|i| format!("Send to {} · Expires in {} minutes", i.recipient, i.expires_in_minutes))
                .unwrap_or_default(),
            "sendFrom": format!(
                "Send from {}. Wait for “Paired”, then send !! to choose a Codex session. Desktop stays connected.",
                self.phone
            ),
            "url": ready.map(|i| i.url.clone()).unwrap_or_default(),
            "retry": self.info.is_some(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_input_accepts_pasted_formatting_but_requires_a_country_code() {
        assert_eq!(
            normalize_phone("+1 (415) 555-2671"),
            Some("+14155552671".into())
        );
        for invalid in [
            "4155552671",
            "+0123456789",
            "+123",
            "+1234567890123456",
            "+1;open",
            "+１２３４５６７",
        ] {
            assert_eq!(normalize_phone(invalid), None);
        }
    }

    #[test]
    fn generating_describes_this_mac_then_pairs_and_drops_stale_replies() {
        let mut p = ImessagePair::default();
        let (first, argv) = p.generate("+14155552671", None);
        assert_eq!(argv, ["serve", "--describe", "--no-qr"]);
        assert_eq!(p.view()["state"], "loading");
        let next = p
            .set_describe(first, Ok(r#"{"machine":"mac-1","relay":""}"#.into()))
            .unwrap();
        assert_eq!(
            next,
            [
                "machine",
                "pair-imessage",
                "mac-1",
                "--phone",
                "+14155552671"
            ]
        );
        // "New code" supersedes the first request.
        let (second, argv) = p.generate("+14155552671", Some("mac-1"));
        assert_eq!(argv[0], "machine");
        p.set_pairing(first, Err("stale".into()));
        assert!(p.info.is_none());
        p.set_pairing(
            second,
            Ok(r#"{"code":"ABC123","expires_in_minutes":10,"recipient":"pair@example.com","url":"sms:x","qr":["101","010"]}"#.into()),
        );
        let v = p.view();
        assert_eq!(v["state"], "ready");
        assert_eq!(v["code"], "ABC123");
        assert_eq!(
            v["sendTo"],
            "Send to pair@example.com · Expires in 10 minutes"
        );
        assert_eq!(v["qr"][1], "010");
        assert_eq!(v["retry"], true);
        p.set_pairing(second, Ok("{}".into()));
        assert_eq!(p.view()["error"], UNREADABLE);
        let (third, _) = p.generate("+14155552671", None);
        assert!(p.set_describe(third, Err("no fleet".into())).is_none());
        assert_eq!(p.view()["error"], "no fleet");
        assert!(p.key("q", &Mods::NONE));
        assert!(!p.open);
    }
}
