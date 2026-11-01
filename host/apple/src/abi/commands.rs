//! The session's command-side calls on the bridge: a command that shows
//! system UI about to run (`share`, LLP 1069.003; `saveFile`, LLP 1069.010),
//! an auth session's arm and report (LLP 1069.006), a host line into the
//! runner's journal, a select's options for the menu the presenter
//! builds (LLP 1069.001 D5), a radio's group (x2apps survey #2), a
//! grouped list's sections (LLP 1084), and
//! whether a followed link names one of the app's routes (LLP 1038 §7),
//! where the host's own Back goes (LLP 1115 D5), and a package's call of
//! the app's handler (LLP 1133.016 D15 item 1).
use super::Bridge;
use exact_runner::auth::{self, Arm, Browser};
use exact_runner::DataSource;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether a runner booted now counts its plan sites' work for `perf` from
/// boot (LLP 1079 D1): until a release build says it counts from the first
/// ask instead, and again once something has asked.
static WORK_FROM_BOOT: AtomicBool = AtomicBool::new(true);

/// A release build's choice (`exact_measure_work_later`): runners booted
/// from now count their work from the first ask (`Bridge::measure_work`).
pub fn measure_work_later() {
    WORK_FROM_BOOT.store(false, Ordering::Relaxed);
}

/// A runner just booted counts its work from now, or from the first ask.
pub fn measure_at_boot<D: DataSource>(runner: &mut exact_runner::Runner<D>, compat: Option<&str>) {
    if WORK_FROM_BOOT.load(Ordering::Relaxed) {
        runner.measure_unless_production(compat, true);
        runner.measure_kernel_layouts();
    } else {
        runner.measure_later(compat);
    }
}

/// The alert shape (LLP 1115 D6), the kernel's rule
/// (`exact_kernel::confirmation`), for the presenter's Swift
/// (`ConfirmationShape.isAlert`): lowering, this host and the Linux one read one rule.
pub fn confirmation_is_alert(texts: u32, actions: u32, cancels: u32) -> bool {
    exact_kernel::confirmation::is_alert(texts as usize, actions as usize, cancels as usize)
}

impl<D: DataSource> Bridge<D> {
    /// The first ask for `perf` (`exact_measure_work`): this runtime counts
    /// its plan sites' work from now, where its bake counts any, and every
    /// runtime booted after counts from boot.
    pub fn measure_work(&mut self) -> u32 {
        WORK_FROM_BOOT.store(true, Ordering::Relaxed);
        if let Some(host) = self.host.as_mut() {
            host.runner_mut().measure_from_now(self.compat, true);
            host.runner_mut().measure_kernel_layouts();
        }
        0
    }

    /// Register the presenter's inline geometry, kept across plan replacements.
    pub fn set_inline_geometry(
        &mut self,
        read: Option<crate::geometry::InlineFrameFn>,
        range: Option<crate::geometry::RangeFrameFn>,
        ctx: *mut std::ffi::c_void,
    ) {
        self.inline_geometry = read.map(|read| {
            std::rc::Rc::new(crate::geometry::InlineGeometry { read, range, ctx })
                as std::rc::Rc<dyn exact_runner::geometry::InlineGeometry>
        });
        if let Some(host) = self.host.as_mut() {
            host.runner_mut()
                .set_inline_geometry(self.inline_geometry.clone());
        }
    }

    /// `exact_route_matches`: whether the location in the input buffer names
    /// a pattern the plan's route table declares (LLP 1038 §7) — a link to
    /// it is followed in the app, as the web's same-document link is; any
    /// other path (a file beside a document) is the containing app's.
    pub fn route_matches(&self, len: usize) -> u32 {
        let location = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        u32::from(
            self.host
                .as_ref()
                .is_some_and(|h| h.runner().route_matches(&location)),
        )
    }

