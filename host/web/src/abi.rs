//! The wasm ABI, with no `unsafe`.
//!
//! @ref LLP 1007 §3
//!
//! The exports include `exact_plan` for the build's exact baked bytes,
//! `exact_boot_plan` for the dev loop, `exact_fonts` for plan-owned font
//! catalog data, and `exact_agent` for LLP 1012's agent API.
//! The glue never hands the host a pointer it did not get from
//! the host: `exact_in(len)` resizes a host-owned input buffer and returns its
//! address; the glue writes the payload there; every call returns the length
//! of the output buffer, whose address `exact_out()` reports. Both buffers
//! are plain `Vec<u8>`s in a thread-local; wasm is single-threaded.
//!
//! The macro [`host!`] instantiates these exports for one app: its data source
//! and its baked plan bytes. An app's wasm crate is one line.

use crate::host::Host;
use exact_runner::{DataSource, Event};
use std::cell::RefCell;

/// The buffers and the host behind the exports.
pub struct Bridge<D: DataSource> {
    host: Option<Host<D>>,
    /// The page's snapshot of the app's kept secrets (LLP 1018 D6), handed
    /// in through `exact_store` before boot and taken by the next boot.
    snapshot: Vec<(String, String)>,
    /// The archive's `compat.json` (LLP 1030 D3a), from the `host!`
    /// invocation: what the runner's `delivery` resource says about this
    /// binary's cohort, its update store, and its executors.
    compat: Option<&'static str>,
    /// A rendered page's digest and checkpoint (LLP 1048.000 D6), handed in
    /// through `exact_checkpoint` before boot and taken by the next boot.
    checkpoint: Option<(String, String)>,
    /// The user's display preferences as the page's media queries last
    /// reported them (LLP 1061 D4): handed in with each boot and resize, so
    /// a dev restart boots under the current ones.
    preferences: exact_runner::Preferences,
    /// What the artifact links that is generic over `D` (LLP 1047 D3), from
    /// the `host!` invocation; the core alone until it says.
    links: crate::HostLinks<D>,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl<D: DataSource> Bridge<D> {
    /// Empty; `boot` fills it.
    pub const fn new() -> Bridge<D> {
        Bridge {
            host: None,
            snapshot: Vec::new(),
            compat: None,
            checkpoint: None,
            preferences: exact_runner::Preferences {
                reduced_motion: false,
                reduced_transparency: false,
            },
            links: crate::HostLinks::CORE,
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    /// This wasm's `compat.json` (LLP 1030 D3a), for the delivery facts
    /// every subsequent boot hands the runner before its first frame. The
    /// `host!` macro passes the app's `COMPAT` const; nothing crosses the
    /// wasm ABI for it.
    pub fn set_compat(&mut self, json: &'static str) {
        self.compat = Some(json);
    }

    /// What this artifact links that is generic over `D` (LLP 1047 D3): the
    /// `host!` macro passes [`crate::HostLinks::of`] the entry's set.
    pub fn set_links(&mut self, links: crate::HostLinks<D>) {
        self.links = links;
    }

    /// The page's snapshot of the app's kept secrets (LLP 1018 D6): the
    /// input buffer's first `len` bytes as `name NUL value NUL …`, taken by
    /// the next `boot` (a `boot_plan` carries the running store instead).
    pub fn store(&mut self, len: usize) {
        let bytes = &self.input[..len.min(self.input.len())];
        let text = String::from_utf8_lossy(bytes);
        let mut parts = text.split('\0');
        let mut snapshot = Vec::new();
        while let (Some(name), Some(value)) = (parts.next(), parts.next()) {
            if !name.is_empty() {
                snapshot.push((name.to_string(), value.to_string()));
            }
        }
        self.snapshot = snapshot;
    }

    /// A rendered page's digest, a newline, and its checkpoint as the page
    /// carries it (LLP 1048.000 D6): the input buffer's first `len` bytes,
    /// taken by the next `boot`.
    pub fn checkpoint(&mut self, len: usize) {
        let text = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.checkpoint = text
            .split_once('\n')
            .map(|(digest, page)| (digest.to_string(), page.to_string()));
    }

    /// The page's `prefers-reduced-motion`/`-transparency` as bits
    /// ([`exact_runner::Preferences::from_bits`]), for the next boot.
    pub fn set_preferences(&mut self, bits: u32) {
        self.preferences = exact_runner::Preferences::from_bits(bits);
    }

    /// UTF-8 launch location carried after optional plan bytes.
    /// @ref LLP 1038 D5 — the page supplies pathname plus search before boot.
    pub fn launch_input(&self, start: usize, len: usize) -> String {
        let start = start.min(self.input.len());
        let end = start.saturating_add(len).min(self.input.len());
        String::from_utf8_lossy(&self.input[start..end]).into_owned()
    }

    /// Resize the input buffer and return its address.
    pub fn input(&mut self, len: usize) -> *mut u8 {
        self.input.clear();
        self.input.resize(len, 0);
        self.input.as_mut_ptr()
    }

    /// The output buffer's address.
    pub fn output(&self) -> *const u8 {
        self.output.as_ptr()
    }

    /// Write `bytes` into the input buffer (what the glue does through the
    /// address `input` returned); the length written.
    pub fn input_write(&mut self, bytes: &[u8]) -> usize {
        self.input.clear();
        self.input.extend_from_slice(bytes);
        self.input.len()
    }

    /// The output buffer's first `len` bytes.
    pub fn output_bytes(&self, len: usize) -> &[u8] {
        &self.output[..len.min(self.output.len())]
    }

    /// A named refusal (LLP 1047 D6) as the export's reply.
    pub fn refuse(&mut self, message: &str) -> u32 {
        self.emit(exact_runner::agent::error(message))
    }

    fn emit(&mut self, s: String) -> u32 {
        self.output = s.into_bytes();
        self.output.len() as u32
    }

    /// Copy the plan baked into an app wasm into the output buffer. The web
    /// build extracts this after linking, so its app.plan cannot come from a
    /// different source snapshot than the plan `exact_boot` will use.
    pub fn baked_plan(&mut self, plan: &[u8]) -> u32 {
        self.output.clear();
        self.output.extend_from_slice(plan);
        self.output.len() as u32
    }

    /// Binary-admitted module metadata, without activating any logic.
    pub fn logic_info(&mut self, data: D) -> u32 {
        if let Some(revision) = data.revision() {
            let mut json = String::from("{");
            for (i, (key, value)) in [
                ("appId", data.app_id()),
                ("grants", data.grants()),
                ("revision", revision),
                ("placement", data.placement().name()),
            ]
            .iter()
            .enumerate()
            {
                if i > 0 {
                    json.push(',');
                }
                exact_runner::agent::quote(key, &mut json);
                json.push(':');
                exact_runner::agent::quote(value, &mut json);
            }
            json.push('}');
            self.emit(json)
        } else {
            self.emit("null".into())
        }
    }

    /// Activate after first pixel; no-op for binary-bound sources.
    pub fn data_ready(&mut self) -> u32 {
        let batch = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            Host::data_ready,
        );
        self.emit(batch)
    }

    /// Whether the current runner still owns a request ticket.
    pub fn request_active(&self, ticket: f64) -> u32 {
        u32::from(
            ticket.is_finite()
                && ticket >= 0.0
                && self.host.as_ref().is_some_and(|host| {
                    host.runner()
                        .pending()
                        .iter()
                        .any(|(_, held)| *held == ticket as u64)
                }),
        )
    }

    /// The input concatenates plan, pairing receipt, and browser environment id.
    /// The JS loader prepares that private environment before this synchronous swap.
    pub fn boot_module(&mut self, lengths: [usize; 3], admitted: D) -> u32 {
        if self
            .host
            .as_ref()
            .is_some_and(|host| host.runner().has_pending())
        {
            return self.emit(exact_runner::agent::error(
                "module replacement waits for in-flight requests to settle; retry the update",
            ));
        }
        let [plan, receipt, module] = lengths;
        if plan
            .checked_add(receipt)
            .and_then(|n| n.checked_add(module))
            != Some(self.input.len())
            || plan > 32 << 20
            || receipt > 1 << 20
            || module > 32 << 20
        {
            return self.emit(exact_runner::agent::error(
                "invalid module generation lengths",
            ));
        }
        let result = std::str::from_utf8(&self.input[plan..plan + receipt])
            .map_err(|e| e.to_string())
            .and_then(|receipt_text| {
                admitted
                    .replacement(
                        &self.input[..plan],
                        receipt_text,
                        self.input[plan + receipt..].to_vec(),
                    )
                    .map_err(|e| format!("{e:?}"))
            });
        let mut data = match result {
            Ok(data) => data,
            Err(error) => return self.emit(exact_runner::agent::error(&error)),
        };
        if let Err(error) = data.activate() {
            return self.emit(exact_runner::agent::error(&format!("{error:?}")));
        }
        let viewport = self
            .host
            .as_ref()
            .map_or_else(Default::default, |h| h.runner().viewport());
        let launch = self.host.as_ref().map_or("/", |h| h.location()).to_owned();
        self.boot_plan(plan, data, viewport.width, viewport.height, &launch)
    }

    /// Boot from `plan` with `data` and the snapshot `store` handed in; the
    /// output is the first batch.
    pub fn boot(&mut self, plan: &[u8], data: D, width: f64, height: f64, launch: &str) -> u32 {
        let snapshot = std::mem::take(&mut self.snapshot);
        let viewport = exact_runner::Viewport {
            width,
            height,
            preferences: self.preferences,
        };
        let booted = match self.checkpoint.take() {
            Some((digest, page)) => Host::boot_checkpoint_linked(
                self.links,
                plan,
                data,
                &page,
                &digest,
                snapshot,
                self.compat,
                viewport,
                launch,
            ),
            None => Host::boot_linked(
                self.links,
                plan,
                data,
                None,
                snapshot,
                self.compat,
                viewport,
                launch,
            ),
        };
        match booted {
            Ok((host, batch)) => {
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// Boot from the input buffer's first `len` bytes — the dev loop's
    /// restart from a freshly compiled plan. Build the candidate beside the
    /// live host: only a successful boot replaces it, while a refusal leaves
    /// the old runner available to its page and in-flight work.
    pub fn boot_plan(&mut self, len: usize, data: D, width: f64, height: f64, launch: &str) -> u32 {
        let plan = self.input[..len.min(self.input.len())].to_vec();
        let carried = self.host.as_ref().map(Host::carry);
        match Host::boot_linked(
            self.links,
            &plan,
            data,
            carried.as_ref(),
            Vec::new(),
            self.compat,
            exact_runner::Viewport {
                width,
                height,
                preferences: self.preferences,
            },
            launch,
        ) {
            Ok((host, batch)) => {
                self.host = Some(host);
                self.emit(batch)
            }
            Err(e) => self.emit(format!(
                "{{\"ops\":[],\"timers\":false,\"error\":\"boot: {}\"}}",
                escape(&format!("{e:?}"))
            )),
        }
    }

    /// The locations `plan` renders at build (LLP 1048.000 D7), as a JSON
    /// array: the web build runs no render step when there are none.
    pub fn build_locations(&mut self, plan: &[u8]) -> u32 {
        // The routes whose pages a source lists count too, by pattern.
        let found = exact_plan::Plan::decode(plan)
            .map_err(|e| format!("{e:?}"))
            .and_then(|plan| {
                let mut found = crate::document::build_locations(&plan)?;
                for row in plan.routes.iter().filter(|r| {
                    r.render == exact_plan::RenderPolicy::Build && !plan.str(r.pages).is_empty()
                }) {
                    found.push((plan.str(row.pattern).to_string(), false));
                }
                Ok(found)
            });
        let out = match found {
            Ok(found) => {
                let mut json = String::from("[");
                for (i, (location, _)) in found.iter().enumerate() {
                    if i > 0 {
                        json.push(',');
                    }
                    json.push('"');
                    json.push_str(&escape(location));
                    json.push('"');
                }
                json + "]"
            }
            Err(error) => format!("{{\"error\":\"{}\"}}", escape(&error)),
        };
        self.emit(out)
    }

    /// Inspect candidate fonts while the live host continues to run.
    pub fn plan_fonts(&mut self, len: usize) -> u32 {
        let bytes = &self.input[..len.min(self.input.len())];
        match crate::host::plan_font_catalog(bytes) {
            Ok(catalog) => self.emit(catalog),
            Err(error) => self.emit(format!("{{\"error\":\"{}\"}}", escape(&error.to_string()))),
        }
    }

    /// Query the current plan's declared face catalog separately from the
    /// operation batch returned by boot and dispatch calls.
    pub fn fonts(&mut self) -> u32 {
        let out = self
            .host
            .as_ref()
            .map_or_else(|| "[]".to_string(), |host| host.font_catalog().to_string());
        self.emit(out)
    }

    /// Report a list scrollport without dispatching an application event.
    #[allow(clippy::too_many_arguments)]
    pub fn list_viewport(
        &mut self,
        view: u32,
        top: f64,
        height: f64,
        width: f64,
        origin: f64,
        focus: u32,
        interaction: u32,
        len: usize,
        limit: usize,
    ) -> u32 {
        let payload = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let rows: Result<Vec<(u32, f64)>, ()> = payload
            .lines()
            .map(|line| {
                let (id, height) = line.split_once(',').ok_or(())?;
                Ok((
                    id.parse().map_err(|_| ())?,
                    exact_num::parse_f64(height).map_err(|_| ())?,
                ))
            })
            .collect();
        let Ok(rows) = rows else {
            return self.emit(r#"{"ops":[],"error":"invalid list measurements"}"#.into());
        };
        let out = match self.host.as_mut() {
            Some(host) => host.list_viewport_within(
                view,
                exact_runner::ListViewport {
                    top,
                    height,
                    width,
                    origin,
                    pins: [focus, interaction],
                    rows: &rows,
                    ..Default::default()
                },
                (limit != 0).then_some(limit),
            ),
            None => r#"{"ops":[],"error":"not booted"}"#.to_string(),
        };
        self.emit(out)
    }

    /// Whether a budgeted list report left creation or retirement for another frame.
    pub fn list_pending(&self, view: u32) -> bool {
        self.host
            .as_ref()
            .and_then(|host| host.runner().list_status(view))
            .is_some_and(|status| status.pending)
    }

    /// Whether the location in the input buffer names a declared route
    /// (LLP 1038 §7): the page then follows a same-origin link in place.
    pub fn route_matches(&self, len: usize) -> u32 {
        let location = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        u32::from(
            self.host
                .as_ref()
                .is_some_and(|host| host.route_matches(&location)),
        )
    }

    /// Resolve an opaque list key, or return the absent-index sentinel.
    pub fn list_index(&self, view: u32, len: usize) -> u32 {
        let key = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.host
            .as_ref()
            .and_then(|h| h.runner().list_index(view, &key))
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(u32::MAX)
    }

    /// Logical text for all rows (empty keys), or two UTF-16 endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn list_text(
        &mut self,
        view: u32,
        first_len: usize,
        len: usize,
        first_paragraph: usize,
        first_offset: usize,
        last_paragraph: usize,
        last_offset: usize,
    ) -> u32 {
        let bytes = &self.input[..len.min(self.input.len())];
        if first_len > bytes.len() {
            return self.emit(String::new());
        }
        let first = String::from_utf8_lossy(&bytes[..first_len]);
        let last = String::from_utf8_lossy(&bytes[first_len..]);
        let range = (first_len != 0).then_some((
            exact_runner::ListTextPosition {
                key: &first,
                paragraph: first_paragraph,
                offset: first_offset,
            },
            exact_runner::ListTextPosition {
                key: &last,
                paragraph: last_paragraph,
                offset: last_offset,
            },
        ));
        let text = self
            .host
            .as_ref()
            .and_then(|h| h.runner().list_text(view, range).ok())
            .unwrap_or_default();
        self.emit(text)
    }

    /// Dispatch an event at `now_ms` (the page's clock); `kind` is 0 = press,
    /// 1 = change, 2 = hover in, 3 = hover out, 4 = focus, 5 = blur, 6 = key,
    /// 7 = submit, 8 = load, 9 = message (the payload — a change's text, a
    /// key's name, or a guest message — is the input buffer's first `len`
    /// bytes, UTF-8).
    /// Kind 14 is navigate: one UTF-8 location at the navigation root (LLP 1038 D8).
    pub fn dispatch(&mut self, view: u32, kind: u32, len: usize, now_ms: f64) -> u32 {
        let payload =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let event = match kind {
            0 => Event::Press,
            2 => Event::Hover(true),
            3 => Event::Hover(false),
            4 => Event::Focus,
            5 => Event::Blur,
            6 => Event::Key(payload),
            7 => Event::Submit,
            8 => Event::Load,
            9 => Event::Message(payload),
            10 => Event::Contextmenu,
            11 => Event::Dblclick,
            12 => Event::Swiperight,
            13 => {
                let Some(event) = Event::scroll_payload(&payload) else {
                    return self
                        .emit(r#"{"ops":[],"error":"invalid scroll coordinates"}"#.to_string());
                };
                event
            }
            // @ref LLP 1038 D8 — the next ABI kind after scroll.
            14 => Event::Navigate(payload),
            15 => {
                let Some(event) = Event::height_release_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid height release"}"#.into());
                };
                event
            }
            16 | 17 => {
                let event = if kind == 16 {
                    Event::transform_geometry_payload(&payload)
                } else {
                    Event::transform_release_payload(&payload)
                };
                let Some(event) = event else {
                    return self.emit(r#"{"ops":[],"error":"invalid transform event"}"#.into());
                };
                event
            }
            19 => {
                let Some(event) = Event::media_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid media event"}"#.into());
                };
                event
            }
            18 => {
                let Some(event) = self.input.get(..len).and_then(Event::reorder_drop_payload)
                else {
                    return self.emit(r#"{"ops":[],"error":"invalid reorder event"}"#.into());
                };
                event
            }
            // @ref LLP 1043.000 §3 D8 — 18 is reorder, 19 is media.
            20 => {
                let Some(event) = Event::pan_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid pan deltas"}"#.into());
                };
                event
            }
            21 => {
                let Some(event) = Event::selection_payload(&payload) else {
                    return self.emit(r#"{"ops":[],"error":"invalid Markdown selection"}"#.into());
                };
                event
            }
            _ => Event::Change(payload),
        };
        let out = match self.host.as_mut() {
            Some(h) => h.dispatch_at(view, event, now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// A request's outcome from the page (LLP 1016 D2): the input buffer
    /// holds `hlen` bytes of header lines, then `blen` bytes of body.
    pub fn fulfill(
        &mut self,
        ticket: f64,
        kind: u32,
        status: u32,
        hlen: usize,
        blen: usize,
        now_ms: f64,
    ) -> u32 {
        let n = self.input.len();
        let hlen = hlen.min(n);
        let blen = blen.min(n - hlen);
        let headers = String::from_utf8_lossy(&self.input[..hlen]).into_owned();
        let body = self.input[hlen..hlen + blen].to_vec();
        let out = match self.host.as_mut() {
            Some(h) => h.fulfill_at(ticket as u64, kind, status, &headers, body, now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// A name alone clears a surface; name NUL JSON publishes it, even if empty.
    pub fn surface_record(&mut self, len: usize) -> u32 {
        let Ok(text) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.emit(exact_runner::agent::error("surface record: invalid UTF-8"));
        };
        let (name, json) = text
            .split_once('\0')
            .map_or((text, None), |(name, json)| (name, Some(json)));
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("surface record: not booted"),
            |host| host.surface_record(name, json),
        );
        self.emit(out)
    }

    /// Re-answer viewport resources and return the resulting batch. The page
    /// reports the size and the display preferences together, on a change
    /// of either (LLP 1061 D4).
    /// @ref LLP 1039 D2 — buffers remain host-owned, with no unsafe code.
    pub fn resize(&mut self, width: f64, height: f64, preferences: u32, now_ms: f64) -> u32 {
        self.set_preferences(preferences);
        let viewport = exact_runner::Viewport {
            width,
            height,
            preferences: self.preferences,
        };
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.resize(viewport, now_ms),
        );
        self.emit(out)
    }

    /// Re-answer `exactTime` resources (LLP 1027.000.000).
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> u32 {
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.set_time(epoch_at_zero, utc_offset),
        );
        self.emit(out)
    }

    /// A topic the page module announced, in the input buffer.
    pub fn changed(&mut self, len: usize) -> u32 {
        let Ok(topic) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.emit(exact_runner::agent::error("changed: invalid UTF-8"));
        };
        let topic = topic.to_owned();
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.changed(&topic),
        );
        self.emit(out)
    }

    /// The locale and time zone, as `locale NUL timeZone` in the input buffer.
    pub fn set_place(&mut self, len: usize) -> u32 {
        // `locale NUL timeZone`, then `NUL seed` at launch.
        let text = std::str::from_utf8(&self.input[..len.min(self.input.len())]).unwrap_or("");
        let mut fields = text.split('\0');
        let (Some(locale), Some(zone)) = (fields.next(), fields.next()) else {
            return self.emit(exact_runner::agent::error(
                "place: expected locale NUL timeZone",
            ));
        };
        let seed = fields.next().and_then(|s| s.parse::<f64>().ok());
        let (locale, zone) = (locale.to_owned(), zone.to_owned());
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.set_place(&locale, &zone, seed),
        );
        self.emit(out)
    }

    /// Apply the common LE collection feedback in the first `len` input bytes.
    /// Unlike events this reports layout facts and never advances the clock.
    pub fn collection_feedback(&mut self, len: usize) -> u32 {
        let out = match (self.host.as_mut(), self.input.get(..len)) {
            (Some(host), None) => crate::batch::Batch::new().finish(
                host.runner().timer_due_ms(),
                host.runner().now_ms(),
                Some("collection input length"),
            ),
            (Some(host), Some(bytes)) => host.collection_feedback(bytes),
            (None, _) => crate::batch::Batch::new().finish(None, 0.0, Some("not booted")),
        };
        self.emit(out)
    }

    /// Fixed 48-byte LE motion request: version/op/view/property u32,
    /// opaque serial u64, then x/y/clock-ms f64. Serials never cross as f64.
    pub fn motion(&mut self, len: usize) -> u32 {
        if self.input.get(..4) == Some(&3u32.to_le_bytes()) {
            let out = match (self.host.as_mut(), self.input.get(..len)) {
                (Some(host), Some(bytes)) => host.reorder_motion(bytes),
                (None, _) => exact_runner::agent::error("not booted"),
                _ => exact_runner::agent::error("malformed reorder input"),
            };
            return self.emit(out);
        }
        if len == 120 {
            let out = match (self.host.as_mut(), self.input.get(..len)) {
                (Some(host), Some(bytes)) => host.transform_motion(bytes),
                (None, _) => exact_runner::agent::error("not booted"),
                _ => exact_runner::agent::error("malformed transform input"),
            };
            return self.emit(out);
        }
        use exact_motion::{HoldEnd, Property, Value};
        let decoded = (|| -> Result<_, exact_plan::PlanError> {
            let mut r = exact_plan::bytes::Reader::new(
                self.input
                    .get(..len)
                    .filter(|_| len == 48)
                    .ok_or(exact_plan::PlanError::BadCount(len as u32))?,
            );
            if r.u32()? != 1 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            Ok((
                r.u32()?,
                r.u32()?,
                r.u32()?,
                r.u64()?,
                Value::new(r.f64()?, r.f64()?),
                r.f64()?,
            ))
        })();
        let out = (|| -> Result<String, String> {
            let (op, view, property, serial, value, now) =
                decoded.map_err(|_| "malformed motion input".to_string())?;
            let host = self.host.as_mut().ok_or("not booted")?;
            if op == 8 || op == 9 {
                if property != Property::Height as u32 {
                    return Err("height drag requires the height property".into());
                }
                if op == 8 {
                    let key = exact_kernel::NodeKey {
                        index: serial as u32,
                        generation: (serial >> 32) as u32,
                    };
                    let Some(binding) = host.height_drag_binding(view).filter(|b| b.handle == key) else {
                        return Ok("{\"accepted\":false}".into());
                    };
                    let target = host.runner().kernel().node_by_key(binding.target).expect("resolved").id;
                    return match host.begin_height_drag(key, value, now).map_err(|e| format!("{e:?}"))? {
                        Some((start, batch)) => Ok(format!(
                            "{{\"token\":\"{}\",\"target\":{target},\"value\":[{},{}],\"batch\":{batch}}}",
                            start.token.serial(), start.value.x, start.value.y)),
                        None => Ok("{\"accepted\":false}".into()),
                    };
                }
                return Ok(match host.dispatch_height_held(serial, view, value.x, value.y, now).map_err(|e| format!("{e:?}"))? {
                    Some(batch) => format!("{{\"accepted\":true,\"batch\":{batch}}}"),
                    None => "{\"accepted\":false}".into(),
                });
            }
            if op == 6 || op == 7 {
                if property != Property::Height as u32 {
                    return Err("height registration requires the height property".into());
                }
                let batch = host.set_height_owner((op == 6).then_some(view))?;
                return Ok(format!("{{\"accepted\":true,\"batch\":{batch}}}"));
            }
            if op == 0 {
                let property = *Property::ALL
                    .get(property as usize)
                    .ok_or("invalid motion property")?;
                return match host
                    .begin_hold(view, property, value, now)
                    .map_err(|e| format!("{e:?}"))?
                {
                    Some((start, batch)) => Ok(format!(
                        "{{\"token\":\"{}\",\"value\":[{},{}],\"batch\":{batch}}}",
                        start.token.serial(),
                        start.value.x,
                        start.value.y
                    )),
                    None => Ok("{\"accepted\":false}".into()),
                };
            }
            let batch = match op {
                1 => host.update_hold(serial, value, now),
                2 => host.end_hold(serial, HoldEnd::Release { velocity: value }, now),
                3 => host.end_hold(serial, HoldEnd::Cancel, now),
                4 => return Ok(format!("{{\"accepted\":{}}}", host.has_hold(serial))),
                5 => Ok(host.dispatch_held(serial, now)),
                _ => return Err("invalid motion operation".into()),
            }
            .map_err(|e| format!("{e:?}"))?;
            Ok(match batch {
                Some(batch) => format!("{{\"accepted\":true,\"batch\":{batch}}}"),
                None => "{\"accepted\":false}".into(),
            })
        })()
        .unwrap_or_else(|error| exact_runner::agent::error(&error));
        self.emit(out)
    }

    /// Move the clock.
    pub fn advance(&mut self, now_ms: f64, until_request: bool) -> u32 {
        let out = match self.host.as_mut() {
            Some(h) if until_request => h.advance_until_request(now_ms),
            Some(h) => h.advance(now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    /// An agent request (the input buffer's first `len` bytes, JSON); the
    /// output is the reply, not a batch.
    pub fn agent(&mut self, len: usize) -> u32 {
        let request =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let out = match self.host.as_ref() {
            Some(h) => h.agent(&request),
            None => exact_runner::agent::error("not booted"),
        };
        self.emit(out)
    }

    /// A page line into the runner's journal (`exact_log`).
    pub fn log(&mut self, len: usize) -> u32 {
        let line = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        if let Some(h) = self.host.as_mut() {
            h.log(&line);
        }
        0
    }
}

impl<D: DataSource> Default for Bridge<D> {
    fn default() -> Self {
        Bridge::new()
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A thread-local bridge cell, for the exports.
pub type Cell<D> = RefCell<Bridge<D>>;

/// Instantiate the web exports for one app.
///
/// `$data` is the app's `DataSource` type (constructed with `Default`);
/// `$plan` a `&'static [u8]` of baked plan bytes (typically `include_bytes!`
/// of what the app's `build.rs` wrote).
/// A fourth argument supplies a data factory; a fifth supplies the paired
/// module's `[receipt, browser script]` byte slices for bake extraction and loading.
///
/// The invoking crate defines `EXACT_LINKED`, the [`crate::Linked`] of the
/// capabilities its plan uses, and `EXACT_REPLACEMENT`, whether a running
/// page may take a new data module; the generated entry writes both
/// (`contract::web_linked`, LLP 1047 D3). Every boot registers the first.
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr, $new:expr, $module:expr) => {
        $crate::host!($data, $plan, $compat, $new);
        /// Copy one exact embedded module artifact for the web producer.
        #[no_mangle]
        pub extern "C" fn exact_module_artifact(index: u32) -> u32 {
            let artifacts: &[&[u8]] = &$module;
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan(artifacts.get(index as usize).copied().unwrap_or(&[])))
        }
    };
    ($data:ty, $plan:expr, $compat:expr) => {
        $crate::host!($data, $plan, $compat, || <$data as ::std::default::Default>::default());
    };
    ($data:ty, $plan:expr, $compat:expr, $new:expr) => {
        thread_local! {
            static EXACT_BRIDGE: $crate::abi::Cell<$data> = ::std::cell::RefCell::new($crate::abi::Bridge::new());
        }
        /// What this artifact links that is generic over its data source.
        const EXACT_HOST_LINKS: $crate::HostLinks<$data> = $crate::HostLinks::of(EXACT_LINKED);

        /// Resize the input buffer; returns its address.
        #[no_mangle]
        pub extern "C" fn exact_in(len: u32) -> *mut u8 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().input(len as usize))
        }

        /// The output buffer's address.
        #[no_mangle]
        pub extern "C" fn exact_out() -> *const u8 {
            EXACT_BRIDGE.with(|b| b.borrow().output())
        }

        /// Copy the plan baked into this wasm to the output buffer. The web
        /// build uses these exact bytes for app.plan and exact.json.
        #[no_mangle]
        pub extern "C" fn exact_plan() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan($plan))
        }

        /// Copy the exact compatibility receipt embedded in this wasm.
        #[no_mangle]
        pub extern "C" fn exact_compat() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().baked_plan($compat.as_bytes()))
        }

        /// The page's snapshot of the app's kept secrets (LLP 1018 D6), from
        /// the input buffer's first `len` bytes (`name NUL value NUL …`),
        /// for the next `exact_boot`.
        #[no_mangle]
        pub extern "C" fn exact_store(len: u32) {
            EXACT_BRIDGE.with(|b| b.borrow_mut().store(len as usize))
        }

        /// A rendered page's digest and checkpoint (LLP 1048.000 D6), from
        /// the input buffer's first `len` bytes, for the next `exact_boot`.
        #[no_mangle]
        pub extern "C" fn exact_checkpoint(len: u32) {
            EXACT_BRIDGE.with(|b| b.borrow_mut().checkpoint(len as usize))
        }

        /// The locations the baked plan renders at build (LLP 1048.000 D7),
        /// as a JSON array, for the web build.
        #[no_mangle]
        pub extern "C" fn exact_build_locations() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().build_locations($plan))
        }

