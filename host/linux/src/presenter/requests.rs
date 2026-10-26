//! One channel out (LLP 1115.001 D5): what the presenter asks of the shell
//! that shows it. Every call between the two goes from the shell into the
//! presenter; a haptic, an opened link, the share sheet, the clipboard, the
//! software keyboard and whether Back has somewhere to go need the other
//! direction, and they all take this one.
//!
//! The presenter queues a [`Request`] and wakes the shell by the wake it
//! already listens to (the executor's descriptor). It does not wait for a
//! paint: a haptic changes no pixel, and a frame may carry no stream. The
//! shell takes what is queued ([`Presenter::take_requests`], each request
//! once) and acts on its own thread. A request that has an answer (the
//! share sheet) is answered or cancelled later by its id
//! ([`Presenter::answer_request`]). No capability has a channel of its own,
//! and nothing layout needs while it runs goes through here: a queue taken
//! after the frame is too late for that.
//!
//! A host with no shell (desktop Linux, a headless run) queues nothing, and
//! each command keeps what it did there. On Android the shell is the Canvas
//! host's reader (`host/android/README.md` has the words as it reads them).
use super::*;
use exact_runner::orientation::Session;
use exact_runner::share::Share;

mod packages;
#[cfg(test)]
mod packages_tests;

/// What the presenter asks of its shell. The number is the kind on the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Leave the app for this absolute `http`, `https`, `mailto` or `tel`
    /// URL: the browser, the mail app, the dialer.
    Open(String),
    /// Play this haptic.
    Haptic(Haptic),
    /// Put this text on the clipboard.
    Copy(String),
    /// Show the system's share sheet with this. Answered `shared` or
    /// `dismissed`, or with why it could not be shown.
    Share(Share),
    /// Whether system Back has somewhere to go in the app (LLP 1115.001
    /// D6): the shell takes Back only while it has, so that Back at the
    /// first screen leaves with the system's own animation.
    Back(bool),
    /// The software keyboard: hidden (0), shown for an `input` (1) or for a
    /// `textarea` (3); and the field it is for where the shell hosts that
    /// field (LLP 1115.001 D8), which then takes the focus and shows its
    /// own keyboard, else 0 (a field this host paints).
    Keyboard(u32, ViewId),
    /// The top layer's surface (LLP 1115.001 D10): the window the shell
    /// shows for a confirmation or a menu, with its picture, or that it
    /// closed. A statement, told when it changes (`top.rs`). The shell says
    /// what happens in the window by a call of its own
    /// ([`Presenter::surface_event`]), not by this request's id.
    Surface(super::top::Surface),
    /// Whether a displayed element says `wakelock="screen"` (LLP 1125 D11):
    /// the shell keeps the screen on while it does. A statement, told when it
    /// changes.
    KeepScreenOn(bool),
    /// A date or time control's picker (LLP 1115.001 D9, as amended by
    /// 1115.001.000): the shell shows the platform's dialog for it and
    /// answers with the value chosen, in HTML's form, or cancels.
    Pick(Pick),
    /// The moments the reader's own hatches hear (LLP 1115.001.000 D4), as
    /// `hatches/reader.rs` writes them. Queued with no wake: the reader
    /// takes them with its requests after each turn.
    Hatch(Vec<i32>),
    /// A call of a module the reader's extensions implement (LLP 1133.001
    /// D5), its body as the runner wrote it (`{"extension", "source"|"send",
    /// "args"}` or `{"extension", "release"}`): answered with the reply's
    /// JSON, which is the call's.
    Module(String),
    /// The platform's check for a presence call (LLP 1133.010 D8), as
    /// `PresenceAsk::json` writes it: the reader's authenticator shows its
    /// prompt and the reader reads the item. Answered by `Native.presence`
    /// on the store's completion path, never by this request's id, so the
    /// value never passes the requests' journal.
    Presence(String),
    /// The session's orientation lock (LLP 1133.029 D6), told when it
    /// changes: none (the manifest's orientation), or the types' set with
    /// whether a holder says `natural` or `current`, and each counted
    /// holder's view and word for a conflict line the shell writes.
    Orientation(Session),
}

/// What a picker is opened with, each value as HTML writes it and empty
/// where the author gave none.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    /// The control.
    pub node: ViewId,
    /// `date`, `time` or `datetime-local`.
    pub kind: &'static str,
    /// What the control shows now.
    pub value: String,
    /// Its `min`.
    pub min: String,
    /// Its `max`.
    pub max: String,
    /// Its `step`.
    pub step: String,
    /// Its `accent-color`, where it or an ancestor sets one: the picker's
    /// colours are made from it, as the author's word over the device's.
    pub accent: Option<[u8; 4]>,
}

