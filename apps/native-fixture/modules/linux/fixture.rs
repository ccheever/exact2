// The native-module fixture's hatches on a painting host (LLP
// 1075.003.000.001 §8 stage 4): Linux, and Windows over the same presenter.
// What the web's page module does (`modules/web/index.js`), with what a
// painter has in place of a platform object: every call counted where the
// smoke reads it; the badge's line, snapshot and span; the `feed`, `presser`
// and `clock` hatches' acts and frame clock; the app and window scopes; and,
// for this kind of host, an overlay on each row's dot and on the detail
// list, one the badge draws and clears on the clock alone, and the input
// observed on the badge, the pressed button and the fed field.
//
// The app's Linux crate includes this file in a module of its own, beside
// the generated `HatchKey` (the words `app.json` gives this platform).
use exact_linux::hatches::{App, Context, Element, Hatches, Input, Phase, Span, Ticket, Window};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Fixture {
    scheme: String,
    /// The window as last told: whether it is this session's, and its size.
    window: Option<(bool, f32, f32)>,
    /// Each badge's span, from its mount to its end.
    shown: BTreeMap<u32, Span>,
    /// The words each node was last told with. On a painting host `changed`
    /// also says a box's size moved, and the acts below answer words only.
    told: BTreeMap<u32, BTreeMap<String, String>>,
    clock: Option<Ticket>,
    ticks: u32,
    agreed: u32,
    /// Input observed, by node.
    heard: BTreeMap<u32, u32>,
}

pub type ExactHatches = Fixture;

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Fixture {
    fn publish_scopes(&self, context: &Context<'_, Self>, moment: &str) {
        let d = context.diagnostics();
        d.count(&format!("scope.{moment}"), 1);
        let (exclusive, width, height) = self.window.unwrap_or_default();
        d.publish(
            "scopes",
            &format!(
                "{{\"scheme\":{},\"exclusive\":{exclusive},\"hasWindow\":false,\"frame\":[{width},{height}],\"last\":{}}}",
                quoted(&self.scheme),
                quoted(moment)
            ),
        );
    }

    /// The input that lands on `element`, counted and published under its
    /// word: what it was, and whether Exact's own dispatch did anything.
    fn observe(element: &Element, context: &mut Context<'_, Self>) {
        let node = element.clone();
        context.observe(element, move |me, input, _| {
            let d = node.diagnostics();
            let count = me.heard.entry(node.node()).or_default();
            *count += 1;
            match input {
                Input::Pointer { phase, handled, local, .. } => {
                    let phase = match phase {
                        Phase::Down => "down",
                        Phase::Move => "move",
                        Phase::Up => "up",
                        Phase::Cancel => "cancel",
                    };
                    d.count(&format!("observed.{phase}"), 1);
                    d.publish(
                        "observed",
                        &format!(
                            "{{\"phase\":\"{phase}\",\"handled\":{handled},\"inside\":{},\"heard\":{count}}}",
                            local.0 >= 0.0 && local.1 >= 0.0
                        ),
                    );
                }
                Input::Key { key, down, handled, .. } => {
                    d.count(if *down { "observed.keydown" } else { "observed.keyup" }, 1);
                    // The key's name is the fixture's to publish; a field's text is not.
                    d.publish(
                        "key",
                        &format!("{{\"key\":{},\"down\":{down},\"handled\":{handled}}}", quoted(key)),
                    );
                }
            }
        });
    }
}

impl Hatches for Fixture {
    // The app and window scopes (§2.1): each moment counted and published.
    fn app(&mut self, app: &App, context: &mut Context<'_, Self>) {
        self.scheme = app.prefers_color_scheme.to_owned();
        context.diagnostics().publish(
            "app",
            &format!(
                "{{\"processOwner\":{},\"hasApplication\":false,\"visibilityState\":{},\"onLine\":{}}}",
                app.process_owner,
                quoted(app.visibility_state),
                app.on_line
            ),
        );
        self.publish_scopes(
            context,
            if app.is_new {
                "app-built"
            } else {
                "app-changed"
            },
        );
    }

    fn app_ended(&mut self, _: &App, context: &mut Context<'_, Self>) {
        self.publish_scopes(context, "app-ended");
    }

    fn window(&mut self, window: &Window, context: &mut Context<'_, Self>) {
        self.window = Some((window.exclusive, window.frame.width, window.frame.height));
        self.publish_scopes(
            context,
            if window.is_new {
                "window-built"
            } else {
                "window-changed"
            },
        );
    }

    fn window_ended(&mut self, _: &Window, context: &mut Context<'_, Self>) {
        self.publish_scopes(context, "window-ended");
    }

