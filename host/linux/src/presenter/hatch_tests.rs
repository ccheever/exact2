//! The hatches through a booted plan (LLP 1075.003.000.001 §8 stage 4, §9):
//! the overlay protocol, observed input, the frame clock at the virtual
//! display's instants, the act queue, the scopes, and when each moment runs.
use super::super::*;
use crate::hatches::{install, App, Context, Element, Hatches, Input, Phase, Ticket, Window};
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {
    /// What the test's hatches did, in order: a test's thread is its own.
    static NOTES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}
fn note(line: impl Into<String>) {
    NOTES.with(|notes| notes.borrow_mut().push(line.into()));
}
fn notes() -> Vec<String> {
    NOTES.with(|notes| std::mem::take(&mut *notes.borrow_mut()))
}

fn boot<H: Hatches>(source: &str, words: &'static [&'static str]) -> Presenter<()> {
    let plan = contract::compile(source).unwrap_or_else(|e| panic!("{e:?}"));
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.set_hatches(Some(install::<H>(words)));
    notes();
    p
}
fn first_pixel(p: &mut Presenter<()>) {
    let _ = p.frame();
    p.first_pixel();
}
fn id(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn agent(p: &mut Presenter<()>, request: Value) -> Value {
    serde_json::from_str(&crate::agent::handle(p, &request.to_string())).unwrap()
}
fn tap(p: &mut Presenter<()>, test_id: &str) -> Value {
    let view = id(p, test_id);
    let reply = agent(p, json!({"op": "tap", "id": view}));
    assert!(reply.get("error").is_none(), "tap {test_id}: {reply}");
    reply
}
fn state(p: &mut Presenter<()>) -> Value {
    agent(p, json!({"op": "state"}))
}
fn journal(p: &mut Presenter<()>) -> Vec<String> {
    let logs = agent(p, json!({"op": "logs", "since": 0}));
    let lines = logs["lines"].as_array().unwrap().iter();
    lines.map(|l| l.as_str().unwrap().to_owned()).collect()
}
fn said(p: &mut Presenter<()>, part: &str) -> Vec<String> {
    let lines = journal(p).into_iter();
    lines.filter(|l| l.contains(part)).collect()
}
fn pixel(p: &mut Presenter<()>, x: u32, y: u32) -> [u8; 3] {
    let frame = p.frame();
    let c = frame.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue()]
}
const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

// The box: x 20..80 (or ..140 when wide), y 110..150, corners rounded by 20.
const PAINT: &str = r##"component App
  state wide = false
  state shown = true
  state n = 0
  action widen
    wide = not wide
  action hide
    shown = false
  action bump
    n = n + 1
  view
    column width=300 height=300 background-color="#ffffff"
      button "Widen" testId="widen" press=widen height=30
      button "Hide" testId="hide" press=hide height=30
      button "Bump" testId="bump" press=bump height=30
      when shown
        column hatch="paint" data-n=toString(n) testId="box" width=(wide ? 120 : 60) height=40 margin-left=20 margin-top=20 border-radius=20 background-color="#0000ff"
"##;

/// Every moment records again: one half of the box, past every edge, the
/// left in red on odd recordings and the right in green on even ones.
#[derive(Default)]
struct Paints {
    drawn: u32,
}
impl Hatches for Paints {
    fn element(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        let (moment, f) = (if e.is_new() { "built" } else { "changed" }, e.frame());
        let n = e.data("n").unwrap_or_default();
        note(format!(
            "{moment} {} n={n} {}x{}",
            e.hatch(),
            f.width,
            f.height
        ));
        self.drawn += 1;
        let odd = self.drawn % 2 == 1;
        let drew = e.overlay().draw(|c, w, h| {
            c.set_fill_style_str(if odd { "#ff0000" } else { "#00ff00" });
            let x = if odd { -50.0 } else { w / 2.0 };
            c.fill_rect(x, -50.0, w / 2.0 + 50.0, h + 100.0);
        });
        assert!(drew);
    }
    fn element_ended(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        note(format!("ended {} live={}", e.hatch(), e.is_live()));
        assert!(
            !e.overlay().draw(|_, _, _| {}),
            "an ended node has no overlay"
        );
    }
}

