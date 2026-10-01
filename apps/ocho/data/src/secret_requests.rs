//! Session-scoped secret requests (secret_requests.rs `Requests`,
//! workspace/secret_requests.rs, ui/secret_requests_ui.rs; #220, #228,
//! #237, #245, #255). An agent asks for a named secret; the Secrets sidebar
//! lists the active session's requests and takes a value, which goes to
//! `fleet secret provide` over stdin and is never shown, logged or put in
//! the view. Requests on other attached sessions raise an alert card. A
//! terminal link `ocho://secret/NAME` (click-only) asks for one by name.
//!
//! Time is the wall clock in seconds (`epoch_s`): a provided secret's
//! `expires_at` is epoch seconds. The model polls on its tick
//! ([`POLL_MS`]): `wanted_list` for the open sidebar, `wanted_alerts` for
//! every other attached session. The demo (`FLEET_SECRET_REQUESTS_DEMO`
//! upstream) keeps everything local and discards values.
//!
//! The `fleet` commands, each answered by the matching `set_*`:
//! - `secret list M S` → JSON `[{id, name, reason, status, expires_at?}]`
//!   (status `pending | provided | dismissed | revoked | expired`).
//! - `secret provide M S ID --ttl {seconds}s`, the value on stdin.
//! - `secret dismiss M S ID`, `secret revoke M S ID`.
//! - `secret link M S NAME` → one request's JSON (a pending one opens its
//!   entry).
//!
//! `view` is the `SecretRequests` shape:
//!
//! ```text
//! shape SecretLifetime
//!   id: string          // "secret-lifetime-900"
//!   label: string       // "15 min" | "1 hour" | "8 hours"
//!   seconds: number
//!   selected: bool      // bg accent 16 % and `text`; else `element` and `muted`
//! shape SecretRequestCard
//!   id: string          // "secret-card-{id}"
//!   requestId: string   // the request's id, for the presses below
//!   name: string
//!   reason: string
//!   status: string      // "Needs a value" | "Expires in 12m" | "Expired" | "Dismissed" | "Revoked"
//!   statusColor: string // warn | good | muted (a theme field)
//!   editing: bool       // border `accent` (else `border`)
//!   entry: bool         // editing a pending request: the entry shows
//!   actions: bool       // pending and not editing: "Provide value" (primary) + "Dismiss"
//!   revoke: bool        // provided: a "Revoke" button
//!   openId: string      // "secret-open-{id}"
//!   dismissId: string   // "secret-dismiss-{id}"
//!   revokeId: string    // "secret-revoke-{id}"
//!   provideId: string   // "secret-provide-{id}" (the entry's primary button)
//!   cancelId: string    // "secret-cancel-{id}"
//! shape SecretAlert
//!   visible: bool       // a request on a session the sidebar is not showing
//!   title: string       // "Secret requested"
//!   name: string        // control characters removed, one truncated line
//!   reason: string      // control characters removed, max 96 px tall, scrolling
//!   openLabel: string   // "Open Secrets" (primary, id "secret-alert-open")
//! shape SecretRequests
//!   visible: bool       // the sidebar is open (demo, or the active tab has a session)
//!   title: string       // "Secrets"
//!   pending: string     // "{n} pending" in the header, "" when none
//!   demo: string        // the demo banner, "" outside the demo
//!   error: string       // `danger`, 11 px, under the header; "" when none
//!   empty: string       // "Loading secrets for this session…" | "No secrets requested for this session." | ""
//!   cards: list<SecretRequestCard>
//!   masked: string      // the draft as "*" per byte; the value never leaves the model
//!   placeholder: string // "Enter value…" | "Enter demo value…"
//!   focused: bool       // the entry has the keyboard
//!   provideLabel: string // "Provide value" | "Provide demo value"
//!   lifetimes: list<SecretLifetime>
//!   busy: bool          // a provide/dismiss/revoke is in flight
//!   reset: string       // "Reset demo" (footer button "secret-demo-reset"), "" outside the demo
//!   alert: SecretAlert
//! ```
//!
//! Drawing (secret_requests_ui.rs, 1:1; TEXT_SM 12 px, TEXT_XS 11 px):
//! - Sidebar: right of the body, 312 px wide, full height, `bg`,
//!   `border_l_1` `border`. Header `px_3 py_2` (12 × 8), `border_b_1`, a
//!   row `gap_2`: "Secrets" semibold `flex_1`; "{n} pending" 11 px `muted`;
//!   "×" (`px_1`, `muted`, hover `text`, id "secret-requests-close"). The
//!   demo banner `px_3 py_2`, bg `accent` at 9 %, 11 px `muted`. The error
//!   `px_3 py_2`, 11 px `danger`. The list (id "secret-requests-list")
//!   scrolls, `p_3` (12), cards `gap_3` (12); an empty line 12 px `muted`.
//!   Demo footer `p_3`, `border_t_1`, the "Reset demo" button.
//! - Card: `rounded_lg` (8), `border_1`, bg `elevated`, `p_3`, column
//!   `gap_2` (8): a row (`gap_2`) of the name (semibold 12 px, `flex_1`)
//!   and the status (11 px, `statusColor`); the reason 12 px `muted`; then
//!   the buttons (row `gap_2`) or the entry.
//! - Entry: column `gap_2`: the input (`rounded_md` 6, `border_1` `border`,
//!   bg `bg`, `px_2 py_1`, min height 22, masked "*"s, id
//!   "secret-demo-input"); the lifetimes (row `gap_1` (4), 11 px; each
//!   `px_2 py_1 rounded_md`, hover bg `selected`); the buttons (row
//!   `gap_2`): provideLabel (primary) and "Cancel".
//! - Buttons (ui.rs `action_button`): row `gap_1p5`, `px_2 py_0p5` (8 × 2),
//!   `rounded_md`, `border_1` (`accent` if primary else `border`), bg
//!   `accent` at 12 % if primary else `element`, hover bg `selected`.
//! - Alert: absolute, right 0, top 52 px, width 100 % up to 376 px, `p_3`
//!   around a card: column `gap_2`, `p_3`, `rounded_lg`, bg `statusBar`,
//!   `border_1` `accent`, `shadow_lg`; a row of "Secret requested"
//!   (semibold 12 px, `flex_1`) and "×" (`muted`, hover `text`, id
//!   "secret-alert-dismiss"); the name (12 px semibold, truncated); the
//!   reason (12 px `muted`, max 96 px, scrolling, id
//!   "secret-alert-reason"); the "Open Secrets" button.

