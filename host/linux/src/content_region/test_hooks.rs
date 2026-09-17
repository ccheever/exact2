//! Test-only parking at the actual font-service call, never synthetic metrics.
use std::sync::{mpsc, Mutex};
use std::time::Duration;
type Gate = (mpsc::Sender<std::thread::ThreadId>, mpsc::Receiver<()>);
static NEXT_TEXT: Mutex<Option<Gate>> = Mutex::new(None);
pub(crate) struct Pause {
    entered: mpsc::Receiver<std::thread::ThreadId>,
    release: mpsc::Sender<()>,
}
pub(crate) fn next_text() -> Pause {
    let (entered, receive) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    assert!(NEXT_TEXT.lock().unwrap().replace((entered, wait)).is_none());
    Pause {
        entered: receive,
        release,
    }
}
pub(super) fn before_text() {
    let gate = NEXT_TEXT.lock().unwrap().take();
    if let Some((entered, wait)) = gate {
        let _ = entered.send(std::thread::current().id());
        let _ = wait.recv();
    }
}
impl Pause {
    pub(crate) fn entered(&self) {
        assert_ne!(
            self.entered.recv_timeout(Duration::from_secs(30)).unwrap(),
            std::thread::current().id()
        );
    }
}
impl Drop for Pause {
    fn drop(&mut self) {
        // Assertion unwinding must never strand an actual font worker.
        let _ = self.release.send(());
        NEXT_TEXT.lock().unwrap().take();
    }
}