        /// Boot; returns the first batch's length.
        #[no_mangle]
        pub extern "C" fn exact_boot(width: f64, height: f64, launch_len: u32, preferences: u32) -> u32 {
            $crate::link(EXACT_LINKED);
            EXACT_BRIDGE.with(|b| {
                let mut b = b.borrow_mut();
                b.set_compat($compat);
                b.set_preferences(preferences);
                b.set_links(EXACT_HOST_LINKS);
                let launch = b.launch_input(0, launch_len as usize);
                b.boot($plan, ($new)(), width, height, &launch)
            })
        }

        /// Boot from plan bytes in the input buffer (the dev loop's restart).
        #[no_mangle]
        pub extern "C" fn exact_boot_plan(len: u32, width: f64, height: f64, launch_len: u32, preferences: u32) -> u32 {
            $crate::link(EXACT_LINKED);
            EXACT_BRIDGE.with(|b| {
                let mut b = b.borrow_mut();
                b.set_compat($compat);
                b.set_preferences(preferences);
                b.set_links(EXACT_HOST_LINKS);
                let launch = b.launch_input(len as usize, launch_len as usize);
                b.boot_plan(len as usize, ($new)(), width, height, &launch)
            })
        }

        /// Metadata for the optional browser module loader.
        #[no_mangle]
        pub extern "C" fn exact_logic() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().logic_info(($new)()))
        }
        /// The first pixel has been painted; activate deferred logic.
        #[no_mangle]
        pub extern "C" fn exact_data_ready() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().data_ready())
        }
        /// Replace the paired plan and privately prepared browser module:
        /// only a build whose entry says `EXACT_REPLACEMENT` links it.
        #[no_mangle]
        pub extern "C" fn exact_boot_module(plan: u32, receipt: u32, module: u32) -> u32 {
            $crate::link(EXACT_LINKED);
            EXACT_BRIDGE.with(|b| {
                let mut b = b.borrow_mut();
                if !EXACT_REPLACEMENT {
                    return b.refuse("this build takes no module replacement (LLP 1047 D6)");
                }
                b.set_links(EXACT_HOST_LINKS);
                b.boot_module([plan as usize, receipt as usize, module as usize], ($new)())
            })
        }

        /// Inspect a plan's fonts without changing the live host.
        #[no_mangle]
        pub extern "C" fn exact_plan_fonts(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().plan_fonts(len as usize))
        }

        /// Query the current plan's declared font catalog. The returned JSON
        /// is separate from operation batches (LLP 1019 D5).
        #[no_mangle]
        pub extern "C" fn exact_fonts() -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().fonts())
        }

        /// Dispatch an event; returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_dispatch(view: u32, kind: u32, len: u32, now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().dispatch(view, kind, len as usize, now_ms))
        }

        /// Whether a location (the input buffer) names a declared route.
        #[no_mangle]
        pub extern "C" fn exact_route_match(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow().route_matches(len as usize))
        }

        /// A request's outcome (LLP 1016 D2): `kind` 0 response / 1 network /
        /// 2 refused / 3 unsupported / 4 aborted / 5 storage / 6 captured
        /// surface / 7 restored surface; the input buffer holds
        /// `hlen` bytes of `name: value` header lines then `blen` bytes of
        /// body (or the message). Returns the batch's length.
        #[no_mangle]
        pub extern "C" fn exact_fulfill(ticket: f64, kind: u32, status: u32, hlen: u32, blen: u32, now_ms: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().fulfill(ticket, kind, status, hlen as usize, blen as usize, now_ms))
        }

        /// The viewport or the display preferences (bit 0 reduced motion,
        /// bit 1 reduced transparency) changed; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_resize(width: f64, height: f64, now_ms: f64, preferences: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().resize(width, height, preferences, now_ms))
        }

        /// The date: Unix ms at clock zero and minutes east of UTC.
        #[no_mangle]
        pub extern "C" fn exact_set_time(epoch_at_zero: f64, utc_offset: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().set_time(epoch_at_zero, utc_offset))
        }

        /// A topic the page module announced, in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_changed(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().changed(len as usize))
        }

        /// The locale and time zone: `locale NUL timeZone` in the input buffer.
        #[no_mangle]
        pub extern "C" fn exact_set_place(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().set_place(len as usize))
        }

        /// Advance the runner clock; nonzero `until_request` stops after a
        /// timer that sends (the agent's jump; the wall clock passes 0).
        #[no_mangle]
        pub extern "C" fn exact_advance(now_ms: f64, until_request: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().advance(now_ms, until_request != 0))
        }

        /// An agent request from the input buffer; returns the reply's length.
        #[no_mangle]
        pub extern "C" fn exact_agent(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().agent(len as usize))
        }

        /// A page line for the runner's journal (LLP 1012 §3; LLP 1035.001
        /// D6): the input buffer's first `len` bytes. Returns 0.
        #[no_mangle]
        pub extern "C" fn exact_log(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().log(len as usize))
        }
    };
}