#[test]
fn an_overlay_replaces_the_last_whole_is_clipped_to_the_box_and_dropped_at_a_new_size() {
    let mut p = boot::<Paints>(PAINT, &["paint"]);
    // The first frame is the hatch-less app: nothing of the module is made.
    assert_eq!(pixel(&mut p, 30, 130), BLUE);
    assert!(notes().is_empty() && !p.hatches.connected());
    first_pixel(&mut p);
    assert_eq!(notes(), ["built paint n=0 60x40"]);
    assert!(p.dirty(), "a publish marks the picture stale");
    // Over the node's own paint, and never outside its border box: the fill
    // ran 50 past every edge, and the box's corners are round.
    assert_eq!(pixel(&mut p, 30, 130), RED);
    assert_eq!(
        pixel(&mut p, 70, 130),
        BLUE,
        "the node's own paint shows where nothing was drawn"
    );
    for outside in [(10, 130), (30, 105), (30, 155), (21, 111)] {
        assert_eq!(pixel(&mut p, outside.0, outside.1), WHITE, "{outside:?}");
    }
    // A recording replaces the last one whole: nothing of the red is kept.
    tap(&mut p, "bump");
    assert_eq!(notes(), ["changed paint n=1 60x40"]);
    assert_eq!(pixel(&mut p, 30, 130), BLUE);
    assert_eq!(pixel(&mut p, 70, 130), GREEN);
    // A new size drops the recording, and the hatch hears `changed` with
    // the new frame and records again, for that size.
    tap(&mut p, "widen");
    assert_eq!(notes(), ["changed paint n=1 120x40"]);
    assert_eq!(pixel(&mut p, 70, 130), RED, "the left half of 120");
    assert_eq!(pixel(&mut p, 110, 130), BLUE);
    let overlay = state(&mut p)["hatches"]["words"]["paint"]["overlay"].clone();
    assert_eq!(overlay, json!({"shown": 1, "published": 3, "dropped": 1}));
    // The node leaves: its hatch hears `ended`, the handle no longer live.
    tap(&mut p, "hide");
    assert_eq!(notes(), ["ended paint live=false"]);
    let word = state(&mut p)["hatches"]["words"]["paint"].clone();
    assert_eq!(word["live"], 0);
    assert_eq!(word["calls"], json!({"built": 1, "changed": 2, "ended": 1}));
    assert_eq!(word["overlay"]["shown"], 0);
}

/// Draws once, when built: what a new size leaves of it.
#[derive(Default)]
struct Once;
impl Hatches for Once {
    fn element(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        note(format!("{} {}", e.is_new(), e.frame().width));
        if e.is_new() {
            e.overlay().draw(|c, w, h| {
                c.set_fill_style_str("#ff0000");
                c.fill_rect(0.0, 0.0, w, h);
            });
        }
    }
}

#[test]
fn a_recording_is_never_stretched_to_a_new_size() {
    let mut p = boot::<Once>(PAINT, &["paint"]);
    first_pixel(&mut p);
    assert_eq!(pixel(&mut p, 50, 130), RED);
    tap(&mut p, "widen");
    assert_eq!(notes(), ["true 60", "false 120"]);
    assert_eq!(pixel(&mut p, 50, 130), BLUE, "dropped, not stretched");
    assert_eq!(pixel(&mut p, 110, 130), BLUE);
    let overlay = state(&mut p)["hatches"]["words"]["paint"]["overlay"].clone();
    assert_eq!(overlay, json!({"shown": 0, "published": 1, "dropped": 1}));
}

/// Past each bound, by its node's `data-n`; the first recording is good.
#[derive(Default)]
struct Greedy;
impl Hatches for Greedy {
    fn element(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        let n = e.data("n").unwrap_or_default();
        let drew = e.overlay().draw(|c, w, h| {
            c.set_fill_style_str("#ff0000");
            match n.as_str() {
                "1" => (0..4100).for_each(|i| c.fill_rect(f64::from(i % 7), 0.0, 1.0, 1.0)),
                "2" => (0..40).for_each(|_| c.set_line_dash(&[1.5; 1000]).unwrap()),
                _ => c.fill_rect(0.0, 0.0, w, h),
            }
        });
        note(format!("n={n} drew={drew}"));
    }
}

#[test]
fn a_recording_past_its_bounds_is_refused_by_name_and_the_last_good_one_stays() {
    let mut p = boot::<Greedy>(PAINT, &["paint"]);
    first_pixel(&mut p);
    assert_eq!(pixel(&mut p, 50, 130), RED);
    tap(&mut p, "bump");
    tap(&mut p, "bump");
    assert_eq!(
        notes(),
        ["n=0 drew=true", "n=1 drew=false", "n=2 drew=false"]
    );
    assert_eq!(pixel(&mut p, 50, 130), RED, "the last good recording stays");
    let refused = said(&mut p, "overlay refused");
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert!(
        refused[0].contains("hatch element paint #") && refused[0].ends_with("ops is over 4,096")
    );
    assert!(refused[1].ends_with("bytes is over 256 KB"), "{refused:?}");
    let hatches = state(&mut p)["hatches"].clone();
    assert_eq!(hatches["overlaysRefused"], 2);
    assert_eq!(hatches["words"]["paint"]["overlay"]["published"], 1);
}

/// A hatch that draws and clears on the clock alone.
#[derive(Default)]
struct Blink;
impl Hatches for Blink {
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        if !e.is_new() {
            return;
        }
        let (drawn, cleared) = (e.clone(), e.clone());
        context.after(100.0, move |_, context| {
            note(format!("draw at {}", context.now()));
            drawn.overlay().draw(|c, w, h| {
                c.set_fill_style_str("#00ff00");
                c.fill_rect(0.0, 0.0, w, h);
            });
        });
        context.after(200.0, move |_, _| cleared.overlay().clear());
    }
}