use crate::picker::{canon, Mods};
use crate::session::clean;
use serde::Deserialize;
use serde_json::{json, Value as Json};
use std::collections::{HashSet, VecDeque};

/// How often the model polls (the workspace tick, `REFRESH_INTERVAL`).
pub const POLL_MS: f64 = 8_000.0;
/// The longest value `fleet secret provide` takes.
pub const MAX_VALUE: usize = 64 * 1024;
/// The default lifetime, seconds.
pub const DEFAULT_LIFETIME_S: u64 = 60 * 60;
/// The lifetime choices: (label, seconds).
pub const LIFETIMES: [(&str, u64); 3] = [("15 min", 900), ("1 hour", 3600), ("8 hours", 28800)];
/// The demo banner.
pub const DEMO_NOTE: &str = "UI demo · values are discarded, never sent to an agent";
/// The sidebar before the active session's list has loaded.
pub const LOADING: &str = "Loading secrets for this session…";
/// The sidebar with no requests.
pub const NONE: &str = "No secrets requested for this session.";
/// Submitting an empty draft.
pub const EMPTY_VALUE: &str = "Enter a value first";
/// Submitting an oversized draft.
pub const TOO_LONG: &str = "Value must be at most 64 KiB";
/// The scheme a terminal link asks for a secret with.
pub const LINK_PREFIX: &str = "ocho://secret/";

/// A (machine, session) pair.
pub type Source = (String, String);

/// A request's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Waiting for a value.
    Pending,
    /// Provided, until `expires_at` (epoch seconds).
    Provided(f64),
    /// Its value lapsed.
    Expired,
    /// The person declined it.
    Dismissed,
    /// The person withdrew a provided value.
    Revoked,
}

/// One request, as the sidebar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// The request's id.
    pub id: String,
    /// The secret's name (`STRIPE_API_KEY`).
    pub name: String,
    /// Why the agent asks.
    pub reason: String,
    /// Its state.
    pub status: Status,
}

/// One request as `fleet secret list` prints it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LiveRequest {
    /// The request's id.
    pub id: String,
    /// The secret's name.
    pub name: String,
    /// Why the agent asks.
    pub reason: String,
    /// `pending`, `provided`, `dismissed`, `revoked`, `expired`.
    pub status: String,
    /// When a provided value lapses, epoch seconds (absent while pending).
    #[serde(default)]
    pub expires_at: i64,
}

impl Request {
    /// A listed request at `epoch_s`: a provided one past its expiry, or of
    /// an unknown status, is expired.
    pub fn from_live(request: LiveRequest, epoch_s: f64) -> Self {
        let status = match request.status.as_str() {
            "pending" => Status::Pending,
            "provided" if request.expires_at as f64 > epoch_s => {
                Status::Provided(request.expires_at as f64)
            }
            "dismissed" => Status::Dismissed,
            "revoked" => Status::Revoked,
            _ => Status::Expired,
        };
        Self {
            id: request.id,
            name: request.name,
            reason: request.reason,
            status,
        }
    }
}