/// Motion's export group (LLP 1047 D3), beside [`host!`]: the generated entry
/// invokes it when the plan uses motion, and the glue calls it only for a
/// hold the plan's gestures begin.
#[macro_export]
macro_rules! motion_exports {
    () => {
        /// Input-driven presentation ownership, with exact u64 token bytes.
        #[no_mangle]
        pub extern "C" fn exact_motion(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().motion(len as usize))
        }
    };
}

/// Lists' export group (LLP 1047 D3), beside [`host!`]: the generated entry
/// invokes it when the plan has a list the host windows, and the glue calls
/// it only for one.
#[macro_export]
macro_rules! list_exports {
    () => {
        /// Report a list scrollport and up to two pinned descendants.
        #[no_mangle]
        pub extern "C" fn exact_list(
            view: u32,
            top: f64,
            height: f64,
            width: f64,
            origin: f64,
            focus: u32,
            interaction: u32,
            len: u32,
            limit: u32,
        ) -> u32 {
            EXACT_BRIDGE.with(|b| {
                b.borrow_mut().list_viewport(
                    view,
                    top,
                    height,
                    width,
                    origin,
                    focus,
                    interaction,
                    len as usize,
                    limit as usize,
                )
            })
        }

        /// Continue a budgeted window on the next animation frame.
        #[no_mangle]
        pub extern "C" fn exact_list_pending(view: u32) -> u32 {
            EXACT_BRIDGE.with(|b| u32::from(b.borrow().list_pending(view)))
        }

        /// Resolve an opaque list key without mounting its row.
        #[no_mangle]
        pub extern "C" fn exact_list_index(view: u32, len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow().list_index(view, len as usize))
        }

        /// Copy logical text, including rows outside the mounted window.
        #[no_mangle]
        pub extern "C" fn exact_list_text(
            view: u32,
            first_len: u32,
            len: u32,
            first_paragraph: u32,
            first_offset: u32,
            last_paragraph: u32,
            last_offset: u32,
        ) -> u32 {
            EXACT_BRIDGE.with(|b| {
                b.borrow_mut().list_text(
                    view,
                    first_len as usize,
                    len as usize,
                    first_paragraph as usize,
                    first_offset as usize,
                    last_paragraph as usize,
                    last_offset as usize,
                )
            })
        }

        /// Report actual collection geometry through the shared binary decoder.
        #[no_mangle]
        pub extern "C" fn exact_collection_feedback(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().collection_feedback(len as usize))
        }
    };
}

/// GPU surfaces' export group (LLP 1047 D3), beside [`host!`]: the generated
/// entry invokes it when the plan has a canvas with a surface, and the glue
/// calls it only for one.
#[macro_export]
macro_rules! surface_exports {
    () => {
        /// Whether a presenter-owned operation may still affect its surface.
        #[no_mangle]
        pub extern "C" fn exact_request_active(ticket: f64) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow().request_active(ticket))
        }

        /// Publish or clear a named surface record; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_surface_record(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| {
                let Ok(mut bridge) = b.try_borrow_mut() else {
                    eprintln!("exact_surface_record refused: nested bridge export");
                    return 0;
                };
                bridge.surface_record(len as usize)
            })
        }
    };
}