#[test]
fn a_publish_with_the_app_idle_wakes_the_loop_and_shows_in_its_next_frame() {
    let mut p = boot::<Blink>(PAINT, &["paint"]);
    first_pixel(&mut p);
    assert_eq!(pixel(&mut p, 50, 130), BLUE);
    assert!(!p.dirty());
    // The wall's loop: its next timer is the hatch's, though the plan has none.
    assert_eq!(p.host().timer_due_ms(), None);
    assert_eq!(p.timer_due_ms(), Some(100.0));
    let epoch = p.host().kernel().epoch();
    assert!(p.advance(100.0).is_none());
    assert_eq!(notes(), ["draw at 100"]);
    assert!(p.dirty(), "the publish asks for a frame");
    assert_eq!(pixel(&mut p, 50, 130), GREEN);
    assert!(!p.dirty());
    assert_eq!(p.timer_due_ms(), Some(200.0));
    assert!(p.advance(200.0).is_none());
    assert!(p.dirty(), "so does a clear");
    assert_eq!(pixel(&mut p, 50, 130), BLUE);
    assert_eq!(
        p.host().kernel().epoch(),
        epoch,
        "the Contract app was idle throughout"
    );
    assert_eq!(p.timer_due_ms(), None);
}

const INPUT: &str = r##"component App
  state presses = 0
  state text = ""
  action pressed
    presses = presses + 1
  action edit(value)
    text = value
  view
    column hatch="outer" testId="outer" width=300 height=300 background-color="#ffffff"
      button "Press" hatch="inner" testId="press" press=pressed width=100 height=40
      column hatch="plain" testId="plain" width=100 height=40 background-color="#eeeeee"
      input hatch="field" value=text input=edit testId="field" width=200 height=32
"##;

/// Observes every node it is given; the `inner` one twice.
#[derive(Default)]
struct Ears {
    second: Option<Ticket>,
}
impl Hatches for Ears {
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        if !e.is_new() {
            return;
        }
        context.observe(e, hear(e.hatch().to_owned()));
        if e.hatch() == "inner" {
            self.second = Some(context.observe(e, hear("inner-again".into())));
        }
        // The module stops its own second observer when its node says to.
        if e.hatch() == "plain" {
            let plain = e.clone();
            context.observe(e, move |me, input, _| {
                if !matches!(
                    input,
                    Input::Pointer {
                        phase: Phase::Up,
                        ..
                    }
                ) || !plain.is_live()
                {
                    return;
                }
                match me.second.take() {
                    Some(second) => {
                        second.stop();
                        note("stopped inner-again")
                    }
                    None => note("plain again"),
                }
            });
        }
    }
}