/// A request on a session the sidebar is not showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretAlert {
    /// The session's machine.
    pub machine: String,
    /// The session.
    pub session: String,
    /// The request's id.
    pub id: String,
    /// The secret's name.
    pub name: String,
    /// Why the agent asks.
    pub reason: String,
}

/// A `fleet` command with its stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// The argv after `fleet`.
    pub argv: Vec<String>,
    /// What the command reads on stdin ("" for none).
    pub stdin: String,
}

/// What a key did in the secret entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretKey {
    /// Esc: the draft is gone and the entry closed.
    Cleared,
    /// Enter: call `submit`.
    Submit,
    /// ⌘C / ⌘X: swallowed, the value never reaches the clipboard.
    Swallowed,
    /// The text field's own key.
    None,
}

/// The name in an `ocho://secret/NAME` link (terminal/links.rs
/// `secret_name`): letters, digits and `_`, not starting with a digit, at
/// most 80 bytes.
pub fn secret_name(uri: &str) -> Option<&str> {
    let name = uri.strip_prefix(LINK_PREFIX)?;
    (name.len() <= 80
        && !name.is_empty()
        && name
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())))
    .then_some(name)
}

fn parse_list(reply: Result<String, String>) -> Result<Vec<LiveRequest>, String> {
    reply.and_then(|output| {
        serde_json::from_str::<Vec<LiveRequest>>(&output)
            .map_err(|e| format!("Read secret requests: {e}"))
    })
}

/// The Secrets sidebar, its entry and the alerts.
#[derive(Clone, Debug, PartialEq)]
pub struct Requests {
    /// The sidebar is open.
    pub open: bool,
    /// The local demo: nothing reaches Fleet.
    pub demo: bool,
    /// The shown session's requests.
    pub items: Vec<Request>,
    /// The session `items` belong to.
    pub source: Option<Source>,
    /// A sidebar list is in flight.
    pub loading: bool,
    /// Alert lists in flight.
    pub alert_loading: HashSet<Source>,
    /// Requests already alerted: (machine, session, id).
    pub seen: HashSet<(String, String, String)>,
    /// Alerts, oldest first; the front one shows.
    pub alerts: VecDeque<SecretAlert>,
    /// A provide, dismiss or revoke is in flight.
    pub busy: bool,
    /// The last failure.
    pub error: Option<String>,
    /// The request whose entry is open.
    pub editing: Option<String>,
    /// The entry has the keyboard.
    pub focused: bool,
    /// The value typed so far (never in the view).
    pub draft: String,
    /// The chosen lifetime, seconds.
    pub lifetime_s: u64,
}

impl Default for Requests {
    fn default() -> Self {
        Self::new(false)
    }
}

fn demo_items() -> Vec<Request> {
    vec![
        Request {
            id: "stripe".into(),
            name: "STRIPE_API_KEY".into(),
            reason: "Check your Stripe webhook configuration".into(),
            status: Status::Pending,
        },
        Request {
            id: "github".into(),
            name: "GITHUB_TOKEN".into(),
            reason: "Inspect a private repository for this task".into(),
            status: Status::Pending,
        },
    ]
}

impl Requests {
    /// The state at launch; `demo` opens the sidebar on two sample requests.
    pub fn new(demo: bool) -> Self {
        Self {
            open: demo,
            demo,
            items: if demo { demo_items() } else { Vec::new() },
            source: None,
            loading: false,
            alert_loading: HashSet::new(),
            seen: HashSet::new(),
            alerts: VecDeque::new(),
            busy: false,
            error: None,
            editing: None,
            focused: false,
            draft: String::new(),
            lifetime_s: DEFAULT_LIFETIME_S,
        }
    }

    /// Requests waiting for a value.
    pub fn pending(&self) -> usize {
        self.items
            .iter()
            .filter(|request| request.status == Status::Pending)
            .count()
    }

    /// Replace the shown list (a new session drops the draft; so does the
    /// edited request leaving `pending`).
    pub fn replace_live(&mut self, source: Source, items: Vec<LiveRequest>, epoch_s: f64) {
        if self.source.as_ref() != Some(&source) {
            self.clear_draft();
        }
        self.source = Some(source);
        self.items = items
            .into_iter()
            .map(|r| Request::from_live(r, epoch_s))
            .collect();
        let editing_pending = self.editing.as_ref().is_none_or(|id| {
            self.items
                .iter()
                .any(|r| &r.id == id && r.status == Status::Pending)
        });
        if !editing_pending {
            self.clear_draft();
        }
        self.error = None;
    }

