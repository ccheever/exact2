//! What the device's shell tells the presenter about itself (LLP 1115.001):
//! the Canvas host's reader on Android, and an agent standing in for it.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The backend's reader was made again (LLP 1115.001 D15.1: Android made
    /// the activity again while this host ran): it holds none of the rows,
    /// faces or pictures the last one was sent. `backend` records for it
    /// from nothing, and the next frame paints everything again; the app,
    /// its state and its scroll offsets are as they were.
    pub fn reader_replaced(&mut self, backend: Box<dyn Backend>) {
        self.brush.replace_backend(backend);
        self.brush.forget_rows();
        self.dirty = true;
    }
}
