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
use exact_runner::share::Share;

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
}

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
        Request::Open(text) | Request::Copy(text) => string(out, text),
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
        Request::Surface(surface) => surface.encode(out),
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
    }

    /// Whether a displayed element holds the wake lock (`exact_runner::wakelock`,
    /// read again only after a commit), when that is not what was last read.
    fn wakelock_moved(&mut self) -> Option<bool> {
        let epoch = self.host.kernel().epoch();
        if self.requests.wakelock_epoch == Some(epoch) {
            return None;
        }
        self.requests.wakelock_epoch = Some(epoch);
        let held = !exact_runner::wakelock::holders(self.host.kernel()).is_empty();
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

    /// Another host took over (`Presenter::replaced`): its kernel may have
    /// the epoch the wake lock was last read at, so it is read again; what
    /// the shell was told stands.
    pub(super) fn wakelock_replaced(&mut self) {
        self.requests.wakelock_epoch = None;
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
        }
        true
    }

    /// The shell was made again (`reader_replaced`): it is told the app's
    /// state anew, and what the last one was asked and never answered is
    /// cancelled, since nothing will answer it.
    pub(super) fn requests_reset(&mut self) {
        (self.requests.back, self.requests.back_epoch) = (None, None);
        self.requests.keyboard = None;
        (self.requests.wakelock, self.requests.wakelock_epoch) = (None, None);
        let waiting: Vec<u32> = self.requests.waiting.keys().copied().collect();
        for id in waiting {
            self.answer_request(id, None);
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
                self.request(Request::Copy(text.text().to_string()));
            }
            _ => self.host.log("copyText requires one string"),
        }
    }

    /// `openURL(url)` on a host whose shell can leave the app: an absolute
    /// `http`, `https`, `mailto` or `tel` URL, the one allowlist every host
    /// opens (a path is a location in the app, which `navigate` takes).
    pub(super) fn open_url(&mut self, args: &[exact_plan::Value]) {
        let url = args.first().and_then(exact_plan::Value::as_str);
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
        let target = crate::text::markup::target(url).filter(|t| !t.starts_with('/'));
        match target.filter(|_| self.requests.shell) {
            Some(target) => {
                self.request(Request::Open(target.to_string()));
                true
            }
            None => false,
        }
    }

    /// `share(…)`: the runner's rule, then the shell's sheet where there
    /// is one. Its outcome arrives by the request's id.
    pub(super) fn share(&mut self, args: &[exact_plan::Value], source: Option<ViewId>) {
        let share = Share::from_args(args);
        let (agent, shell) = (self.agent, self.requests.shell);
        let runner = self.host.runner_mut();
        if let exact_runner::share::Arm::Present(share) =
            exact_runner::share::arm(runner, share, source, agent, shell)
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

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "wakelock_tests.rs"]
mod wakelock_tests;