    /// Raise an alert for each new pending request on `source` unless the
    /// sidebar shows it, and drop alerts whose request is resolved.
    pub fn observe_live(&mut self, source: &Source, items: &[LiveRequest]) {
        let viewing = self.open && self.source.as_ref() == Some(source);
        let pending: HashSet<&str> = items
            .iter()
            .filter(|request| request.status == "pending")
            .map(|request| request.id.as_str())
            .collect();
        self.alerts.retain(|alert| {
            (&alert.machine, &alert.session) != (&source.0, &source.1)
                || (!viewing && pending.contains(alert.id.as_str()))
        });
        for request in items.iter().filter(|request| request.status == "pending") {
            if self
                .seen
                .insert((source.0.clone(), source.1.clone(), request.id.clone()))
                && !viewing
            {
                self.alerts.push_back(SecretAlert {
                    machine: source.0.clone(),
                    session: source.1.clone(),
                    id: request.id.clone(),
                    name: request.name.clone(),
                    reason: request.reason.clone(),
                });
            }
        }
    }

    /// Drop `source`'s alerts (its sidebar opened).
    pub fn clear_alerts_for_source(&mut self, source: &Source) {
        self.alerts
            .retain(|alert| (&alert.machine, &alert.session) != (&source.0, &source.1));
    }

    /// Close the entry and forget the draft.
    pub fn clear_draft(&mut self) {
        self.draft.clear();
        self.editing = None;
        self.focused = false;
    }

    /// The demo's provide: the value is discarded.
    pub fn provide(&mut self, epoch_s: f64) -> bool {
        if self.draft.is_empty() || !self.demo {
            return false;
        }
        let editing = self.editing.clone();
        if let Some(request) = self.items.iter_mut().find(|request| {
            Some(&request.id) == editing.as_ref() && request.status == Status::Pending
        }) {
            request.status = Status::Provided(epoch_s + self.lifetime_s as f64);
            self.clear_draft();
            return true;
        }
        false
    }

    /// "Reset demo".
    pub fn reset_demo(&mut self) {
        if self.demo {
            self.clear_draft();
            self.items = demo_items();
        }
    }

    // ----- opening -------------------------------------------------------------

    /// ToggleSecrets is offered (palette.rs filter): the demo, or the active
    /// tab has a session.
    pub fn can_toggle(&self, active: Option<&Source>) -> bool {
        self.demo || active.is_some()
    }

    /// ToggleSecrets: open (then `wanted_list` reads) or close (the draft goes).
    pub fn toggle(&mut self, active: Option<&Source>) {
        if !self.can_toggle(active) {
            return;
        }
        self.open = !self.open;
        if !self.open {
            self.clear_draft();
        }
    }

    /// The sidebar's "×".
    pub fn close(&mut self) {
        self.open = false;
        self.clear_draft();
    }

    /// The sidebar shows (ui.rs: open, and the demo or a session tab).
    pub fn visible(&self, active: Option<&Source>) -> bool {
        self.open && (self.demo || active.is_some())
    }

    /// "Provide value" on a card: its entry opens with the keyboard.
    pub fn edit(&mut self, id: &str) {
        self.clear_draft();
        self.editing = Some(id.to_string());
        self.focused = true;
    }

    /// A lifetime chip.
    pub fn set_lifetime(&mut self, seconds: u64) {
        self.lifetime_s = seconds;
    }

    /// The entry's text changed.
    pub fn input(&mut self, text: &str) {
        if self.editing.is_some() {
            self.draft = text.to_string();
        }
    }

    /// The entry lost the keyboard (a click elsewhere, a tab switch).
    pub fn blur(&mut self) {
        self.focused = false;
    }

    /// "Open Secrets" on the alert, once the model has opened the alert's
    /// session (`open_notification_session` returned true): the sidebar
    /// opens on it. Returns the list to read (`wanted_list`).
    pub fn open_from_alert(
        &mut self,
        source: &Source,
        active: Option<&Source>,
    ) -> Option<Vec<String>> {
        self.open = true;
        self.clear_alerts_for_source(source);
        self.wanted_list(active)
    }

    /// The alert's "×".
    pub fn dismiss_alert(&mut self) {
        self.alerts.pop_front();
    }

    // ----- the sidebar's list ---------------------------------------------------

    /// The list to read for the open sidebar (`refresh_secret_requests`),
    /// marking it in flight: `secret list M S`. Switching sessions clears
    /// the old list and draft; no session clears the list.
    pub fn wanted_list(&mut self, active: Option<&Source>) -> Option<Vec<String>> {
        if self.demo || !self.open || self.loading {
            return None;
        }
        let Some(source) = active.cloned() else {
            self.items.clear();
            self.source = None;
            return None;
        };
        self.clear_alerts_for_source(&source);
        if self.source.as_ref() != Some(&source) {
            self.items.clear();
            self.clear_draft();
            self.source = Some(source.clone());
        }
        self.loading = true;
        Some(vec!["secret".into(), "list".into(), source.0, source.1])
    }

