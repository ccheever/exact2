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
            self.p.tell_shell();
        }
        self.p.take_requests()
    }

    /// The reader's answer to request `id`, or its cancellation (`None`).
    pub fn answer(&mut self, id: u32, answer: Option<&str>) {
        self.p.answer_request(id, answer);
    }
}