/// An observer that notes what it hears: the input, whether Exact's dispatch
/// did anything, and whether the point is in a 100×40 box at the node's origin.
fn hear(name: String) -> impl FnMut(&mut Ears, &Input, &mut Context<'_, Ears>) + 'static {
    move |_, input, _| match input {
        Input::Pointer {
            phase,
            handled,
            local,
            ..
        } => {
            let inside = local.0 >= 0.0 && local.1 >= 0.0 && local.0 < 100.0 && local.1 < 40.0;
            note(format!(
                "{name} {phase:?} handled={handled} inside={inside}"
            ))
        }
        Input::Key {
            key, down, handled, ..
        } => note(format!("{name} key {key} down={down} handled={handled}")),
    }
}

#[test]
fn input_is_observed_after_exacts_dispatch_with_handled_and_captured_until_it_lifts() {
    let mut p = boot::<Ears>(INPUT, &["outer", "inner", "plain", "field"]);
    first_pixel(&mut p);
    // Down on the button: innermost first along the hit chain, each node's
    // observers in registration order. Nothing has run for it yet.
    p.pointer_down(50., 20., 0.).unwrap();
    assert_eq!(
        notes(),
        [
            "inner Down handled=false inside=true",
            "inner-again Down handled=false inside=true",
            "outer Down handled=false inside=true"
        ]
    );
    // It leaves the box, and stays with the boxes it went down in.
    p.pointer_move(50., 60., 1.).unwrap();
    assert_eq!(
        notes(),
        [
            "inner Move handled=false inside=false",
            "inner-again Move handled=false inside=false",
            "outer Move handled=false inside=false"
        ]
    );
    // Lifted out there, it pressed nothing; the boxes of its down hear so.
    p.pointer_up(50., 60., 2.).unwrap();
    assert_eq!(state(&mut p)["slots"]["presses"], 0);
    assert_eq!(
        notes(),
        [
            "inner Up handled=false inside=false",
            "inner-again Up handled=false inside=false",
            "outer Up handled=false inside=false"
        ]
    );
    // Down and up on the button: Exact's dispatch pressed it first.
    p.pointer_down(50., 20., 3.).unwrap();
    notes();
    p.pointer_up(50., 20., 3.).unwrap();
    assert_eq!(state(&mut p)["slots"]["presses"], 1);
    assert_eq!(
        notes(),
        [
            "inner Up handled=true inside=true",
            "inner-again Up handled=true inside=true",
            "outer Up handled=true inside=true"
        ]
    );
    // A free pointer's move is heard where it is; the capture ended with the up.
    p.pointer_move(50., 60., 4.).unwrap();
    assert_eq!(
        notes(),
        [
            "plain Move handled=false inside=true",
            "outer Move handled=false inside=false"
        ]
    );
    // A cancel is told to what the pointer went down in.
    p.pointer_down(50., 60., 5.).unwrap();
    p.pointer_cancel(6.).unwrap();
    let heard = notes();
    assert_eq!(
        heard[2..],
        [
            "plain Cancel handled=false inside=true",
            "outer Cancel handled=false inside=false"
        ],
        "{heard:?}"
    );
    // The agent's tap is a press at a point: a down and an up are heard,
    // the up with what the press did. A box with no handler has done nothing.
    tap(&mut p, "press");
    let heard = notes();
    assert_eq!(heard[0], "inner Down handled=false inside=true");
    assert_eq!(
        heard[3..],
        [
            "inner Up handled=true inside=true",
            "inner-again Up handled=true inside=true",
            "outer Up handled=true inside=true"
        ]
    );
    // A box with no handler: nothing ran for it. Its second observer, in
    // registration order after the first, stops `inner`'s second one.
    tap(&mut p, "plain");
    assert_eq!(
        notes(),
        [
            "plain Down handled=false inside=true",
            "outer Down handled=false inside=false",
            "plain Up handled=false inside=true",
            "stopped inner-again",
            "outer Up handled=false inside=false"
        ]
    );
    // A stopped observer hears no more.
    tap(&mut p, "press");
    let heard = notes();
    assert!(
        heard.len() == 4 && !heard.iter().any(|n| n.starts_with("inner-again")),
        "{heard:?}"
    );
    // Keys go to the focused node and its ancestors, innermost first; a node
    // with no focus inside it hears none.
    let field = id(&p, "field");
    p.type_text(field, "a").unwrap();
    notes();
    p.key_down("b", 10.);
    assert_eq!(state(&mut p)["slots"]["text"], "ab");
    assert_eq!(
        notes(),
        [
            "field key b down=true handled=true",
            "outer key b down=true handled=true"
        ]
    );
    p.key_up("b", "KeyB", 11.);
    p.key_down("F5", 12.);
    assert_eq!(
        notes(),
        [
            "field key b down=false handled=false",
            "outer key b down=false handled=false",
            "field key F5 down=true handled=false",
            "outer key F5 down=true handled=false"
        ]
    );
    // An observer claims nothing: what a press does is the same without one.
    let mut bare = boot::<Ears>(INPUT, &[]);
    first_pixel(&mut bare);
    bare.pointer_down(50., 20., 0.).unwrap();
    bare.pointer_up(50., 20., 1.).unwrap();
    assert_eq!(state(&mut bare)["slots"]["presses"], 1);
    assert!(notes().is_empty());
}

const CLOCK: &str = r##"component App
  state running = false
  state frames = 0
  state presses = 0
  action start
    running = true
  action halt
    running = false
  action countFrame
    frames = frames + 1
  action press
    presses = presses + 1
  task framer when running
    every(frame, countFrame)
  view
    column width=300 height=300
      button "Start" testId="start" press=start height=30
      button "Halt" testId="halt" press=halt height=30
      column hatch="clock" data-run=(running ? "true" : "false") data-frames=toString(frames) data-presses=toString(presses) press=press testId="clock" width=24 height=24
"##;

/// The fixture's clock hatch: a ticket while its node says to run.
#[derive(Default)]
struct Clocked {
    ticket: Option<Ticket>,
    ticks: u32,
    agreed: u32,
}
impl Hatches for Clocked {
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        let run = e.data("run").as_deref() == Some("true");
        if run && self.ticket.is_none() {
            let node = e.clone();
            self.ticket = Some(context.frames(move |me, frame, context| {
                me.ticks += 1;
                assert_eq!(frame.now, context.now());
                if node.data("frames") == Some(me.ticks.to_string()) {
                    me.agreed += 1;
                }
                note(format!(
                    "tick {} at {} seq {}",
                    me.ticks, frame.now, frame.seq
                ));
                let d = context.diagnostics();
                d.count("ticks", 1);
                d.publish(
                    "clock",
                    &format!("{{\"ticks\":{},\"agreed\":{}}}", me.ticks, me.agreed),
                );
                if me.ticks == 1 {
                    context.after(0.0, |_, context| {
                        note(format!("after0 at {}", context.now()));
                        context.after(20.0, |_, context| {
                            note(format!("after20 at {}", context.now()))
                        });
                    });
                    context.after(30.0, |_, _| note("stopped: never")).stop();
                }
                if me.ticks == 3 {
                    (0..65).for_each(|_| node.click());
                }
            }));
        }
        if !run {
            if let Some(ticket) = self.ticket.take() {
                ticket.stop();
            }
        }
        if !e.is_new() && e.data("presses").as_deref() == Some("65") {
            e.click();
        }
    }
}