    fn element(&mut self, element: &Element, context: &mut Context<'_, Self>) {
        let moment = if element.is_new() { "built" } else { "changed" };
        let key = HatchKey::of(element.hatch());
        // The smoke's stand-in for a crash in hatch code (§4.4): the run
        // ends inside the badge's hatch, and the next launch says so.
        if key == Some(HatchKey::Badge)
            && std::env::var("EXACT_FIXTURE_DIE").as_deref() == Ok("badge")
        {
            std::process::abort();
        }
        let d = element.diagnostics();
        d.count(moment, 1);
        let words = element.words();
        let before = self.told.insert(element.node(), words.clone());
        let reworded = before.is_some_and(|before| before != words);
        let word = |name: &str| element.data(name).unwrap_or_default();
        match key {
            // What a hatch asks of an authored node (§2.5), each queued:
            // `feed` replaces its field's value, `presser` clicks its own
            // button once when armed and on every change while looping.
            Some(HatchKey::Feed) => {
                if element.is_new() {
                    Self::observe(element, context);
                }
                if reworded && !word("feed").is_empty() {
                    element.input(&word("feed"));
                }
            }
            Some(HatchKey::Presser) => {
                if element.is_new() {
                    Self::observe(element, context);
                }
                if reworded && (word("loop") != "off" || word("armed") == "true") {
                    element.click();
                }
            }
            // The frame clock (§2.4), as the web's and the Swift module's:
            // a ticket while the node says to run. Each tick counts itself
            // and whether the node's `data-frames`, which a frame task
            // advances, is the tick's own number; the first chains
            // `after`s; the third presses the node 65 times.
            Some(HatchKey::Clock) => {
                let run = word("run") == "true";
                if run && self.clock.is_none() {
                    let node = element.clone();
                    (self.ticks, self.agreed) = (0, 0);
                    self.clock = Some(context.frames(move |me, frame, context| {
                        me.ticks += 1;
                        if node.data("frames") == Some(me.ticks.to_string()) {
                            me.agreed += 1;
                        }
                        let d = context.diagnostics();
                        d.count("clock.ticks", 1);
                        if me.ticks == 1 {
                            context.after(0.0, |_, context| {
                                context.diagnostics().count("clock.after0", 1);
                                context.after(20.0, |_, context| {
                                    context.diagnostics().count("clock.after20", 1)
                                });
                            });
                            context
                                .after(30.0, |_, context| {
                                    context.diagnostics().count("clock.stopped", 1)
                                })
                                .stop();
                        }
                        if me.ticks == 3 {
                            for _ in 0..65 {
                                node.click();
                            }
                        }
                        d.publish(
                            "clock",
                            &format!(
                                "{{\"ticks\":{},\"agreed\":{},\"now\":{}}}",
                                me.ticks, me.agreed, frame.now
                            ),
                        );
                    }));
                }
                if !run {
                    if let Some(ticket) = self.clock.take() {
                        ticket.stop();
                    }
                }
                if reworded && word("presses") == "65" {
                    element.click();
                }
            }
            // The badge says what it did (§3.2): a line, a snapshot of its
            // tone and a span from its mount to its end. Its change draws an
            // overlay a tenth of a second on and clears it a tenth after, on
            // the session clock alone: the Contract app is idle for both.
            Some(HatchKey::Badge) => {
                let tone = word("tone");
                d.log(&format!("{moment}, tone {tone}"));
                d.publish(
                    "tone",
                    &format!(
                        "{{\"tone\":{},\"moment\":{}}}",
                        quoted(&tone),
                        quoted(moment)
                    ),
                );
                if element.is_new() {
                    self.shown.insert(element.node(), d.begin("shown"));
                    Self::observe(element, context);
                } else if reworded {
                    let (drawn, cleared) = (element.clone(), element.clone());
                    context.after(100.0, move |_, _| {
                        drawn.overlay().draw(|c, w, h| {
                            c.set_fill_style_str("#00c853");
                            c.fill_rect(0.0, 0.0, w, h);
                        });
                        drawn.diagnostics().count("overlay.drawn", 1);
                    });
                    context.after(200.0, move |_, _| {
                        cleared.overlay().clear();
                        cleared.diagnostics().count("overlay.cleared", 1);
                    });
                }
            }
            // Each row's dot under a recording of its own: one fill past
            // every edge, which the host clips to the dot's round box.
            Some(HatchKey::Dot) => {
                element.overlay().draw(|c, w, h| {
                    c.set_fill_style_str("#ff3b30");
                    c.fill_rect(-8.0, -8.0, w + 16.0, h + 16.0);
                });
            }
            // The detail list's box follows the window: a frame along its
            // edges, recorded again at each new size.
            Some(HatchKey::DetailList) => {
                let drew = element.overlay().draw(|c, w, h| {
                    c.set_stroke_style_str("#ff00ff");
                    c.set_line_width(4.0);
                    c.stroke_rect(2.0, 2.0, w - 4.0, h - 4.0);
                });
                d.count(
                    if drew {
                        "overlay.drawn"
                    } else {
                        "overlay.refused"
                    },
                    1,
                );
                let frame = element.frame();
                d.publish("frame", &format!("[{},{}]", frame.width, frame.height));
            }
            None => {}
        }
    }

    fn element_ended(&mut self, element: &Element, _: &mut Context<'_, Self>) {
        element.diagnostics().count("ended", 1);
        if let Some(span) = self.shown.remove(&element.node()) {
            span.end();
        }
        self.told.remove(&element.node());
    }
}