    /// The reply to the list asked for `source`. Returns true when the
    /// active session changed meanwhile: read again (`wanted_list`).
    pub fn set_list(
        &mut self,
        source: &Source,
        active: Option<&Source>,
        reply: Result<String, String>,
        epoch_s: f64,
    ) -> bool {
        self.loading = false;
        if active != Some(source) {
            return true;
        }
        if self.source.as_ref() != Some(source) {
            return false;
        }
        match parse_list(reply) {
            Ok(items) => {
                self.observe_live(source, &items);
                self.replace_live(source.clone(), items, epoch_s);
            }
            Err(error) => self.error = Some(error),
        }
        false
    }

    // ----- alerts ---------------------------------------------------------------

    /// The lists to read for alerts (`refresh_secret_alerts`): every
    /// attached session but the one the open sidebar shows, one read each
    /// at a time. Each is marked in flight.
    pub fn wanted_alerts(
        &mut self,
        attached: &[Source],
        active: Option<&Source>,
    ) -> Vec<(Source, Vec<String>)> {
        if self.demo {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut unique: Vec<&Source> = Vec::new();
        for source in attached {
            if !unique.contains(&source) {
                unique.push(source);
            }
        }
        for source in unique {
            if self.open && active == Some(source) {
                continue;
            }
            if !self.alert_loading.insert(source.clone()) {
                continue;
            }
            out.push((
                source.clone(),
                vec![
                    "secret".into(),
                    "list".into(),
                    source.0.clone(),
                    source.1.clone(),
                ],
            ));
        }
        out
    }

    /// The reply to an alert read; `still_open` is whether a tab still has
    /// the session. Failures are silent.
    pub fn set_alert(&mut self, source: &Source, still_open: bool, reply: Result<String, String>) {
        self.alert_loading.remove(source);
        if !still_open {
            return;
        }
        if let Ok(items) = parse_list(reply) {
            self.observe_live(source, &items);
        }
    }

    // ----- actions ---------------------------------------------------------------

    /// "Provide value" in the entry, or Enter: `secret provide M S ID --ttl
    /// Ns` with the value on stdin (the draft clears now). The demo provides
    /// locally. `None` when nothing is sent (a validation error is shown).
    pub fn submit(&mut self, epoch_s: f64) -> Option<Command> {
        if self.busy {
            return None;
        }
        if self.draft.is_empty() {
            self.error = Some(EMPTY_VALUE.into());
            return None;
        }
        if self.draft.len() > MAX_VALUE {
            self.error = Some(TOO_LONG.into());
            return None;
        }
        if self.demo {
            self.provide(epoch_s);
            return None;
        }
        let (machine, session) = self.source.clone()?;
        let id = self.editing.clone()?;
        let value = std::mem::take(&mut self.draft);
        self.clear_draft();
        self.busy = true;
        self.error = None;
        Some(Command {
            argv: vec![
                "secret".into(),
                "provide".into(),
                machine,
                session,
                id,
                "--ttl".into(),
                format!("{}s", self.lifetime_s),
            ],
            stdin: value,
        })
    }

    /// "Dismiss" (`action` "dismiss") or "Revoke" ("revoke") on a card:
    /// `secret ACTION M S ID`. The demo changes the card locally.
    pub fn finish(&mut self, id: &str, action: &str) -> Option<Vec<String>> {
        if self.busy {
            return None;
        }
        if self.demo {
            if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
                item.status = if action == "dismiss" {
                    Status::Dismissed
                } else {
                    Status::Revoked
                };
            }
            return None;
        }
        let (machine, session) = self.source.clone()?;
        self.busy = true;
        self.error = None;
        Some(vec![
            "secret".into(),
            action.into(),
            machine,
            session,
            id.into(),
        ])
    }

    /// The reply to `submit` or `finish`; the model then reads the list
    /// again (`wanted_list`).
    pub fn set_action(&mut self, reply: Result<String, String>) {
        self.busy = false;
        if let Err(error) = reply {
            self.error = Some(error);
        }
    }

    // ----- terminal links ----------------------------------------------------------

    /// A clicked `ocho://secret/NAME` in a session's terminal: `secret link
    /// M S NAME`.
    pub fn wanted_link(source: &Source, name: &str) -> Vec<String> {
        vec![
            "secret".into(),
            "link".into(),
            source.0.clone(),
            source.1.clone(),
            name.into(),
        ]
    }

    /// The link's reply: `Ok(pending id)` (the model then opens the
    /// session and calls `open_from_link`), or the message to show
    /// ("Could not request secret: …").
    pub fn set_link(reply: Result<String, String>) -> Result<Option<String>, String> {
        match reply {
            Ok(output) => Ok(serde_json::from_str::<LiveRequest>(&output)
                .ok()
                .filter(|request| request.status == "pending")
                .map(|request| request.id)),
            Err(error) => Err(format!("Could not request secret: {error}")),
        }
    }