    /// `exact_module_invoke`: a package's call of the app's handler,
    /// `{"extension", "handle", "handler", "args"}` in the input buffer.
    /// `reply(ctx, answer, len)` is called once with the runner's JSON, now
    /// or when the handler's own work lands. Not a batch.
    pub fn module_invoke(&mut self, len: usize, reply: InvokeReply, ctx: *mut std::ffi::c_void) {
        let body = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        // The context crosses threads as an address: the host's, which `reply` frees.
        let ctx = ctx as usize;
        let answer: Box<dyn FnOnce(String) + Send> = Box::new(move |text: String| {
            reply(
                ctx as *mut std::ffi::c_void,
                text.as_ptr(),
                text.len() as u32,
            )
        });
        match self.host.as_mut() {
            Some(h) => h.runner_mut().invoke_module(&body, answer),
            None => answer(r#"{"error":"InvalidStateError","message":"no session runs"}"#.into()),
        }
        // The handler's requests, for the host to run.
        if let Some(executor) = self.executor.as_deref() {
            executor.notify();
        }
    }

    /// `exact_location_beneath`: the location of the visit beneath visit
    /// `id` on its stack, UTF-8 in the output buffer, empty when there is
    /// none — where the host's own Back goes for a route with no authored
    /// Back control (LLP 1115 D5). Not a batch: nothing changes.
    pub fn location_beneath(&mut self, id: u64) -> u32 {
        let location = self
            .host
            .as_ref()
            .and_then(|h| h.runner().location_beneath(id))
            .unwrap_or_default();
        self.output = location.into_bytes();
        self.output.len() as u32
    }

    /// Where the platform's Back goes from the selected visit, UTF-8;
    /// empty at the stack's root. @ref LLP 1115.000 D5.5
    pub fn location_behind(&mut self) -> u32 {
        self.output = self
            .host
            .as_ref()
            .and_then(|h| h.runner().location_behind())
            .unwrap_or_default()
            .into_bytes();
        self.output.len() as u32
    }

    /// Where the platform's Forward goes, UTF-8; empty with nothing ahead.
    /// @ref LLP 1115.000 D5.5
    pub fn location_ahead(&mut self) -> u32 {
        self.output = self
            .host
            .as_ref()
            .and_then(|h| h.runner().location_ahead())
            .unwrap_or_default()
            .into_bytes();
        self.output.len() as u32
    }

    /// A retained visit's canonical location, UTF-8; empty when absent.
    pub fn visit_location(&mut self, id: u64) -> u32 {
        self.output = self
            .host
            .as_ref()
            .and_then(|h| h.runner().visit_location(id))
            .unwrap_or_default()
            .into_bytes();
        self.output.len() as u32
    }

    /// `exact_scrolled`: a scroller the presenter shows, or the page, now
    /// stands at `(left, top)` CSS px, for `frame` (LLP 1051.000 D1).
    pub fn scrolled(&mut self, page: bool, view: u32, left: f64, top: f64) {
        if let Some(h) = self.host.as_mut() {
            h.runner_mut().scrolled((!page).then_some(view), left, top);
            if !h.runner_mut().intersect_due().is_empty() {
                if let Some(executor) = self.executor.as_deref() {
                    executor.notify();
                }
            }
        }
    }

    /// `exact_top_layer`: the presenter's open top-layer boxes, and where
    /// the frontmost modal presentation begins among them.
    pub fn top_layer(
        &mut self,
        frames: &[exact_runner::geometry::TopLayerFrame],
        modal: Option<usize>,
    ) {
        if let Some(h) = self.host.as_mut() {
            h.runner_mut().top_layer_frames(frames, modal);
        }
    }

    /// `exact_canvas_held`: a 2D canvas's replay is behind, or caught up.
    pub fn canvas_held(&mut self, view: u32, held: bool) {
        if let Some(h) = self.host.as_mut() {
            h.canvas_held(view, held);
        }
    }

    /// A host this bridge boots: inline geometry, Canvas 2D text measurement
    /// (LLP 1056 D8), and where its draws run (LLP 1072 §8.5).
    pub(super) fn presenter_hooks(&self, host: &mut crate::host::Host<D>, hooks: &super::Hooks) {
        host.runner_mut()
            .set_inline_geometry(self.inline_geometry.clone());
        if let Some(f) = hooks.canvas_text {
            host.set_canvas_text(f, hooks.ctx);
        }
        host.set_canvas_deferred(self.canvas_deferred);
        host.runner_mut().use_native_timers(self.native_timers);
        host.runner_mut()
            .hold_waits(exact_runner::WaitKinds::from_bits(self.wait_kinds));
        host.runner_mut()
            .set_live_clock(self.live_clock.map(super::preferences::sampler));
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
        x: &dyn crate::executor::Io,
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
        if let Some(x) = self.executor.as_deref() {
            x.notify();
        }
        0
    }

    /// How a presence check the host made ended (`exact_presence`, LLP
    /// 1133.010 D13): the value goes to the waiting call alone.
    pub fn presence(&mut self, len: usize) -> u32 {
        if let Some(h) = self.host.as_mut() {
            h.runner_mut()
                .presence_reply(&self.input[..len.min(self.input.len())]);
        }
        if let Some(x) = self.executor.as_deref() {
            x.notify();
        }
        0
    }

    /// A button's face (`exact_press_face`, LLP 1069.011.000 D1; LLP 1069.011
    /// D2, D5), custom or native, as JSON in the output buffer:
    /// `{"button","title","symbol","raster","leading","label","fits","style",
    /// "ios","iosBefore26","macos","known"}` — `button` false (and the rest
    /// empty) for a node that is not a button, the symbol as the platform
    /// names it (an svg-only icon's "", its `symbolPath` and `symbolViewBox`
    /// beside it, and `symbolIcon`, its token's name, for the agent), the style's row of the `buttonStyles` table (`bordered` for a
    /// name not in it); with no `buttonStyle` written, `"style":null` and no
    /// platform column (`button::face_json`). Not a batch: nothing changes.
    pub fn press_face(&mut self, view: u32) -> u32 {
        let json = self.host.as_ref().map_or_else(
            || "{\"button\":false}".into(),
            |h| {
                let kernel = h.runner().kernel();
                let face = kernel.press_face(view);
                let rows = kernel.button_face_style(view);
                let style = kernel
                    .node(view)
                    .and_then(|n| n.props.str(exact_kernel::PropId::ButtonStyle));
                let icon = face
                    .as_ref()
                    .and_then(|f| f.symbol.as_deref())
                    .and_then(|s| crate::icon_path::icon_name(h.runner().plan(), s));
                crate::button::face_json_named(face.as_ref(), rows.as_ref(), style, icon)
            },
        );
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

    /// A radio's group (`exact_radio_group`, x2apps survey #2), as JSON in
    /// the output buffer: `{"group":[...],"next":id|null,"previous":id|null}`,
    /// the radios of its `name` in tree order (`Kernel::radio_group`, itself
    /// alone when it has none) and the enabled radio each arrow moves the
    /// check to (`Kernel::radio_step`). Not a batch: nothing changes.
    pub fn radio_group(&mut self, view: u32) -> u32 {
        let id = |v: Option<u32>| v.map_or("null".into(), |v| v.to_string());
        let json = match self.host.as_ref() {
            Some(h) => {
                let kernel = h.runner().kernel();
                let group: Vec<String> = kernel
                    .radio_group(view)
                    .iter()
                    .map(u32::to_string)
                    .collect();
                format!(
                    "{{\"group\":[{}],\"next\":{},\"previous\":{}}}",
                    group.join(","),
                    id(kernel.radio_step(view, true)),
                    id(kernel.radio_step(view, false))
                )
            }
            None => "{\"group\":[],\"next\":null,\"previous\":null}".into(),
        };
        self.output = json.into_bytes();
        self.output.len() as u32
    }

    /// A navigation root's tabs (`exact_root_tabs`; `Kernel::root_tabs`, LLP
    /// 1115.001 D11), as JSON in the output buffer: `{"tablist":view,
    /// "tabs":[view...],"panels":[view...],"unsaid":bool,"covered":bool}`, the tabs that
    /// name a panel and those panels in tab order, or `null` for a root with
    /// one stack. `unsaid`: the app left the bar's placement unsaid, by the
    /// web's rule for its own sidebar (`host/web/navigation.js` `unsaidBar`,
    /// LLP 1116.004 D1.1): the tablist is a direct child of the root, not
    /// vertical, in flow in the root's flex column, and itself a row. It is
    /// read here because the layout rows it asks of never cross to the
    /// presenter (`style::presenter_ignores`). `covered`: the tablist is
    /// covered whole, as the boot covers one a tab bar replaces
    /// (`closed.rs`). Not a batch: nothing changes.
    pub fn root_tabs(&mut self, view: u32) -> u32 {
        use exact_kernel::{Display, FlexDirection, PositionType, PropId};
        let list = |ids: &[u32]| ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let json = self.host.as_ref().and_then(|h| {
            let kernel = h.runner().kernel();
            let tabs = kernel.root_tabs(view)?;
            let (root, bar) = (kernel.node(view)?, kernel.node(tabs.tablist)?);
            let column = |d| matches!(d, FlexDirection::Column | FlexDirection::ColumnReverse);
            let unsaid = root.children().contains(&tabs.tablist)
                && bar.props.str(PropId::AccessibilityOrientation) != Some("vertical")
                && root.style.display == Display::Flex
                && column(root.style.flex_direction)
                && matches!(
                    bar.layout_position(),
                    PositionType::Static | PositionType::Relative
                )
                && !column(bar.style.flex_direction);
            let arena = kernel.arena();
            let covered = arena.slot_of(tabs.tablist).and_then(|s| arena.cover(s))
                == Some(exact_kernel::HostCover::Whole);
            Some(format!(
                "{{\"tablist\":{},\"tabs\":[{}],\"panels\":[{}],\"unsaid\":{unsaid},\"covered\":{covered}}}",
                tabs.tablist,
                list(&tabs.tabs),
                list(&tabs.panels)
            ))
        });
        self.output = json.unwrap_or_else(|| "null".into()).into_bytes();
        self.output.len() as u32
    }

    /// Whether the stack whose root route is `view` splits (`exact_stack_split`;
    /// `Runner::stack_split`, LLP 1115.002 D4), as JSON in the output buffer:
    /// `{"list":view|null,"width":number|null,"rows":[[route,row]...],
    /// "locations":[[route,url]...],"pressWrites":bool|null}`, the list whose
    /// row pushes, the list column's width where `navigationSplit` names one,
    /// each route above the root that is a row's detail with that row, each
    /// route above the root with its location, and whether the press of
    /// `pressed` (a view being pressed, or 0) writes the router; `null` for a
    /// stack that pushes. Not a batch: nothing changes.
    pub fn stack_split(&mut self, view: u32, retained: u32, pressed: u32) -> u32 {
        let json = self
            .host
            .as_ref()
            .and_then(|h| {
                let runner = h.runner();
                let writes = (pressed != 0)
                    .then(|| runner.press_writes_router(pressed))
                    .flatten();
                runner
                    .stack_split(view, (retained != 0).then_some(retained))
                    .map(|split| (split, writes))
            })
            .map(|(split, writes)| {
                let rows: Vec<String> = split
                    .rows
                    .iter()
                    .map(|(route, row)| format!("[{route},{row}]"))
                    .collect();
                let locations: Vec<String> = split
                    .locations
                    .iter()
                    .map(|(route, url)| {
                        format!("[{route},{}]", serde_json::Value::from(url.as_str()))
                    })
                    .collect();
                format!(
                    "{{\"list\":{},\"width\":{},\"rows\":[{}],\"locations\":[{}],\"pressWrites\":{}}}",
                    split.list.map_or("null".into(), |v| v.to_string()),
                    split.width.map_or("null".into(), |w| w.to_string()),
                    rows.join(","),
                    locations.join(","),
                    writes.map_or("null".into(), |w| w.to_string())
                )
            });
        self.output = json.unwrap_or_else(|| "null".into()).into_bytes();
        self.output.len() as u32
    }

    /// A grouped list's sections and rows (`exact_grouped_list`, LLP 1084
    /// D4), as JSON in the output buffer: `{"style","sections":[{"view",
    /// "header","footer","headerLook","footerLook","card","spaceAbove","rows":[{"view","custom","symbol",("symbolPath","symbolViewBox": an
    /// svg-only icon's, `icon_path::push_json`),"title",
    /// "secondary","subtitle","accessory","target","pressable","action",
    /// "checked","destructive","disabled","authoredEdge","nextAuthoredTop","titleLook","secondaryLook"}]}],"spaceBelow"}`,
    /// `null` for a node that is not one. A look is `{"lines","ellipsis",
    /// "color","font"}`: a colour row's wire form and `{"size","weight",
    /// "italic","family","numeric"}`, each `null` where the author said nothing.
    /// `accessory` is `none`, `disclosure`, `checkmark`, `toggle`, `detail`
    /// or `radio`; `target` the toggle's control, the detail's button or the
    /// radio, whose bound state is `checked`. Not a batch: nothing changes.
    pub fn grouped_list(&mut self, view: u32) -> u32 {
        use exact_kernel::Accessory;
        let quote = exact_runner::agent::quote;
        let opt = |v: &Option<String>, json: &mut String| match v {
            Some(t) => quote(t, json),
            None => json.push_str("null"),
        };
        // How a text is set where its author said so (LLP 1116.003 D7): its
        // colour as a style row crosses, its font as its computed rows.
        let look = |key: &str, l: &exact_kernel::TextLook, json: &mut String| {
            json.push_str(key);
            json.push_str(&format!(
                "{{\"lines\":{},\"ellipsis\":{},\"color\":",
                l.lines, l.ellipsis
            ));
            match l.color {
                Some(c) => crate::style::push_color_value(json, c),
                None => json.push_str("null"),
            }
            json.push_str(",\"font\":");
            match l.font {
                Some(f) => {
                    let [size, weight, italic, family, numeric] = exact_kernel::TextFont::SAID;
                    let part = |bit: u8, value: String| {
                        if f.said & bit != 0 {
                            value
                        } else {
                            "null".into()
                        }
                    };
                    json.push_str(&format!(
                        "{{\"size\":{},\"weight\":{},\"italic\":{},\"family\":{},\"numeric\":{}}}",
                        part(size, f.size.to_string()),
                        part(weight, f.weight.to_string()),
                        part(italic, f.italic.to_string()),
                        part(family, f.family.to_string()),
                        part(numeric, f.numeric.to_string())
                    ));
                }
                None => json.push_str("null"),
            }
            json.push('}');
        };
        let list = self
            .host
            .as_ref()
            .and_then(|h| h.runner().kernel().grouped_list(view));
        let mut json = String::new();
        match list {
            None => json.push_str("null"),
            Some(list) => {
                json.push_str("{\"style\":");
                quote(&list.style, &mut json);
                json.push_str(",\"sections\":[");
                for (i, s) in list.sections.iter().enumerate() {
                    json.push_str(if i == 0 { "{" } else { ",{" });
                    json.push_str(&format!("\"view\":{},\"header\":", s.view));
                    opt(&s.header, &mut json);
                    json.push_str(",\"footer\":");
                    opt(&s.footer, &mut json);
                    look(",\"headerLook\":", &s.header_look, &mut json);
                    look(",\"footerLook\":", &s.footer_look, &mut json);
                    json.push_str(&format!(",\"card\":{}", s.card));
                    json.push_str(&format!(
                        ",\"spaceAbove\":{}",
                        s.space_above.map_or("null".into(), |v| v.to_string())
                    ));
                    json.push_str(",\"rows\":[");
                    for (j, r) in s.rows.iter().enumerate() {
                        json.push_str(if j == 0 { "{" } else { ",{" });
                        json.push_str(&format!(
                            "\"view\":{},\"custom\":{},\"symbol\":",
                            r.view, r.custom
                        ));
                        opt(&r.symbol, &mut json);
                        // An svg-only icon's path (LLP 1121 D10, stage 9 A2).
                        crate::icon_path::push_json(r.icon.as_deref(), &mut json);
                        json.push_str(",\"title\":");
                        opt(&r.title, &mut json);
                        json.push_str(",\"secondary\":");
                        opt(&r.secondary, &mut json);
                        let (accessory, target, checked) = match r.accessory {
                            Accessory::None => ("none", None, false),
                            Accessory::Disclosure => ("disclosure", None, false),
                            Accessory::Checkmark => ("checkmark", None, false),
                            Accessory::Toggle(id) => ("toggle", Some(id), false),
                            Accessory::Detail(id) => ("detail", Some(id), false),
                            Accessory::Radio(id, on) => ("radio", Some(id), on),
                        };
                        json.push_str(&format!(
                            ",\"subtitle\":{},\"accessory\":\"{accessory}\",\"target\":{},\"checked\":{checked},\"pressable\":{},\"action\":{},\"destructive\":{},\"disabled\":{},\"authoredEdge\":{},\"nextAuthoredTop\":{}",
                            r.subtitle,
                            target.map_or("null".into(), |t| t.to_string()),
                            r.pressable,
                            r.action,
                            r.destructive,
                            r.disabled,
                            r.authored_edge,
                            r.next_authored_top
                        ));
                        look(",\"titleLook\":", &r.title_look, &mut json);
                        look(",\"secondaryLook\":", &r.secondary_look, &mut json);
                        json.push('}');
                    }
                    json.push_str("]}");
                }
                json.push_str(&format!(
                    "],\"spaceBelow\":{}}}",
                    list.space_below.map_or("null".into(), |v| v.to_string())
                ));
            }
        }
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

    /// What the presenter applied of the orientation lock (`exact_orientation_applied`).
    pub fn orientation_applied(&mut self, len: usize, conflict: bool) -> u32 {
        let applied =
            String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        if let Some(h) = self.host.as_mut() {
            h.runner_mut().set_orientation_applied(applied, conflict);
        }
        0
    }
}

/// The host's answer to a package's call back: `reply(ctx, answer, len)`.
pub type InvokeReply = extern "C" fn(*mut std::ffi::c_void, *const u8, u32);
