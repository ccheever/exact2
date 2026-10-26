//! What the presenter asks of the reader (LLP 1115.001 D5). The presenter
//! queues what it asks of the platform and wakes the reader by the
//! descriptor it listens to; the reader takes the queue here
//! (`Native.requests`) and answers by id (`Native.answer`). The queue, its
//! words and its kinds are the presenter's (`presenter/requests.rs`).

use super::CanvasHost;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// What the presenter asks of the reader since it last took them, as
    /// words ([`crate::presenter::Presenter::take_requests`]), with what the
    /// reader is told of the app's state when that changed (whether Back
    /// has somewhere to go, whether a field wants the keyboard). The state
    /// is not read during a scroll's own frames, which run no command and
    /// commit only rows: it is told with the frame after.
    ///
    /// The reader calls this after every turn of the host, which is where
    /// the journal is written out when asked for (`canvas/journal.rs`).
    pub fn requests(&mut self) -> Option<Vec<i32>> {
        self.echo_journal();
        crate::canvas::jni::jni_extensions::stamp(self.p.host().runner().now_ms());
        self.p.surfaces.publish_names();
        if let Some(names) = crate::canvas::jni::jni_extensions::take_bound() {
            self.bind_extensions(&names);
        }
        if !self.scrolled {
            // The reader takes its requests once it has the turn's frame:
            // after first pixel (a displayed frame activates it), the
            // hatches connect, once (LLP 1075.003.000.001 §2.1).
            if self.p.first_pixel_told() {
                self.p.connect_hatches();
            }
            self.p.tell_shell();
            self.p.tell_reader_hatches();
        }
        self.p.take_requests()
    }

    /// What the reader's hatches ask (`hatches::reader::act`), run at once,
    /// between turns: its answer, -1 when refused. A view hosted or let go
    /// paints again.
    pub fn hatch_act(&mut self, node: u32, kind: i32, text: Option<&str>) -> i64 {
        let answer = crate::hatches::reader::act(node, kind, text);
        if answer >= 0 {
            self.p.hatch_turn();
            if matches!(kind, 13 | 14) {
                self.p.hatch_views_changed();
            }
        }
        answer
    }

    /// An app module's event, only for the allocation whose view the reader still holds.
    pub fn module_event(
        &mut self,
        id: u32,
        key: exact_kernel::NodeKey,
        kind: &str,
        payload: String,
    ) {
        let now = self.now();
        self.force |= self.p.reader_module_event(id, key, kind, payload, now);
    }

    /// The reader's answer to request `id`, or its cancellation (`None`).
    pub fn answer(&mut self, id: u32, answer: Option<&str>) {
        self.p.answer_request(id, answer);
    }

    /// How a presence check the reader made ended (LLP 1133.010 D13).
    pub fn presence_reply(&mut self, answer: &str) {
        self.p
            .host_mut()
            .runner_mut()
            .presence_reply(answer.as_bytes());
        self.force = true;
    }

    /// What the activity asked for the session's orientation lock
    /// (`Orientation.kt`, LLP 1133.029 D6), and a line its resolution
    /// journals: a conflict, or a type this device does not show.
    pub fn orientation_applied(&mut self, applied: &str, conflict: bool, line: Option<&str>) {
        if let Some(line) = line {
            self.p.host_mut().runner_mut().log(line);
        }
        self.p.orientation_applied(applied.to_owned(), conflict);
        self.force = true;
    }

    /// The activity started or stopped: the page's visibility, which a
    /// return lock reads (LLP 1133.010 §9).
    pub fn page_hidden(&mut self, hidden: bool) {
        let page = self.p.host().runner().page();
        if page.hidden != hidden {
            let error = self.p.set_page(exact_runner::Page { hidden, ..page });
            if let Some(error) = error {
                eprintln!("exact: page: {error}");
            }
            self.force = true;
        }
    }

    /// A topic the reader's extensions announced (`ExtensionContext.changed`,
    /// LLP 1016.002): its readers are asked again.
    pub fn extension_changed(&mut self, topic: &str) {
        self.p.host().runner().native_slot().changed(topic);
    }

    /// The extensions the APK carries (`exact.app.ExactExtensions`, LLP
    /// 1133.001 D5), one a line: they answer their names from now on,
    /// outside the agent; under it, those `EXACT_AGENT_BIND` names
    /// (`--bind`) as in production, and the rest told they run under the
    /// agent, which answer only an interface that is its own substitute
    /// (LLP 1124 D7, LLP 1133.006 D8).
    pub fn bind_extensions(&mut self, names: &str) {
        let under_agent = crate::canvas::jni::jni_extensions::under_agent;
        let (under, real): (Vec<&str>, Vec<&str>) =
            (names.lines().filter(|n| !n.is_empty())).partition(|n| under_agent(n));
        let runner = self.p.host_mut().runner_mut();
        // An interface's `owned` and `images.owned` are by target (LLP
        // 1133.019 D11 item 4).
        runner.set_module_target("android");
        runner.bind_extensions(&real, &under);
        self.p.bind_pictures();
    }

    /// A package's picture for request `id` (`Native.picture`): its pixels,
    /// or the implementation's error.
    pub fn picture_answer(&mut self, id: u32, answer: Result<tiny_skia::Pixmap, String>) {
        self.force |= self.p.picture_answered(id, answer);
    }

    /// An extension announced picture `source` changed or revoked it
    /// (`ExtensionContext.changedPicture`, `revokePicture`).
    pub fn picture_changed(&mut self, source: &str, revoked: bool) {
        self.p.picture_changed(source, revoked);
        self.force = true;
    }
}