impl Request {
    fn kind(&self) -> i32 {
        match self {
            Request::Open(_) => 1,
            Request::Haptic(_) => 2,
            Request::Copy(_) => 3,
            Request::Share(_) => 4,
            Request::Back(_) => 5,
            Request::Keyboard(..) => 6,
            Request::Surface(_) => 7,
            Request::KeepScreenOn(_) => 8,
            Request::Pick(_) => 9,
            Request::Hatch(_) => 10,
            Request::Module(_) => 11,
            Request::Presence(_) => 12,
            Request::Orientation(_) => 13,
        }
    }
}

/// Exact's haptics (LLP 1077 D14): `-exact-press-haptic`'s kinds, and
/// `haptic()`'s, which adds the three outcomes. The number is the wire's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haptic {
    /// `selection`: a choice ticked over.
    Selection = 1,
    /// `impact-light`.
    ImpactLight = 2,
    /// `impact-medium`, and `haptic("impact")`.
    ImpactMedium = 3,
    /// `impact-heavy`.
    ImpactHeavy = 4,
    /// `impact-soft`.
    ImpactSoft = 5,
    /// `impact-rigid`.
    ImpactRigid = 6,
    /// `success`.
    Success = 7,
    /// `warning`.
    Warning = 8,
    /// `error`.
    Error = 9,
}

impl Haptic {
    /// The haptic a name means, as the Apple host reads `haptic()`'s
    /// argument (`Haptics.play`); `None` for a name no host plays.
    pub fn named(name: &str) -> Option<Haptic> {
        Some(match name {
            "selection" => Haptic::Selection,
            "impact-light" => Haptic::ImpactLight,
            "impact-medium" | "impact" => Haptic::ImpactMedium,
            "impact-heavy" => Haptic::ImpactHeavy,
            "impact-soft" => Haptic::ImpactSoft,
            "impact-rigid" => Haptic::ImpactRigid,
            "success" => Haptic::Success,
            "warning" => Haptic::Warning,
            "error" => Haptic::Error,
            _ => return None,
        })
    }

    fn pressed(kind: exact_kernel::PressHaptic) -> Option<Haptic> {
        use exact_kernel::PressHaptic as P;
        Some(match kind {
            P::None => return None,
            P::Selection => Haptic::Selection,
            P::ImpactLight => Haptic::ImpactLight,
            P::ImpactMedium => Haptic::ImpactMedium,
            P::ImpactHeavy => Haptic::ImpactHeavy,
            P::ImpactSoft => Haptic::ImpactSoft,
            P::ImpactRigid => Haptic::ImpactRigid,
        })
    }
}

/// A request the shell took and has not answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waiting {
    /// The share sheet: its outcome is a journal line (LLP 1069.003 D2).
    Share,
    /// A date or time control's picker: its answer is the control's value.
    Pick(ViewId),
    /// A package's picture (`pictures.rs`): answered with its pixels, by
    /// the image sync's ticket.
    Picture(u64),
}

/// Module calls' bodies and replies, handed over from the native slot's
/// handler on any thread.
type Inbox = std::sync::Arc<std::sync::Mutex<Vec<(Vec<u8>, exact_runner::Reply)>>>;

/// What the presenter keeps of its shell's requests.
#[derive(Default)]
pub(super) struct State {
    /// Whether a shell takes requests ([`Presenter::serve_requests`]).
    shell: bool,
    /// The last id given; ids start at 1 and are never reused.
    last: u32,
    queued: Vec<(u32, Request)>,
    waiting: BTreeMap<u32, Waiting>,
    /// What the shell was last told of Back, the kernel epoch that was read
    /// at, and what it was last told of the keyboard.
    back: Option<bool>,
    back_epoch: Option<u64>,
    keyboard: Option<(u32, ViewId)>,
    /// What the shell was last told of the wake lock and the kernel epoch
    /// that was read at; whether a host with no shell journaled it.
    wakelock: Option<bool>,
    wakelock_epoch: Option<u64>,
    wakelock_said: bool,
    /// The orientation lock the shell was last told and the kernel epoch it
    /// was read at; whether a host with no shell journaled it.
    orientation: Option<Session>,
    orientation_epoch: Option<u64>,
    orientation_said: bool,
    /// Module calls the runner handed the native slot, from any thread,
    /// until a turn queues them (`modules_in`).
    inbox: Inbox,
    /// Module calls the shell took, by id, until it answers.
    modules: BTreeMap<u32, exact_runner::Reply>,
    /// The packages this binary composes, which answer their extensions'
    /// calls on the loop's thread.
    packages: packages::Packages,
    /// Whether the native slot's handler is installed.
    handling: bool,
}