fn clock_drive(steps: &[f64]) -> (Value, Vec<String>, Vec<String>) {
    let mut p = boot::<Clocked>(CLOCK, &["clock"]);
    first_pixel(&mut p);
    tap(&mut p, "start");
    for to in steps {
        let reply = agent(&mut p, json!({"op": "clock", "to": to}));
        assert_eq!(reply["clock"], *to, "{reply}");
        assert!(reply.get("error").is_none(), "{reply}");
    }
    (state(&mut p), journal(&mut p), notes())
}

#[test]
fn a_ticket_ticks_at_the_virtual_displays_instants_and_a_seek_is_its_steps() {
    let (whole, lines, heard) = clock_drive(&[1000.0]);
    let told = &whole["hatches"]["scopes"]["module"];
    assert_eq!(told["counters"]["ticks"], 60, "sixty instants in a second");
    // Tick k sees the count the frame task made at the same instant.
    assert_eq!(
        told["published"]["clock"],
        json!({"ticks": 60, "agreed": 60})
    );
    assert_eq!(whole["slots"]["frames"], 60);
    // Each at its own instant, `base + k·1000/60`, the product first.
    let ticks: Vec<&String> = heard.iter().filter(|n| n.starts_with("tick ")).collect();
    for (i, tick) in ticks.iter().enumerate() {
        let k = i as u32 + 1;
        let at = exact_runner::virtual_frame(0.0, k);
        assert!(
            tick.starts_with(&format!("tick {k} at {at} seq ")),
            "{tick}"
        );
    }
    // An `after` chained in a tick fires inside the seek, at the tick's
    // instant and twenty past it; one stopped there never fires.
    let first = exact_runner::virtual_frame(0.0, 1);
    let afters: Vec<&String> = heard.iter().filter(|n| !n.starts_with("tick ")).collect();
    assert_eq!(
        afters,
        [
            &format!("after0 at {first}"),
            &format!("after20 at {}", first + 20.0)
        ]
    );
    let at = |part: &str| heard.iter().position(|n| n.starts_with(part)).unwrap();
    assert!(at("tick 1 ") < at("after0") && at("after0") < at("tick 2 "));
    assert!(at("tick 2 ") < at("after20") && at("after20") < at("tick 3 "));
    // What the third tick asked, and the one their `changed` asked, all
    // land at that tick's instant: 66 presses, each journaled, one time.
    assert_eq!(whole["slots"]["presses"], 66);
    let acts: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains(": click (delivery: hatch)"))
        .collect();
    assert_eq!(acts.len(), 66);
    let third = format!("t={} ", exact_runner::virtual_frame(0.0, 3));
    assert!(acts.iter().all(|l| l.starts_with(&third)), "{:?}", acts[0]);
    assert_eq!(whole["hatches"]["inFlight"], 0);
    // One seek of a second is ten of a tenth: state, journal and calls alike.
    let tenths: Vec<f64> = (1..=10).map(|k| f64::from(k) * 100.0).collect();
    let (stepped, stepped_lines, stepped_heard) = clock_drive(&tenths);
    assert_eq!(stepped["slots"], whole["slots"]);
    assert_eq!(stepped["hatches"]["scopes"], whole["hatches"]["scopes"]);
    assert_eq!(stepped["hatches"]["words"], whole["hatches"]["words"]);
    assert_eq!(stepped_heard, heard);
    assert_eq!(stepped_lines, lines);
}

