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
}