/// `EXACT_LOG_REQUESTS=1`: each request and each answer on stderr (a
/// haptic or a clipboard write shows nowhere else).
static LOG: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("EXACT_LOG_REQUESTS").is_ok_and(|v| v == "1"));

/// Length, then UTF-8 padded to whole words: a string as the op stream has it.
pub(super) fn string(out: &mut Vec<i32>, s: &str) {
    out.push(s.len() as i32);
    for chunk in s.as_bytes().chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        out.push(i32::from_le_bytes(word));
    }
}

/// One request as the shell reads it: its kind, its id, how many words
/// follow, then those words (a shell skips a kind it does not know).
fn encode(id: u32, request: &Request, out: &mut Vec<i32>) {
    let at = out.len();
    out.extend([request.kind(), id as i32, 0]);
    match request {
        Request::Open(text)
        | Request::Copy(text)
        | Request::Module(text)
        | Request::Presence(text) => string(out, text),
        Request::Haptic(haptic) => out.push(*haptic as i32),
        Request::Share(share) => {
            // Which of the three the app gave (bit 0 the title, 1 the text,
            // 2 the URL), then each, empty where it gave none.
            let parts = [&share.title, &share.text, &share.url];
            let given = |i: usize| i32::from(parts[i].is_some()) << i;
            out.push(given(0) | given(1) | given(2));
            for part in parts {
                string(out, part.as_deref().unwrap_or_default());
            }
        }
        Request::Back(goes) | Request::KeepScreenOn(goes) => out.push(i32::from(*goes)),
        Request::Keyboard(kind, field) => out.extend([*kind as i32, *field as i32]),
        // 0 for none; else 1, the types' bits (portrait-primary 1,
        // portrait-secondary 2, landscape-primary 4, landscape-secondary 8),
        // `natural`, `current`, the count of holders, then each one's view
        // and word.
        Request::Orientation(session) => match session.lock() {
            None => out.push(0),
            Some(lock) => {
                out.extend([1, i32::from(lock.types), i32::from(lock.natural)]);
                out.extend([i32::from(lock.current), lock.by.len() as i32]);
                for (view, word) in &lock.by {
                    out.push(*view as i32);
                    string(out, word);
                }
            }
        },
        Request::Surface(surface) => surface.encode(out),
        Request::Hatch(words) => out.extend(words),
        // The node, then the kind and the four values.
        Request::Pick(pick) => {
            out.push(pick.node as i32);
            for text in [pick.kind, &pick.value, &pick.min, &pick.max, &pick.step] {
                string(out, text);
            }
            // Opaque ARGB, or 0 for none.
            out.push(
                pick.accent
                    .map_or(0, |[r, g, b, _]| i32::from_be_bytes([0xff, r, g, b])),
            );
        }
    }
    out[at + 2] = (out.len() - at - 3) as i32;
}

impl<D: DataSource> Presenter<D> {
    /// A shell takes this presenter's requests from now on: the commands
    /// that ask the platform for something are queued for it, where a host
    /// with no shell refuses them or does nothing.
    pub fn serve_requests(&mut self) {
        self.requests.shell = true;
        (self.requests.wakelock, self.requests.wakelock_epoch) = (None, None);
        (self.requests.orientation, self.requests.orientation_epoch) = (None, None);
        // The shell's extensions answer the module calls the runner hands
        // the native slot, once their names are bound (`bind_extensions`).
        self.handle_modules();
    }

