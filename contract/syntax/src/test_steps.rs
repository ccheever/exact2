//! Authored tests: a `test` block and its steps (LLP 1017 P7).
use crate::{Expr, Span, TapForm};

/// `test "name"` with steps: the agent API's own operations (LLP 1012) and
/// `expect` lines that read their replies — compiled to a script the agent
/// driver runs against the real hosts; never a second evaluator (LLP 1017 P7).
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    /// The name.
    pub name: String,
    /// `on web, macos`: the hosts the test is for; empty for every host.
    pub hosts: Vec<String>,
    /// The steps, in order.
    pub steps: Vec<Step>,
    /// Where.
    pub span: Span,
}
// Keep the census's vocabulary tied to the parser's exhaustive step enum.
macro_rules! steps {
    ($(#[$meta:meta])* pub enum Step { $($(#[$variant_meta:meta])* $kind:ident { $($fields:tt)* }),* $(,)? }) => {
        $(#[$meta])* pub enum Step { $($(#[$variant_meta])* $kind { $($fields)* }),* }
        impl Step {
            /// Every step kind the parser can produce.
            pub const KINDS: &[&str] = &[$(stringify!($kind)),*];
            /// This step's kind, from the parser's enum.
            pub fn kind(&self) -> &'static str {
                match self { $(Self::$kind { .. } => stringify!($kind)),* }
            }
        }
    };
}

steps! {
/// One step of a `test`.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// `tap "testId"`, or one of the driver's other forms of it
    /// (`modifiers "Shift"` for a press with keys held).
    Tap {
        /// The node, by `testId`.
        target: String,
        /// Which input: a press, unless the step names another.
        form: TapForm,
        /// `modifiers "Shift+Meta"`: the keys held through a press (empty
        /// for none; gallery F20).
        modifiers: String,
        /// Where.
        span: Span,
    },
    /// `tap "testId" drag dx dy [from x y] [mouse] [press ms] [over ms]
    /// [hold ms] [during "op" …]`: one whole drag from the node's middle, or
    /// from `from` in its box, a finger's or the left button's; the driver's
    /// `tap … drag` (kanban F18, files diary F10). `tap "A" drag to "B" [at x y] …`
    /// ends at B's middle, or at `at` from its top left (LLP 1094 D12).
    /// `during` is last: quoted reads or `clock` while the finger is down,
    /// after the move and before the hold (drums R8).
    Drag {
        /// The node, by `testId`.
        target: String,
        /// Points across (0 for a drag `to` a node).
        dx: f64,
        /// Points down (0 for a drag `to` a node).
        dy: f64,
        /// The node it ends on, by `testId`, and where in its box.
        to: Option<(String, Option<(f64, f64)>)>,
        /// Where it starts, an offset from the node's top left; its middle when `None`.
        from: Option<(f64, f64)>,
        /// The left button rather than a finger, where a carrier has both.
        mouse: bool,
        /// Milliseconds held before the move.
        press: Option<f64>,
        /// Milliseconds the move takes.
        over: Option<f64>,
        /// Milliseconds held after the move.
        hold: Option<f64>,
        /// Quoted ops run while the finger is down, after the move, before
        /// the hold. Empty when the step names none.
        during: Vec<String>,
        /// Where.
        span: Span,
    },
    /// `size 1200x800`: the viewport the test's session opens at, its first
    /// step (the driver's `--size`; kanban F18, paint F5).
    Size {
        /// Points across.
        width: f64,
        /// Points down.
        height: f64,
        /// Where.
        span: Span,
    },
    /// `epoch "2026-09-21T12:00:00Z"` (or Unix milliseconds, or `now`): the
    /// date at the session clock's zero, the driver's `--epoch` (habits F7).
    Epoch {
        /// An ISO date, whole milliseconds or `now`, as the driver takes it.
        value: String,
        /// Where.
        span: Span,
    },
    /// `time-zone "America/New_York"`: the session's IANA zone, `--time-zone`.
    TimeZone {
        /// The zone.
        zone: String,
        /// Where.
        span: Span,
    },
    /// `locale "fr-FR"`: the session's BCP 47 locale, `--locale`.
    Locale {
        /// The tag.
        tag: String,
        /// Where.
        span: Span,
    },
    /// A media preference, before observations or between steps.
    Prefer {
        /// CSS media feature name.
        name: String,
        /// The driver's value.
        value: String,
        /// Where.
        span: Span,
    },
    /// `before data` skips the initial `clock data` wait. A native app runs
    /// on real time before the driver connects; the web uses the agent's.
    BeforeData {
        /// Where.
        span: Span,
    },
    /// `fail fetch "https://api.example.com/" [times 2]` (LLP 1103): every
    /// later fetch whose URL starts with the prefix fails as a refused
    /// connection does, or the next `times` of them. Leading a test's steps
    /// (or at a file's top level) it is armed before the app's first data
    /// load, as a launch line; later it is a step.
    FailFetch {
        /// What a matching fetch's URL starts with.
        prefix: String,
        /// How many matching fetches fail; `None`, every one until `pass`.
        times: Option<u32>,
        /// Where.
        span: Span,
    },
    /// `pass fetch "…"`: the prefix stops failing (LLP 1103).
    PassFetch {
        /// The prefix a `fail fetch` armed.
        prefix: String,
        /// Where.
        span: Span,
    },
    /// `request POST "/items" {"title": "Bread"} as grace [expect 409]` (LLP 1116.006
    /// D8): the driver writes through the app's server as another person.
    Request {
        /// `GET`, `POST`, `PUT`, `PATCH` or `DELETE`.
        method: String,
        /// Under the server URL the app calls (`/items`), or a whole URL.
        path: String,
        /// The body, as JSON text.
        body: Option<String>,
        /// Whose bearer token it carries.
        persona: Option<String>,
        /// `expect 409` (LLP 1116.010 D3): the refusal's status; `None`: any 2xx.
        status: Option<u16>,
        /// Where.
        span: Span,
    },
    /// `fresh` or `fresh "seed.json"`: before the test opens, the Snapback
    /// dev server the app fetches is emptied (`POST /dev/reset`), and seeded
    /// with the file's `[{table, row}]` when one is named (a path from the
    /// test file). At a file's top level, before every test in it (the
    /// author's ruling of 2026-10-12, via the sb4 app farm).
    Fresh {
        /// The seed file, as written.
        seed: Option<String>,
        /// Where.
        span: Span,
    },
    /// `seed 7`: the session's `exactTime().seed`, `--seed`.
    Seed {
        /// A whole number from 0 through 2^32 − 1.
        seed: f64,
        /// Where.
        span: Span,
    },
    /// `type "testId" "text"`, or `… append`.
    Type {
        /// The field, by `testId`.
        target: String,
        /// The text.
        text: String,
        /// `append`: after the field's value, not in place of it (feed F8).
        append: bool,
        /// Where.
        span: Span,
    },
    /// `type "testId" key "Enter"`, or `down`, `up`, or `for <ms>` (platformer R7).
    Key {
        /// The field, by `testId`.
        target: String,
        /// The key's web name.
        key: String,
        /// `down` or `up`; `None` is a press (down and up).
        phase: Option<String>,
        /// Milliseconds the key stays down, on the virtual clock. Not with `phase`.
        duration: Option<f64>,
        /// Where.
        span: Span,
    },
    /// `pick "id" "path"…` / `pick "id" cancel`: answer the device request
    /// held at the node whose `id` (or `testId`) a picker named, or the one
    /// hold with that capability (`open-directory`, `pick`, `export`, …),
    /// whatever its ticket (files F11). Paths are the test file's.
    Pick {
        /// The node the answer arrives at, or a capability.
        target: String,
        /// The files or folders chosen; empty for `cancel`.
        paths: Vec<String>,
        /// Where.
        span: Span,
    },
    /// `authorize` / `authorize cancel`: complete the held sign-in (LLP 1146
    /// D4). The driver requests its authorization URL from the app's
    /// declared fixture server, redirects not followed, and answers the hold
    /// with the `Location` it returns; `cancel` answers 499.
    Authorize {
        /// `authorize cancel`.
        cancel: bool,
        /// Where.
        span: Span,
    },
    /// `type "testId" copy`, `… cut`, `… paste "text"`: the clipboard's
    /// event at the node, a paste carrying `text` as the clipboard's.
    Clipboard {
        /// The node, by `testId`.
        target: String,
        /// `copy`, `cut` or `paste`.
        edit: String,
        /// A paste's text; empty for copy and cut.
        text: String,
        /// Where.
        span: Span,
    },
    /// `clock settle`, `clock data`, `clock held …`, `clock until state …`,
    /// `clock +ms`, `clock +ms real`, `clock ms`.
    Clock {
        /// The argument as the agent takes it.
        arg: String,
        /// Where.
        span: Span,
    },
    /// `resize 800x600`: the window (a desktop's, the browser's), mid-test,
    /// as the driver's `resize` (reader: repagination on resize).
    Resize {
        /// Points.
        width: f64,
        /// Points.
        height: f64,
        /// Where.
        span: Span,
    },
    /// `screenshot "file.png"`.
    Screenshot {
        /// The file.
        path: String,
        /// Where.
        span: Span,
    },
    /// `expect refusal "mutation"`: the named send must be refused.
    ExpectRefusal {
        /// Mutation name, as reported by the runner.
        target: String,
        /// Where.
        span: Span,
    },
    /// `expect tree has "testId"` / `expect tree missing "testId"`.
    ExpectTree {
        /// The node, by `testId`.
        target: String,
        /// Present, or absent.
        present: bool,
        /// Where.
        span: Span,
    },
    /// `expect disabled "testId"` / `expect enabled "testId"` (LLP 1116.010 D3): whether
    /// the control the node names is disabled (its `disabled` attribute).
    ExpectDisabled {
        /// The node, by `testId`.
        target: String,
        /// `disabled`, or `enabled`.
        disabled: bool,
        /// Where.
        span: Span,
    },
    /// `expect text "testId" == "value"`: the node's text — its `text` prop,
    /// else its descendants' text in order (the web's `textContent`), else a
    /// field's value.
    ExpectText {
        /// The node, by `testId`.
        target: String,
        /// The text.
        value: String,
        /// Where.
        span: Span,
    },
    /// `reload`: the app restarts on the scratch store it had, so what it
    /// kept is what it reads (mail F19, kanban F25, weather F3).
    Reload {
        /// Where.
        span: Span,
    },
    /// `close`: the window's close button, as ⌘W or the red button press
    /// it, asking its `beforeunload` first (the driver's `close`, studio
    /// diary R17); a window a handler keeps stays, and the test goes on.
    Close {
        /// Where.
        span: Span,
    },
    /// `back`: the platform's Back, as a person takes it (LLP 1116.006 D4):
    /// an open dialog's cancel (Escape), else the top sheet dismissed (its
    /// swipe down), else the pushed route popped (iOS's Back button, the
    /// browser's Back, ⌘[). With nothing to go back from, the step fails.
    Back {
        /// Where.
        span: Span,
    },
    /// `expect state name == literal`: a slot, derive, or resource from the
    /// `state` reply, or a field of one (`name.field.field`, feed F10), or a
    /// list index (`rows.0`, drums R7), compared to a number (negative
    /// included), string, bool, or `none`.
    ExpectState {
        /// The declaration's name, then fields and list indexes, joined by `.`.
        name: String,
        /// The literal.
        value: Expr,
        /// Where.
        span: Span,
    },
    /// `expect mediasession FIELD == "value"` (LLP 1098 D10): a field of
    /// `state.mediaSession`, the owner by its testId; `none` for no owner.
    ExpectMediaSession {
        /// `owner`, `title`, `artist`, `album`, `artwork` or `playbackState`.
        field: String,
        /// The value; `None` is `none`.
        value: Option<String>,
        /// Where.
        span: Span,
    },
    /// `expect mediasession has|missing "action"` (LLP 1098 D10): whether
    /// the session's owner offers it to the platform.
    ExpectMediaSessionAction {
        /// `play`, `pause`, or one of the six.
        action: String,
        /// `has`, or `missing`.
        present: bool,
        /// Where.
        span: Span,
    },
    /// `expect sound has|missing "src" [at N] [gain N] [ends N]
    /// [playbackRate N] [pan N] [by word]`
    /// (LLP 1096 D10): a voice in the runner's record of that source that
    /// matches every clause given, or none that does.
    ExpectSound {
        /// The declared sound, by its path.
        src: String,
        /// `has`, or `missing`.
        present: bool,
        /// Its effective start, in runner milliseconds.
        at: Option<f64>,
        /// Its gain.
        gain: Option<f64>,
        /// Its end, in runner milliseconds.
        ends: Option<f64>,
        /// Its playback rate (LLP 1125 D10); a voice without one has 1.
        playback_rate: Option<f64>,
        /// Its pan; a voice without one has 0.
        pan: Option<f64>,
        /// How it ended: `end`, `group`, `cut`, `stop` or `cancelled` (D4).
        by: Option<String>,
        /// Where.
        span: Span,
    },
    /// `expect wakelock held|released`: the displayed elements requesting
    /// the lock, not whether the OS granted it (LLP 1125 D11).
    ExpectWakelock {
        /// Whether the holder record is nonempty.
        held: bool,
        /// Where.
        span: Span,
    },
    /// MIDI's steps (LLP 1125 §4): `prefer midi.…`, `tap midi`, `expect midi`.
    Midi {
        /// Which.
        step: crate::MidiStep,
        /// Where.
        span: Span,
    },
    /// `perf` with an indented block of steps: their work, counted as one
    /// window that the `expect perf` lines after it read (LLP 1146 D3).
    Perf {
        /// The measured steps.
        steps: Vec<Step>,
        /// Where.
        span: Span,
    },
    /// `expect perf query <resource> == n`, `built "<name>" <= n` or
    /// `evaluated "<name>" == n`: a count of the last `perf` block's window.
    ExpectPerf {
        /// `query`, `built` or `evaluated`.
        subject: String,
        /// The resource, or a `testId` or component name.
        target: String,
        /// `<=` rather than `==`.
        at_most: bool,
        /// The count.
        value: u32,
        /// Where.
        span: Span,
    },
    /// `expect no failures`: no resource or send failed, and the runner
    /// raised no alert, that the test did not arrange (LLP 1146 D4).
    ExpectNoFailures {
        /// Where.
        span: Span,
    },
}

}