#[test]
fn a_stopped_ticket_ticks_no_more_and_perf_counts_the_live_ones() {
    let mut p = boot::<Clocked>(CLOCK, &["clock"]);
    first_pixel(&mut p);
    tap(&mut p, "start");
    assert_eq!(
        agent(&mut p, json!({"op": "perf", "hatches": true}))["tickets"],
        1
    );
    agent(&mut p, json!({"op": "clock", "to": 50.0}));
    tap(&mut p, "halt");
    agent(&mut p, json!({"op": "clock", "to": 500.0}));
    let perf = agent(&mut p, json!({"op": "perf", "hatches": true}));
    assert_eq!(perf["tickets"], 0);
    assert_eq!(perf["counters"]["module"]["ticks"], 3);
    // `perf hatches`: every call timed, by hatch and by site and moment,
    // with `perf`'s own tags; the frame clock's under `frames` and `after`.
    assert_eq!(perf["hatches"]["frames"]["calls"], 3);
    assert_eq!(perf["hatches"]["after"]["calls"], 2);
    assert!(
        perf["measuring"] == true && perf["seq"].is_u64() && perf["plan"].is_string(),
        "{perf}"
    );
    let rows = perf["calls"].as_array().unwrap();
    let built = rows
        .iter()
        .find(|c| c["hatch"] == "element clock" && c["moment"] == "built")
        .unwrap();
    assert!(built["calls"] == 1 && built["site"].is_u64() && built["ms"].as_f64() >= Some(0.0));
    // `perf <target>`: the hatched site's row names its hatch's calls.
    let clock = id(&p, "clock");
    let target = agent(&mut p, json!({"op": "perf", "target": clock}));
    let row = target["sites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["site"] == built["site"]);
    assert!(
        row.unwrap()["hatch"]["calls"].as_u64() >= Some(2),
        "{target}"
    );
    // A read changes nothing.
    assert_eq!(
        agent(&mut p, json!({"op": "perf", "hatches": true}))["calls"],
        perf["calls"]
    );
}

const STORM: &str = r##"component App
  state n = 0
  action bump
    n = n + 1
  view
    column width=300 height=300
      column hatch="storm" data-n=toString(n) press=bump testId="storm" width=24 height=24
"##;

/// More acts than a command may run: a click for every change, begun on the clock.
#[derive(Default)]
struct Storm;
impl Hatches for Storm {
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        if e.is_new() {
            let node = e.clone();
            context.after(10.0, move |_, _| node.click());
        } else {
            e.click();
        }
    }
}

/// More ticks than a command may run: 5,000 tickets.
#[derive(Default)]
struct Flood;
impl Hatches for Flood {
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        if e.is_new() {
            (0..5000).for_each(|_| drop(context.frames(|_, _, _| note("tick"))));
        }
    }
}

#[test]
fn a_command_runs_at_most_4096_ticks_and_says_so() {
    let mut p = boot::<Flood>(STORM, &["storm"]);
    first_pixel(&mut p);
    let reply = agent(&mut p, json!({"op": "clock", "to": 100.0}));
    assert_eq!(reply["error"], "clock: HatchFireLimit");
    assert_eq!(notes().len(), 4096);
    let first = exact_runner::virtual_frame(0.0, 1);
    assert_eq!(
        reply["clock"], first,
        "the seek stops at the instant that passed the cap"
    );
    let refused = said(&mut p, "refused advance");
    assert_eq!(
        refused,
        [format!(
            "t={first} hatch refused advance: HatchFireLimit (4096 in one command)"
        )]
    );
    // The next command's caps start at nothing.
    notes();
    agent(&mut p, json!({"op": "clock", "to": 100.0}));
    assert_eq!(notes().len(), 4096);
    // On the wall no cap applies: each presented frame ticks every ticket.
    let mut wall = boot::<Flood>(STORM, &["storm"]);
    first_pixel(&mut wall);
    assert!(wall.wants_frames() && wall.wants_display_frames() && wall.needs_animation_frame());
    assert!(wall.animation_frame(16.0).is_none());
    assert_eq!(notes().len(), 5000);
}

#[test]
fn a_command_runs_at_most_4096_acts_and_says_so() {
    let mut p = boot::<Storm>(STORM, &["storm"]);
    first_pixel(&mut p);
    let reply = agent(&mut p, json!({"op": "clock", "to": 100.0}));
    assert_eq!(reply["error"], "clock: HatchActLimit");
    assert_eq!(
        reply["clock"], 10.0,
        "the seek stops at the instant that passed the cap"
    );
    assert_eq!(
        state(&mut p)["slots"]["n"].as_u64(),
        Some(4096 + 1),
        "and the next command's turn ran one more"
    );
    let refused = said(&mut p, "refused advance");
    assert_eq!(
        refused,
        ["t=10 hatch refused advance: HatchActLimit (4096 in one command)"]
    );
}

const ASKS: &str = r##"component App
  state armed = false
  state looping = false
  state presses = 0
  state text = ""
  state secret = ""
  action arm
    armed = true
  action loop
    looping = true
  action stop
    looping = false
  action pressed
    presses = presses + 1
  action edit(value)
    text = value
  action hide(value)
    secret = value
  view
    column width=300 height=300
      button "Arm" testId="arm" press=arm height=30
      button "Loop" testId="loop" press=loop height=30
      button "Stop" testId="stop" press=stop height=30
      button "Pressed" hatch="presser" data-armed=(armed ? "true" : "false") data-loop=(looping ? toString(presses) : "off") press=pressed testId="pressed" height=30
      input hatch="feed" data-feed=(armed ? "Palo Alto, CA" : "") value=text input=edit maxlength=9 testId="fed" width=200 height=30
      input hatch="feed" data-feed=(armed ? "hunter2" : "") type="password" value=secret input=hide testId="secret" width=200 height=30
      input hatch="feed" data-feed=(armed ? "no" : "") value="locked" disabled=true testId="locked" width=200 height=30
      input hatch="feed" data-feed=(armed ? "no" : "") value="kept" readonly=true testId="readonly" width=200 height=30
      column hatch="feed" data-feed=(armed ? "no" : "") testId="notfield" width=200 height=10
