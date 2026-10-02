//! The session's command-side calls on the bridge: a command that shows
//! system UI about to run (`share`, LLP 1069.003; `saveFile`, LLP 1069.010),
//! an auth session's arm and report (LLP 1069.006), a host line into the
//! runner's journal, and a select's options for the menu the presenter
//! builds (LLP 1069.001 D5).
use super::Bridge;
use exact_runner::auth::{self, Arm, Browser};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// `exact_canvas_held`: a 2D canvas's replay is behind, or caught up.
    pub fn canvas_held(&mut self, view: u32, held: bool) {
        if let Some(h) = self.host.as_mut() {
            h.canvas_held(view, held);
        }
    }

    /// A host this bridge boots: its Canvas 2D text measurer (LLP 1056 D8)
    /// and where its draws run (LLP 1072 §8.5).
    pub(super) fn canvas_hooks(&self, host: &mut crate::host::Host<D>, hooks: &super::Hooks) {
        if let Some(f) = hooks.canvas_text {
            host.set_canvas_text(f, hooks.ctx);
        }
        host.set_canvas_deferred(self.canvas_deferred);
    }

    /// Canvas draws in a turn of their own from now on, or in every turn
    /// (`exact_canvas_defer`, LLP 1072 §8.5).
    pub fn canvas_defer(&mut self, deferred: bool) {
        self.canvas_deferred = deferred;
        if let Some(h) = self.host.as_mut() {
            h.set_canvas_deferred(deferred);
        }
    }

    /// The deferred canvas draws (`exact_canvas_draw`, LLP 1072 §8.5).
    pub fn canvas_draw(&mut self) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(super::not_booted, |h| h.canvas_draw());
        self.emit(out)
    }

    /// What SVG pixel work the live plan can need (`exact_svg_islands`):
    /// 1 an island, 2 a filter.
    pub fn svg_islands(&self) -> u8 {
        self.host.as_ref().map_or(0, |h| {
            let plan = h.runner().plan();
            u8::from(exact_runner::svg_islands(plan))
                | u8::from(exact_runner::svg_filters(plan)) << 1
        })
    }

    /// A command the session is about to run (`exact_command`).
    pub fn command(&mut self, len: usize) -> u32 {
        let request = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let out = match self.host.as_mut() {
            Some(h) => h.command(&request),
            None => exact_runner::agent::error("not booted"),
        };
        self.emit(out)
    }

    /// An `exact-auth:` request (LLP 1069.006 D3): refused now (the
    /// executor wakes to deliver it), or handed to the session to open an
    /// `ASWebAuthenticationSession` — which, under the agent, holds it
    /// instead (`exact_auth` `hold`).
    pub(super) fn auth_arm(
        h: &mut crate::host::Host<D>,
        x: &crate::executor::Executor,
        presenter: &mut crate::batch::Batch,
        r: &exact_runner::RequestOut,
    ) {
        match auth::arm(h.runner_mut(), r, false, Browser::Native, None) {
            Arm::Settled => x.notify(),
            Arm::Held => {}
            Arm::Present(session) => presenter.auth(r.ticket, &session),
        }
    }

    /// Sessions whose tickets the runner let go: the session cancels each
    /// and drops its late completion (D3).
    pub(super) fn auth_forgotten(
        h: &mut crate::host::Host<D>,
        presenter: &mut crate::batch::Batch,
    ) {
        for ticket in auth::forgotten(h.runner_mut()) {
            presenter.auth_cancel(ticket);
        }
    }

    /// The session's word on an auth request (`exact_auth`):
    /// `{"op":"hold","ticket":N}` under the agent, or `{"op":"done",
    /// "ticket":N,"url":…}` / `{…,"status":N,"message":…}` when it ended.
    /// The executor wakes so the next pump delivers the answer.
    pub fn auth(&mut self, len: usize) -> u32 {
        let json = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        let field = |k: &str| exact_runner::agent::field_str(&json, k);
        let ticket = exact_runner::agent::field_num(&json, "ticket").unwrap_or(0.0) as u64;
        if let Some(h) = self.host.as_mut() {
            let runner = h.runner_mut();
            match field("op").as_deref() {
                Some("hold") => auth::hold_opened(runner, ticket),
                Some("done") => match field("url") {
                    Some(url) => auth::complete(runner, ticket, Ok(&url)),
                    None => {
                        let status =
                            exact_runner::agent::field_num(&json, "status").unwrap_or(502.0) as u16;
                        let message = field("message").unwrap_or_default();
                        auth::complete(runner, ticket, Err((status, &message)))
                    }
                },
                _ => {}
            }
        }
        if let Some(x) = self.executor.as_ref() {
            x.notify();
        }
        0
    }

    /// A native button's face (`exact_button_face`, LLP 1069.011 D2, D5), as
    /// JSON in the output buffer: `{"title","symbol","leading","style","ios",
    /// "iosBefore26","macos","known"}` — the symbol as the platform names it,
    /// the style's row of the `buttonStyles` table (`bordered` for a name not
    /// in it). Not a batch: nothing changes.
    pub fn button_face(&mut self, view: u32) -> u32 {
        let quote = exact_runner::agent::quote;
        let (face, style) = self
            .host
            .as_ref()
            .map(|h| {
                let kernel = h.runner().kernel();
                let style = kernel
                    .node(view)
                    .and_then(|n| {
                        n.props
                            .str(exact_kernel::PropId::ButtonStyle)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "bordered".into());
                (kernel.button_face(view), style)
            })
            .unwrap_or_default();
        let row = exact_kernel::generated::button_style(&style);
        let drawn = row.or_else(|| exact_kernel::generated::button_style("bordered"));
        let mut json = String::from("{\"title\":");
        match &face.title {
            Some(t) => quote(t, &mut json),
            None => json.push_str("null"),
        }
        json.push_str(",\"symbol\":");
        match face
            .symbol
            .as_deref()
            .and_then(exact_kernel::generated::symbol)
        {
            Some((apple, _, _)) => quote(apple, &mut json),
            None => json.push_str("null"),
        }
        json.push_str(&format!(",\"leading\":{},\"style\":", face.leading));
        quote(&style, &mut json);
        if let Some(d) = drawn {
            json.push_str(",\"ios\":");
            quote(d.ios, &mut json);
            json.push_str(",\"iosBefore26\":");
            quote(d.ios_before_26, &mut json);
            json.push_str(",\"macos\":");
            quote(d.macos, &mut json);
        }
        json.push_str(&format!(",\"known\":{}}}", row.is_some()));
        self.output = json.into_bytes();
        self.output.len() as u32
    }

    /// A select's options (`exact_select_options`), as JSON in the output
    /// buffer: `[{"value","label","disabled"}]` in order, and which one it
    /// shows (`chosen`, an index or null). Not a batch: nothing changes.
    pub fn select_options(&mut self, view: u32) -> u32 {
        let quote = exact_runner::agent::quote;
        let mut json = String::from("{\"options\":[");
        let mut chosen = None;
        if let Some(h) = self.host.as_ref() {
            let kernel = h.runner().kernel();
            let shown = kernel.select_chosen(view).map(|c| c.view);
            for (i, c) in kernel.select_choices(view).iter().enumerate() {
                if Some(c.view) == shown {
                    chosen = Some(i);
                }
                json.push_str(if i == 0 {
                    "{\"value\":"
                } else {
                    ",{\"value\":"
                });
                quote(&c.value, &mut json);
                json.push_str(",\"label\":");
                quote(&c.label, &mut json);
                json.push_str(&format!(",\"disabled\":{}}}", c.disabled));
            }
        }
        json.push_str(&match chosen {
            Some(i) => format!("],\"chosen\":{i}}}"),
            None => "],\"chosen\":null}".into(),
        });
        self.output = json.into_bytes();
        self.output.len() as u32
    }

    /// A host line into the runner's journal (`exact_log`).
    pub fn log(&mut self, len: usize) -> u32 {
        let line = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        if let Some(h) = self.host.as_mut() {
            h.log(&line);
        }
        0
    }
}