    /// The link's session opened: the sidebar opens on it, with the pending
    /// request's entry open. Returns the list to read.
    pub fn open_from_link(
        &mut self,
        pending: Option<String>,
        active: Option<&Source>,
    ) -> Option<Vec<String>> {
        self.open = true;
        let argv = self.wanted_list(active);
        if let Some(id) = pending {
            self.editing = Some(id);
        }
        argv
    }

    // ----- keys ---------------------------------------------------------------------

    /// A key while the entry has the keyboard (`focused` and `editing`).
    pub fn key(&mut self, name: &str, mods: &Mods) -> SecretKey {
        if !(self.focused && self.editing.is_some()) {
            return SecretKey::None;
        }
        match canon(name).as_str() {
            "escape" => {
                self.clear_draft();
                SecretKey::Cleared
            }
            "enter" if !mods.meta && !mods.ctrl => SecretKey::Submit,
            "c" | "x" if mods.meta => SecretKey::Swallowed,
            _ => SecretKey::None,
        }
    }

    // ----- view -----------------------------------------------------------------------

    /// A card's status line and its colour (a theme field).
    pub fn status_label(status: Status, epoch_s: f64) -> (String, &'static str) {
        match status {
            Status::Pending => ("Needs a value".into(), "warn"),
            Status::Provided(expires) if expires > epoch_s => (
                format!("Expires in {}m", ((expires - epoch_s) as u64).div_ceil(60)),
                "good",
            ),
            Status::Provided(_) | Status::Expired => ("Expired".into(), "muted"),
            Status::Dismissed => ("Dismissed".into(), "muted"),
            Status::Revoked => ("Revoked".into(), "muted"),
        }
    }