"##;

/// The fixture's `feed` and `presser` hatches.
#[derive(Default)]
struct Asks;
impl Hatches for Asks {
    fn element(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        let word = |name: &str| e.data(name).unwrap_or_default();
        match e.hatch() {
            "feed" if !e.is_new() && !word("feed").is_empty() => e.input(&word("feed")),
            "presser" if !e.is_new() && (word("loop") != "off" || word("armed") == "true") => {
                e.click()
            }
            _ => {}
        }
    }
}

#[test]
fn acts_are_queued_run_between_turns_and_journaled_an_input_by_its_length() {
    let mut p = boot::<Asks>(ASKS, &["presser", "feed"]);
    first_pixel(&mut p);
    // The command that caused them answers with them queued: none ran
    // inside the hatch, nor inside the commit that called it.
    tap(&mut p, "arm");
    assert_eq!(p.hatch_in_flight(), 6);
    let before: Value = serde_json::from_str(&p.host().agent("{\"op\":\"state\"}")).unwrap();
    assert_eq!(
        (
            before["slots"]["presses"].clone(),
            before["slots"]["text"].clone()
        ),
        (json!(0), json!(""))
    );
    // The next command is the next turn: they run, each a commit.
    let s = state(&mut p);
    assert_eq!(s["hatches"]["inFlight"], 0);
    assert_eq!(s["slots"]["presses"], 1);
    assert_eq!(
        s["slots"]["text"], "Palo Alto",
        "cut to the field's maxlength, as typing is"
    );
    assert_eq!(s["slots"]["secret"], "hunter2");
    assert_eq!(
        p.focus(),
        Some(id(&p, "arm")),
        "an input moves no focus: it is where the tap left it"
    );

    let lines = said(&mut p, " hatch element ");
    let has = |part: &str| lines.iter().any(|l| l.ends_with(part));
    assert!(has(": click (delivery: hatch)"), "{lines:?}");
    assert!(has(": input (9 chars, delivery: hatch)"));
    assert!(has(": input (protected, delivery: hatch)"));
    assert!(has(": input refused: the field is disabled"));
    assert!(has(": input refused: the field is readonly"));
    assert!(has(": input refused: not an editable text field"));
    assert!(
        !journal(&mut p)
            .iter()
            .any(|l| l.contains("Palo") || l.contains("hunter")),
        "never the text"
    );
    assert_eq!(s["hatches"]["refused"], 3);
    // `clock settle` drains what is queued before it says settled; a hatch
    // whose acts keep causing acts is named after 16 drains, without hanging.
    tap(&mut p, "loop");
    let stuck = agent(&mut p, json!({"op": "clock", "settle": true}));
    assert_eq!(
        (stuck["settled"].clone(), stuck["reason"].clone()),
        (json!(false), json!("hatches"))
    );
    tap(&mut p, "stop");
    let done = agent(&mut p, json!({"op": "clock", "settle": true}));
    assert_eq!(done["settled"], true, "{done}");
    let s = state(&mut p);
    assert!(s["slots"]["presses"].as_u64() > Some(16), "{}", s["slots"]);
    assert_eq!(s["hatches"]["inFlight"], 0);
}

const WORDS: &str = r##"component App
  state shown = true
  action hide
    shown = false
  view
    column width=300 height=300
      button "Hide" testId="hide" press=hide height=30
      column hatch="known" testId="known" width=40 height=40
      when shown
        column hatch="unknown" testId="unknown" width=40 height=40
"##;

/// Says everything it is told, and when it is made.
struct Says {
    tickets: Vec<Ticket>,
}
impl Default for Says {
    fn default() -> Self {
        note("made");
        Says {
            tickets: Vec::new(),
        }
    }
}
impl Hatches for Says {
    fn app(&mut self, app: &App, _: &mut Context<'_, Self>) {
        note(format!(
            "app new={} {} owner={}",
            app.is_new, app.prefers_color_scheme, app.process_owner
        ));
    }
    fn app_ended(&mut self, app: &App, _: &mut Context<'_, Self>) {
        note(format!("app ended live={}", app.is_live));
    }
    fn window(&mut self, window: &Window, _: &mut Context<'_, Self>) {
        let f = window.frame;
        note(format!(
            "window new={} {}x{} exclusive={}",
            window.is_new, f.width, f.height, window.exclusive
        ));
    }
    fn window_ended(&mut self, window: &Window, _: &mut Context<'_, Self>) {
        note(format!("window ended live={}", window.is_live));
    }
    fn element(&mut self, e: &Element, context: &mut Context<'_, Self>) {
        note(format!(
            "element {} new={} id={:?}",
            e.hatch(),
            e.is_new(),
            e.id()
        ));
        if e.is_new() {
            self.tickets.push(context.frames(|_, _, _| note("tick")));
            e.diagnostics().count("built", 1);
        }
    }
    fn element_ended(&mut self, e: &Element, _: &mut Context<'_, Self>) {
        note(format!("element {} ended", e.hatch()));
    }
}