    /// The packages this binary composes (`crate::package`, LLP 1133.022
    /// D10 item 5): bound now, each told it runs under the agent unless the
    /// drive binds it (`--bind`), on the target this binary was built for.
    pub fn serve_packages(&mut self, table: Vec<crate::package::Entry>) {
        let agent = self.agent || std::env::var("EXACT_AGENT").as_deref() == Ok("1");
        self.requests.packages.set(table, agent);
        if self.requests.packages.is_empty() {
            return;
        }
        self.handle_modules();
        let (real, under) = self.requests.packages.bound();
        let target = if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };
        let runner = self.host.runner_mut();
        runner.set_module_target(target);
        runner.bind_extensions(&real, &under);
    }

    /// The session ends: each package instance's `destroy`.
    pub fn end_packages(&mut self) {
        self.requests.packages.destroy();
        self.package_lines();
    }

    /// The native slot's one handler: every module call queued, with its
    /// reply, for the loop's turn (`modules_in`), and the loop woken.
    fn handle_modules(&mut self) {
        if std::mem::replace(&mut self.requests.handling, true) {
            return;
        }
        let (inbox, wake) = (self.requests.inbox.clone(), self.executor.waker());
        let handler: exact_runner::NativeHandler = std::sync::Arc::new(move |body, reply| {
            inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((body, reply));
            wake();
        });
        self.host.runner().native_slot().host(Some(handler));
    }

    /// The module calls handed over since the last turn: a composed
    /// package's, called now; a shell's, queued for it, each answered by its
    /// id (`answer_request`); any other, `NotSupportedError`.
    pub(super) fn modules_in(&mut self) {
        let calls = std::mem::take(
            &mut *self
                .requests
                .inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        if calls.is_empty() {
            return;
        }
        let mut queued = false;
        let (native, wake) = (self.host.runner().native_slot(), self.executor.waker());
        for (body, reply) in calls {
            let extension = packages::extension_of(&body).unwrap_or_default();
            if self.requests.packages.serves(&extension) {
                let now = self.host.now();
                let inbox = self.requests.inbox.clone();
                (self.requests.packages).dispatch(&body, reply, now, &native, &inbox, &wake);
            } else if self.requests.shell {
                let id = self.queue(Request::Module(String::from_utf8_lossy(&body).into_owned()));
                self.requests.modules.insert(id, reply);
                queued = true;
            } else {
                crate::package::Reply::new(reply).error("NotSupportedError", None);
            }
        }
        self.package_lines();
        if queued {
            self.executor.notify();
        }
    }

    /// The packages' journal lines, into the journal.
    fn package_lines(&mut self) {
        for line in self.requests.packages.take_lines() {
            self.host.log(line);
        }
    }

    /// Whether a shell takes requests.
    pub(crate) fn has_shell(&self) -> bool {
        self.requests.shell
    }

    pub(super) fn queue(&mut self, request: Request) -> u32 {
        self.requests.last += 1;
        let id = self.requests.last;
        if *LOG {
            match &request {
                // Its picture is words by the hundred: what it is, not that.
                Request::Surface(s) => eprintln!(
                    "exact: request {id}: Surface {} {:?} {:?}, {} words",
                    s.serial,
                    s.kind,
                    s.frame,
                    s.stream.len()
                ),
                request => eprintln!("exact: request {id}: {request:?}"),
            }
        }
        self.requests.queued.push((id, request));
        id
    }

    /// Ask the shell for `request`: queued, and the shell woken by the wake
    /// it listens to for the executor's replies, whether or not a frame
    /// follows. Its id, which an answer names.
    pub(crate) fn request(&mut self, request: Request) -> u32 {
        let id = self.queue(request);
        self.executor.notify();
        id
    }

    /// What the shell is told of the app's state, when it is not what it
    /// was last told: whether Back has somewhere to go (read again only
    /// after a commit, since it walks the tree) and whether a text field
    /// holds the focus, which shows and hides the software keyboard. Queued
    /// with no wake: the shell calls this as it takes its requests.
    pub fn tell_shell(&mut self) {
        if !self.requests.shell {
            return;
        }
        // The top layer first: a surface that closes here changes Back.
        self.tell_surface();
        let epoch = self.host.kernel().epoch();
        if self.requests.back_epoch != Some(epoch) {
            self.requests.back_epoch = Some(epoch);
            let goes = self.back_goes();
            if self.requests.back != Some(goes) {
                self.requests.back = Some(goes);
                self.queue(Request::Back(goes));
            }
        }
        let keyboard = (self.editing(), self.hosted_focus());
        if self.requests.keyboard != Some(keyboard) {
            self.requests.keyboard = Some(keyboard);
            self.queue(Request::Keyboard(keyboard.0, keyboard.1));
        }
        if let Some(held) = self.wakelock_moved() {
            self.queue(Request::KeepScreenOn(held));
        }
        if let Some(session) = self.orientation_moved() {
            self.queue(Request::Orientation(session));
        }
        // A package view's displayed state, by the same rule, now the bars'
        // covers are set (LLP 1133.008 D8 item 2).
        self.host.runner_mut().report_views();
    }

    /// Whether a displayed element holds the wake lock (`exact_runner::wakelock`,
    /// read again only after a commit), when that is not what was last read.
    fn wakelock_moved(&mut self) -> Option<bool> {
        let epoch = self.host.kernel().epoch();
        if self.requests.wakelock_epoch == Some(epoch) {
            return None;
        }
        self.requests.wakelock_epoch = Some(epoch);
        let held = !exact_runner::wakelock::holders(self.host.runner()).is_empty();
        // Untold is off, as a new reader's view is (`keepScreenOn` false).
        (self.requests.wakelock.unwrap_or(false) != held).then(|| {
            self.requests.wakelock = Some(held);
            held
        })
    }

    /// A host with no shell (desktop Linux, Windows, a headless run) keeps no
    /// display awake: the first time an element holds the wake lock, the
    /// journal says so (LLP 1125 D11). Asked as a loop's turn runs its
    /// commands (`run_commands`), which Android's reader serves before.
    pub(super) fn wakelock_unavailable(&mut self) {
        if self.requests.shell || self.requests.wakelock_said || self.wakelock_moved() != Some(true)
        {
            return;
        }
        self.requests.wakelock_said = true;
        self.host
            .log("wakelock: unavailable: this host keeps no display awake");
    }

    /// The session's orientation lock (`exact_runner::orientation`, read
    /// again only after a commit), when it is not what the shell was last
    /// told; untold is none, as a new reader's activity has the manifest's.
    /// Reading it journals a conflict or a lock in a top-layer box.
    fn orientation_moved(&mut self) -> Option<Session> {
        let epoch = self.host.kernel().epoch();
        if self.requests.orientation_epoch == Some(epoch) {
            return None;
        }
        self.requests.orientation_epoch = Some(epoch);
        let session = self.host.runner_mut().orientation_session();
        let told = self.requests.orientation.as_ref().unwrap_or(&Session::None);
        (*told != session).then(|| {
            self.requests.orientation = Some(session.clone());
            session
        })
    }

    /// A host with no shell (desktop Linux, a headless run) has no screen
    /// orientation to lock (LLP 1133.029 D12): the first time a lock is
    /// held, the journal says so, and `state.orientationlock.applied` reads
    /// "unavailable". The Windows host applies the lock itself
    /// ([`Presenter::orientation_lock`]).
    pub(super) fn orientation_unavailable(&mut self) {
        if self.requests.shell || cfg!(windows) {
            return;
        }
        let held = self.orientation_moved().is_some_and(|s| s.lock().is_some());
        if held && !self.requests.orientation_said {
            self.requests.orientation_said = true;
            self.host
                .log("orientationlock: this host has no screen orientation");
        }
    }

    /// The session's orientation lock when it changed since the last call,
    /// for a host that applies it itself (Windows, LLP 1133.029 D8).
    pub fn orientation_lock(&mut self) -> Option<Session> {
        self.orientation_moved()
    }

    /// What the host applied (LLP 1133.029 §8), JSON, and whether its
    /// resolution found a conflict: `state.orientationlock.applied`.
    pub fn orientation_applied(&mut self, applied: String, conflict: bool) {
        self.host
            .runner_mut()
            .set_orientation_applied(applied, conflict);
    }

    /// Another host took over (`Presenter::replaced`), or the top layer
    /// opened or closed: the kernel may have the epoch the holds while
    /// displayed were last read at, so they are read again; what the shell
    /// was told stands.
    pub(super) fn holds_changed(&mut self) {
        self.requests.wakelock_epoch = None;
        self.requests.orientation_epoch = None;
    }

    /// Whether Back has somewhere to go may have changed with no commit (a
    /// surface of the top layer opened or closed): it is read again.
    pub(super) fn back_changed(&mut self) {
        self.requests.back_epoch = None;
    }

    /// The shell's own idea of the keyboard may no longer be what it was
    /// told (a hosted field's view took a focus this presenter refused):
    /// it is told again, and woken for it.
    pub(super) fn keyboard_changed(&mut self) {
        self.requests.keyboard = None;
        self.executor.notify();
    }

    /// Whether an editable text field holds the focus: 0 none, 1 an
    /// `input`, 3 a `textarea` (the keyboard then offers a newline and not
    /// a submit).
    fn editing(&self) -> u32 {
        let field = self
            .focus
            .and_then(|id| self.host.kernel().node(id))
            .filter(|n| {
                n.node_type == NodeType::TextInput
                    && n.props.bool(PropId::Editable) != Some(false)
                    && n.props.bool(PropId::Disabled) != Some(true)
            });
        match field {
            None => 0,
            Some(n) if n.props.str(PropId::SemanticTag) == Some("textarea") => 3,
            Some(_) => 1,
        }
    }

    /// The requests queued since the shell last took them, each once, as
    /// words: for each its kind, its id, the count of words that follow,
    /// and those words. `None` when nothing is queued.
    pub fn take_requests(&mut self) -> Option<Vec<i32>> {
        if self.requests.queued.is_empty() {
            return None;
        }
        let mut words = Vec::new();
        for (id, request) in std::mem::take(&mut self.requests.queued) {
            encode(id, &request, &mut words);
        }
        Some(words)
    }

    /// The shell's answer to request `id`, or its cancellation (`None`);
    /// whether a request waited under that id. An answer to an id that
    /// waits for none (answered already, or never asked) changes nothing.
    pub fn answer_request(&mut self, id: u32, answer: Option<&str>) -> bool {
        if *LOG {
            eprintln!("exact: answer {id}: {answer:?}");
        }
        if let Some(reply) = self.requests.modules.remove(&id) {
            reply.send(match answer {
                Some(json) => exact_runner::Outcome::Response(exact_runner::Response {
                    status: 200,
                    headers: Vec::new(),
                    body: json.as_bytes().to_vec(),
                }),
                None => exact_runner::Outcome::Failed {
                    kind: exact_runner::FailureKind::Aborted,
                    message: "the reader's extension did not answer".into(),
                },
            });
            return true;
        }
        let Some(waiting) = self.requests.waiting.remove(&id) else {
            return false;
        };
        match waiting {
            // The outcome is a journal line, as on every host (LLP 1069.003 D2).
            Waiting::Share => self.host.log(match answer {
                Some("shared") => "share: shared".to_string(),
                None | Some("dismissed") => "share: dismissed".to_string(),
                Some(reason) => format!("share: refused: {reason}"),
            }),
            // A choice is the person's, as a select's row is: `input`, then
            // `change`. A dismissed picker changes nothing.
            Waiting::Pick(node) => match answer {
                // The control may have become one a person cannot change
                // while its picker was up.
                Some(_) if self.pickable(node).is_none() => self
                    .host
                    .log(format!("picker: {node} no longer takes a choice")),
                Some(value) => {
                    if let Err(e) = self.set_control_value(node, value) {
                        self.host.log(format!("picker: {e}"));
                    }
                }
                None => self.host.log(format!("picker: dismissed for {node}")),
            },
            Waiting::Picture(ticket) => self.picture_cancelled(ticket, answer),
        }
        true
    }

    /// The shell was made again (`reader_replaced`): it is told the app's
    /// state anew, and what the last one was asked and never answered is
    /// cancelled, since nothing will answer it. A picker is the exception:
    /// it is the activity's, which answers it as it closes, whether the
    /// person chose or the activity went (`Requests.destroy`), and a reader
    /// made again over the same activity leaves it up.
    pub(super) fn requests_reset(&mut self) {
        (self.requests.back, self.requests.back_epoch) = (None, None);
        self.requests.keyboard = None;
        (self.requests.wakelock, self.requests.wakelock_epoch) = (None, None);
        let waiting: Vec<u32> = (self.requests.waiting.iter())
            .filter(|(_, w)| !matches!(w, Waiting::Pick(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in waiting {
            self.answer_request(id, None);
        }
    }

    /// Views a commit renewed (LLP 1078): one whose picker is up is another
    /// item now, so that picker's answer is not its (a list's row given
    /// another item while it was up).
    pub(crate) fn picks_renewed(&mut self, renewed: &[ViewId]) {
        let stale: Vec<(u32, ViewId)> = (self.requests.waiting.iter())
            .filter_map(|(id, w)| match w {
                Waiting::Pick(node) if renewed.contains(node) => Some((*id, *node)),
                _ => None,
            })
            .collect();
        for (id, node) in stale {
            self.requests.waiting.remove(&id);
            self.host.log(format!(
                "picker: {node} was made again; its answer is dropped"
            ));
        }
        // And one held for the agent.
        let kinds = ["date", "time", "datetime-local"];
        self.host.runner_mut().retire_holds_on(renewed, &kinds);
    }

    /// `id`'s kind, where it is a date or time control a person can change:
    /// not disabled, not read-only.
    fn pickable(&self, id: ViewId) -> Option<&'static str> {
        let node = self.host.kernel().node(id)?;
        if node.props.bool(PropId::Disabled) == Some(true)
            || node.props.bool(PropId::Editable) == Some(false)
        {
            return None;
        }
        match exact_kernel::ControlKind::of(node.node_type, node.props)? {
            exact_kernel::ControlKind::Date => Some("date"),
            exact_kernel::ControlKind::Time => Some("time"),
            exact_kernel::ControlKind::DateTimeLocal => Some("datetime-local"),
            _ => None,
        }
    }

    /// What the reader's hatches heard since it last took its requests.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn tell_reader_hatches(&mut self) {
        if let Some(words) = crate::hatches::reader::take() {
            self.queue(Request::Hatch(words));
        }
    }

    /// A press on a date or time control where a shell takes requests: its
    /// picker, with what the control shows now (the person's choice while
    /// the bound value is the one it was made at, else the bound value).
    /// A read-only control opens none, and one whose picker is up opens no
    /// second.
    pub(super) fn pick(&mut self, id: ViewId) {
        let Some(kind) = self.pickable(id) else {
            return;
        };
        if (self.requests.waiting.values()).any(|w| matches!(w, Waiting::Pick(n) if *n == id)) {
            return;
        }
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        let text = |prop| node.props.str(prop).unwrap_or_default().to_owned();
        let value = crate::paint::control::choice(&node, self.chosen.get(&id))
            .map(str::to_owned)
            .unwrap_or_else(|| text(PropId::Value));
        let pick = Pick {
            node: id,
            kind,
            value,
            min: text(PropId::Min),
            max: text(PropId::Max),
            step: text(PropId::Step),
            accent: crate::paint::control::accent(&node, self.brush.dark),
        };
        // Under the agent the picker is held for it, on every host (LLP
        // 1069.007 D4): `type @date 2026-10-23`, `tap @date clear` or
        // `tap @date cancel` answer it, by the control's kind.
        if self.agent {
            let held = (self.host.runner().device_holds().iter())
                .any(|h| h.node == Some(id) && h.capability == kind);
            if !held {
                let args = serde_json::json!({
                    "value": pick.value, "min": pick.min, "max": pick.max, "step": pick.step,
                });
                (self.host.runner_mut()).hold(kind, Some(id), &args.to_string(), &["clear"], true);
                self.dirty = true;
            }
            return;
        }
        let request = self.request(Request::Pick(pick));
        self.requests.waiting.insert(request, Waiting::Pick(id));
    }

    /// The agent's answer to a held date or time picker, after the runner
    /// consumed it: the value typed, empty for `clear`, nothing for
    /// `cancel`. It lands as a person's choice does.
    pub(crate) fn answer_date(&mut self, request: &str, reply: &str) {
        let r: serde_json::Value = serde_json::from_str(reply).unwrap_or_default();
        let kind = r["capability"].as_str().unwrap_or_default();
        if !matches!(kind, "date" | "time" | "datetime-local") {
            return;
        }
        let Some(node) = r["node"].as_u64().map(|n| n as ViewId) else {
            return;
        };
        let q: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
        let value = match r["answered"].as_str() {
            Some("cancel") => {
                self.host.log(format!("picker: dismissed for {node}"));
                return;
            }
            Some("clear") => "",
            _ => q["text"].as_str().unwrap_or_default().trim(),
        };
        if self.pickable(node) != Some(kind) {
            self.host
                .log(format!("picker: {node} no longer takes a choice"));
        } else if let Err(e) = self.set_control_value(node, value) {
            self.host.log(format!("picker: {e}"));
        }
    }

    /// `haptic(kind)`, and a pressable's `-exact-press-haptic`: the shell
    /// plays it. Desktop Linux has no haptic engine (LLP 1077 D14), so
    /// there is nothing to feel there.
    pub(crate) fn haptic(&mut self, kind: &str) {
        if !self.requests.shell {
            return;
        }
        match Haptic::named(kind) {
            Some(haptic) => {
                self.request(Request::Haptic(haptic));
            }
            None => self
                .host
                .log(format!("haptic {kind:?} is not one this platform plays")),
        }
    }

    /// A pointer pressed `id` (LLP 1077 D14): its `-exact-press-haptic`
    /// plays at the press, host-owned as `-exact-press-scale` is, with no
    /// commit.
    pub(super) fn press_haptic(&mut self, id: ViewId) {
        if !self.requests.shell {
            return;
        }
        let kind = self.host.kernel().node(id).map(|n| n.style.press_haptic);
        if let Some(haptic) = kind.and_then(Haptic::pressed) {
            self.request(Request::Haptic(haptic));
        }
    }

    /// `copyText(text)` on a host whose shell has a clipboard.
    pub(super) fn copy_text(&mut self, args: &[exact_plan::Value]) {
        match args {
            [text] if text.is_str() => {
                if let Some(clipboard) = &mut self.clipboard {
                    if let Err(error) = clipboard.write_text(text.text()) {
                        self.host.log(error);
                    }
                } else {
                    self.request(Request::Copy(text.text().to_string()));
                }
            }
            _ => self.host.log("copyText requires one string"),
        }
    }

    /// `openURL(url)` on a host whose shell can leave the app: an absolute
    /// `http`, `https`, `mailto` or `tel` URL, the one allowlist every host
    /// opens (a path is a location in the app, which `navigate` takes).
    pub(super) fn open_url(&mut self, args: &[exact_plan::Value]) {
        let url = args.first().and_then(exact_plan::Value::as_str);
        // The app's page in Settings (LLP 1133.002 D11), the shell's to open;
        // under the agent, journaled only.
        if url == Some("app-settings:") {
            if !self.has_shell() {
                self.host
                    .log("openURL app-settings: unavailable on this host");
                return;
            }
            let agent = std::env::var("EXACT_AGENT").is_ok_and(|v| v == "1");
            self.host.log(format!(
                "openURL app-settings: opens the app's page in Settings{}",
                if agent { " (not under the agent)" } else { "" }
            ));
            if !agent {
                self.request(Request::Open("app-settings:".into()));
            }
            return;
        }
        match url.and_then(|url| self.leave(url).then_some(())) {
            Some(()) => {}
            None => self.host.log(format!(
                "openURL refused: {:?} is not an absolute http, https, mailto or tel URL",
                url.unwrap_or_default()
            )),
        }
    }

    /// Leave the app for `url`, where a shell can and `url` is one of the
    /// four kinds that do; whether it was asked.
    pub(super) fn leave(&mut self, url: &str) -> bool {
        let target =
            crate::text::markup::target(url).filter(|t| !t.starts_with('/') && !t.contains('\0'));
        if let (Some(target), Some(open)) = (target, self.url_opener.as_ref()) {
            if let Err(error) = open(target) {
                self.host.log(format!("openURL: {error}"));
            }
            return true;
        }
        match target.filter(|_| self.requests.shell) {
            Some(target) => {
                self.request(Request::Open(target.to_string()));
                true
            }
            None => false,
        }
    }

    /// `share(…)`: the runner's rule, then the shell's sheet where there
    /// is one. Its outcome arrives by the request's id. The request carries
    /// no files: Android's sheet would take them as content URIs from a
    /// `FileProvider`, which the shell does not have (LLP 1125 D12).
    pub(super) fn share(&mut self, args: &[exact_plan::Value], source: Option<ViewId>) {
        const ANDROID: &str = "files are not carried to Android's share sheet yet";
        let share = Share::from_args(args);
        let (agent, shell) = (self.agent, self.requests.shell);
        let reach = exact_runner::share::Reach {
            sheet: shell,
            files: shell.then_some(ANDROID),
            pictures: shell.then_some(ANDROID),
        };
        let runner = self.host.runner_mut();
        if let exact_runner::share::Arm::Present(share, _) =
            exact_runner::share::arm(runner, share, source, agent, reach)
        {
            let id = self.request(Request::Share(share));
            self.requests.waiting.insert(id, Waiting::Share);
        }
    }

    /// A capability this channel does not carry yet (notifications, the
    /// pickers), after the runner refused it as unavailable: on a host with
    /// a shell the journal says the channel exists and what is missing is
    /// its request, so the refusal does not read as "this platform has none".
    pub(super) fn not_carried(&mut self, what: &str, reason: &str) {
        if self.requests.shell && reason == "unavailable" {
            self.host.log(format!(
                "{what}: the host's channel to the platform (LLP 1115.001 D5) does not carry it yet"
            ));
        }
    }
}

#[path = "pictures.rs"]
mod pictures;
#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "wakelock_tests.rs"]
mod wakelock_tests;