    /// The `SecretRequests` shape for the active tab's session.
    pub fn view(&self, active: Option<&Source>, epoch_s: f64) -> Json {
        let current = self.demo || self.source.as_ref() == active;
        let pending = if current { self.pending() } else { 0 };
        let empty = if !current {
            LOADING
        } else if self.items.is_empty() {
            NONE
        } else {
            ""
        };
        let cards: Vec<Json> = if current {
            self.items
                .iter()
                .map(|request| {
                    let (status, color) = Self::status_label(request.status, epoch_s);
                    let editing = self.editing.as_ref() == Some(&request.id);
                    let pending = request.status == Status::Pending;
                    let id = &request.id;
                    json!({
                        "id": format!("secret-card-{id}"),
                        "requestId": id,
                        "name": request.name,
                        "reason": request.reason,
                        "status": status,
                        "statusColor": color,
                        "editing": editing,
                        "entry": editing && pending,
                        "actions": pending && !editing,
                        "revoke": matches!(request.status, Status::Provided(_)),
                        "openId": format!("secret-open-{id}"),
                        "dismissId": format!("secret-dismiss-{id}"),
                        "revokeId": format!("secret-revoke-{id}"),
                        "provideId": format!("secret-provide-{id}"),
                        "cancelId": format!("secret-cancel-{id}"),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let lifetimes: Vec<Json> = LIFETIMES
            .iter()
            .map(|(label, seconds)| {
                json!({
                    "id": format!("secret-lifetime-{seconds}"),
                    "label": label,
                    "seconds": seconds,
                    "selected": self.lifetime_s == *seconds,
                })
            })
            .collect();
        let alert = match self.alerts.front() {
            Some(alert) => json!({
                "visible": true,
                "title": "Secret requested",
                "name": clean(&alert.name),
                "reason": clean(&alert.reason),
                "openLabel": "Open Secrets",
            }),
            None => json!({
                "visible": false, "title": "Secret requested", "name": "", "reason": "", "openLabel": "Open Secrets",
            }),
        };
        json!({
            "visible": self.visible(active),
            "title": "Secrets",
            "pending": if pending > 0 { format!("{pending} pending") } else { String::new() },
            "demo": if self.demo { DEMO_NOTE } else { "" },
            "error": self.error.clone().unwrap_or_default(),
            "empty": empty,
            "cards": cards,
            "masked": "*".repeat(self.draft.len()),
            "placeholder": if self.demo { "Enter demo value…" } else { "Enter value…" },
            "focused": self.focused && self.editing.is_some(),
            "provideLabel": if self.demo { "Provide demo value" } else { "Provide value" },
            "lifetimes": lifetimes,
            "busy": self.busy,
            "reset": if self.demo { "Reset demo" } else { "" },
            "alert": alert,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(m: &str, s: &str) -> Source {
        (m.into(), s.into())
    }

    fn live(status: &str) -> LiveRequest {
        LiveRequest {
            id: "request-one".into(),
            name: "API_KEY".into(),
            reason: "Test the handoff".into(),
            status: status.into(),
            expires_at: 0,
        }
    }

    #[test]
    fn pending_request_without_expiry_deserializes() {
        let response = r#"[{"id":"request-one","session":"session-one","name":"API_KEY","reason":"test","status":"pending","created_at":1790718836}]"#;
        let requests: Vec<LiveRequest> = serde_json::from_str(response).unwrap();
        assert_eq!(requests.len(), 1);
        let request = Request::from_live(requests.into_iter().next().unwrap(), 0.0);
        assert_eq!(request.status, Status::Pending);
        let mut provided = live("provided");
        provided.expires_at = 1000;
        assert_eq!(
            Request::from_live(provided.clone(), 400.0).status,
            Status::Provided(1000.0)
        );
        assert_eq!(Request::from_live(provided, 1000.0).status, Status::Expired);
        assert_eq!(
            Request::from_live(live("weird"), 0.0).status,
            Status::Expired
        );
    }

    #[test]
    fn switching_sessions_clears_the_previous_secret_draft() {
        let mut requests = Requests::new(false);
        requests.replace_live(
            src("machine-one", "session-one"),
            vec![live("pending")],
            0.0,
        );
        requests.edit("request-one");
        requests.input("dummy-value");
        requests.replace_live(src("machine-two", "session-two"), Vec::new(), 0.0);
        assert!(requests.draft.is_empty());
        assert!(requests.editing.is_none());
        assert!(requests.items.is_empty());
    }

    #[test]
    fn new_pending_request_alerts_once_and_resolved_request_clears_it() {
        let mut requests = Requests::new(false);
        let source = src("machine-one", "session-one");
        requests.observe_live(&source, &[live("pending")]);
        requests.observe_live(&source, &[live("pending")]);
        assert_eq!(requests.alerts.len(), 1);
        assert_eq!(requests.alerts.front().unwrap().machine, "machine-one");
        requests.observe_live(&source, &[live("provided")]);
        assert!(requests.alerts.is_empty());
    }

    #[test]
    fn opening_one_session_keeps_other_sessions_alerts() {
        let mut requests = Requests::new(false);
        let first = src("machine-one", "session-one");
        let second = src("machine-two", "session-two");
        requests.observe_live(&first, &[live("pending")]);
        requests.observe_live(&second, &[live("pending")]);
        requests.clear_alerts_for_source(&first);
        assert_eq!(requests.alerts.len(), 1);
        assert_eq!(requests.alerts.front().unwrap().machine, "machine-two");
    }

    #[test]
    fn visible_secrets_panel_does_not_alert_again() {
        let mut requests = Requests::new(false);
        requests.open = true;
        let source = src("machine-one", "session-one");
        requests.source = Some(source.clone());
        requests.observe_live(&source, &[live("pending")]);
        assert!(requests.alerts.is_empty());
        requests.open = false;
        requests.observe_live(&source, &[live("pending")]);
        assert!(requests.alerts.is_empty());
    }

    #[test]
    fn the_sidebar_lists_provides_over_stdin_and_never_shows_the_value() {
        let mut r = Requests::new(false);
        let s = src("m", "s");
        assert!(!r.can_toggle(None));
        r.toggle(Some(&s));
        assert!(r.open);
        assert_eq!(r.view(Some(&s), 0.0)["empty"], LOADING);
        let argv = r.wanted_list(Some(&s)).unwrap();
        assert_eq!(argv, ["secret", "list", "m", "s"]);
        assert!(r.wanted_list(Some(&s)).is_none(), "one read at a time");
        let reply = r#"[{"id":"a","name":"API_KEY","reason":"why","status":"pending"},
            {"id":"b","name":"TOKEN","reason":"r","status":"provided","expires_at":1600}]"#;
        assert!(!r.set_list(&s, Some(&s), Ok(reply.into()), 1000.0));
        let v = r.view(Some(&s), 1000.0);
        assert_eq!(v["pending"], "1 pending");
        assert_eq!(v["cards"][0]["status"], "Needs a value");
        assert_eq!(v["cards"][0]["statusColor"], "warn");
        assert_eq!(v["cards"][0]["actions"], true);
        assert_eq!(v["cards"][1]["status"], "Expires in 10m");
        assert_eq!(v["cards"][1]["revoke"], true);
        assert!(r.alerts.is_empty(), "the open sidebar does not alert");
        r.edit("a");
        assert_eq!(r.submit(1000.0), None);
        assert_eq!(r.error.as_deref(), Some(EMPTY_VALUE));
        r.input("hunter2");
        assert_eq!(r.view(Some(&s), 1000.0)["masked"], "*******");
        assert!(!r.view(Some(&s), 1000.0).to_string().contains("hunter2"));
        r.set_lifetime(900);
        let job = r.submit(1000.0).unwrap();
        assert_eq!(
            job.argv,
            ["secret", "provide", "m", "s", "a", "--ttl", "900s"]
        );
        assert_eq!(job.stdin, "hunter2");
        assert!(r.draft.is_empty() && r.editing.is_none() && r.busy);
        assert_eq!(r.finish("b", "revoke"), None, "busy");
        r.set_action(Err("boom".into()));
        assert_eq!(r.error.as_deref(), Some("boom"));
        assert_eq!(
            r.finish("b", "revoke").unwrap(),
            ["secret", "revoke", "m", "s", "b"]
        );
        r.set_action(Ok(String::new()));
        // A reply for a session no longer active asks to read again.
        r.loading = true;
        assert!(r.set_list(&s, Some(&src("m", "t")), Ok("[]".into()), 0.0));
        r.close();
        assert!(!r.open);
    }

    #[test]
    fn alerts_poll_other_sessions_and_open_the_sidebar() {
        let mut r = Requests::new(false);
        let a = src("m", "a");
        let b = src("m", "b");
        let wanted = r.wanted_alerts(&[a.clone(), b.clone(), a.clone()], Some(&a));
        assert_eq!(wanted.len(), 2);
        assert!(
            r.wanted_alerts(std::slice::from_ref(&a), None).is_empty(),
            "in flight"
        );
        r.set_alert(
            &b,
            true,
            Ok(r#"[{"id":"x","name":"K\u0007EY","reason":"r","status":"pending"}]"#.into()),
        );
        let v = r.view(Some(&a), 0.0);
        assert_eq!(v["alert"]["visible"], true);
        assert_eq!(v["alert"]["name"], "KEY");
        r.set_alert(&a, false, Ok("[]".into()));
        assert!(r.alert_loading.is_empty());
        let argv = r.open_from_alert(&b, Some(&b)).unwrap();
        assert_eq!(argv, ["secret", "list", "m", "b"]);
        assert!(r.alerts.is_empty());
        // The open sidebar's session is not polled for alerts.
        assert_eq!(
            r.wanted_alerts(std::slice::from_ref(&b), Some(&b)),
            Vec::new()
        );
    }

    #[test]
    fn terminal_links_name_a_secret_and_open_its_entry() {
        assert_eq!(
            secret_name("ocho://secret/TEST_API_KEY"),
            Some("TEST_API_KEY")
        );
        for invalid in [
            "ocho://secret/../BAD",
            "ocho://secret/KEY?value=oops",
            "ocho://secret/1BAD",
            "ocho://secret/",
        ] {
            assert!(secret_name(invalid).is_none(), "{invalid}");
        }
        let s = src("m", "s");
        assert_eq!(
            Requests::wanted_link(&s, "KEY"),
            ["secret", "link", "m", "s", "KEY"]
        );
        let pending = Requests::set_link(Ok(
            r#"{"id":"r1","name":"KEY","reason":"","status":"pending"}"#.into(),
        ))
        .unwrap();
        assert_eq!(pending.as_deref(), Some("r1"));
        assert_eq!(
            Requests::set_link(Err("no".into())),
            Err("Could not request secret: no".into())
        );
        let mut r = Requests::new(false);
        assert!(r.open_from_link(pending, Some(&s)).is_some());
        assert!(r.open);
        assert_eq!(r.editing.as_deref(), Some("r1"));
    }

    #[test]
    fn entry_keys_and_the_demo() {
        let mut r = Requests::new(true);
        assert!(r.open && r.visible(None));
        assert_eq!(r.view(None, 0.0)["cards"].as_array().unwrap().len(), 2);
        assert!(r.wanted_list(Some(&src("m", "s"))).is_none());
        assert_eq!(r.key("Enter", &Mods::NONE), SecretKey::None);
        r.edit("stripe");
        r.input("v");
        let cmd = Mods {
            meta: true,
            ..Mods::NONE
        };
        assert_eq!(r.key("c", &cmd), SecretKey::Swallowed);
        assert_eq!(r.key("Enter", &Mods::NONE), SecretKey::Submit);
        assert_eq!(r.submit(100.0), None);
        assert_eq!(r.items[0].status, Status::Provided(100.0 + 3600.0));
        assert_eq!(r.finish("github", "dismiss"), None);
        assert_eq!(r.items[1].status, Status::Dismissed);
        r.edit("github");
        assert_eq!(r.key("Escape", &Mods::NONE), SecretKey::Cleared);
        assert!(r.editing.is_none());
        r.reset_demo();
        assert_eq!(r.pending(), 2);
        let v = r.view(None, 0.0);
        assert_eq!(v["demo"], DEMO_NOTE);
        assert_eq!(v["provideLabel"], "Provide demo value");
        assert_eq!(v["lifetimes"][1]["selected"], true);
        assert_eq!(v["reset"], "Reset demo");
    }
}