#[test]
fn only_this_platforms_words_are_called_and_never_before_first_pixel() {
    let mut p = boot::<Says>(WORDS, &["known"]);
    // Whatever is painted or committed, nothing of the module is made
    // before first pixel is acknowledged: the entry held only its type.
    let _ = p.frame();
    let hide = id(&p, "hide");
    p.tap(hide).unwrap();
    p.hatch_turn();
    assert!(notes().is_empty() && !p.hatches.connected());
    first_pixel(&mut p);
    let heard = notes();
    assert_eq!(heard[0], "made");
    assert_eq!(
        heard[1..],
        [
            "app new=true light owner=true",
            "window new=true 300x300 exclusive=true",
            "element known new=true id=\"\""
        ]
    );
    let hatches = state(&mut p)["hatches"].clone();
    assert_eq!(hatches["platform"], json!(["known"]));
    assert!(hatches["words"].get("unknown").is_none());
    assert_eq!(
        hatches["unhandled"],
        json!([]),
        "its node had left before the hatches connected"
    );
    let mut fresh = boot::<Says>(WORDS, &["known"]);
    first_pixel(&mut fresh);
    notes();
    let hatches = state(&mut fresh)["hatches"].clone();
    assert_eq!(
        hatches["unhandled"],
        json!([{"word": "unknown", "reason": "module"}])
    );
    let lines = said(&mut fresh, "not handled");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("hatch element unknown: not handled by this build's module; its nodes are shown and never called"));
    let journal = journal(&mut fresh);
    let at = |part: &str| {
        journal
            .iter()
            .position(|l| l.contains(part))
            .unwrap_or_else(|| panic!("{part}: {journal:?}"))
    };
    assert!(
        at("hatch: connected") < at("hatch app: built")
            && at("hatch app: built") < at("hatch window: built")
    );
    assert!(at("hatch window: built") < at("hatch element known #"));
    // An app with no hatches in its binary: its marked nodes are shown, and the journal says so.
    let plan = contract::compile(WORDS).unwrap().encode();
    let (mut bare, _) = Presenter::boot_with(
        &plan,
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    first_pixel(&mut bare);
    assert!(bare.hatch_state().is_none());
    assert_eq!(said(&mut bare, "hatch").len(), 1);
    assert!(said(&mut bare, "hatch")[0].ends_with("hatched nodes are shown and never called"));
}

#[test]
fn the_scopes_are_told_a_change_once_end_in_order_and_are_built_again_after_a_reload() {
    let mut p = boot::<Says>(WORDS, &["known", "unknown"]);
    first_pixel(&mut p);
    notes();
    // A fact that moves is one `changed`, however many moved; a new size is the window's.
    let prefer = json!({"op": "prefer", "media": {"prefers-color-scheme": "dark", "prefers-reduced-motion": "reduce"}});
    assert!(agent(&mut p, prefer).get("error").is_none());
    assert_eq!(notes(), ["app new=false dark owner=true"]);
    assert!(agent(&mut p, json!({"op": "tap", "resize": [260, 280]}))
        .get("error")
        .is_none());
    assert_eq!(notes(), ["window new=false 260x280 exclusive=true"]);
    let scopes = state(&mut p)["hatches"]["scopes"].clone();
    assert_eq!(scopes["app"]["calls"], json!({"built": 1, "changed": 1}));
    assert_eq!(scopes["window"]["calls"], json!({"built": 1, "changed": 1}));
    // A reload (§4.3): every node, then the window, then the app, ends; the
    // old incarnation's registrations are dropped; all are built again.
    let plan = contract::compile(WORDS).unwrap().encode();
    assert!(p.reload(&plan, ()).unwrap().is_none());
    assert_eq!(
        notes(),
        [
            "element known ended",
            "element unknown ended",
            "window ended live=false",
            "app ended live=false"
        ]
    );
    assert!(!p.hatch_ticking());
    p.hatch_turn();
    let heard = notes();
    assert_eq!(
        heard[..2],
        [
            "app new=true dark owner=true",
            "window new=true 260x280 exclusive=true"
        ]
    );
    assert_eq!(
        heard[2..],
        [
            "element known new=true id=\"\"",
            "element unknown new=true id=\"\""
        ]
    );
    let dropped = said(&mut p, "from before the reload");
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0]
        .ends_with("hatch frames: 2 ticket(s) and 0 after(s) from before the reload were dropped"));
    // A new incarnation's counts start at nothing.
    assert_eq!(
        state(&mut p)["hatches"]["words"]["known"]["counters"],
        json!({"built": 1})
    );
    // The session's end: the same order, and the module is gone.
    notes();
    p.end_hatches();
    assert_eq!(
        notes(),
        [
            "element known ended",
            "element unknown ended",
            "window ended live=false",
            "app ended live=false"
        ]
    );
    p.end_hatches();
    p.hatch_turn();
    assert!(notes().is_empty());
}
